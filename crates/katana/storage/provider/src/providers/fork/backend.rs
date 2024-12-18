use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};
use std::pin::Pin;
use std::sync::mpsc::{
    channel as oneshot, Receiver as OneshotReceiver, RecvError, Sender as OneshotSender,
};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::{io, thread};

use anyhow::anyhow;
use futures::channel::mpsc::{channel as async_channel, Receiver, SendError, Sender};
use futures::future::BoxFuture;
use futures::stream::Stream;
use futures::{Future, FutureExt};
use katana_primitives::block::BlockHashOrNumber;
use katana_primitives::class::{ClassHash, CompiledClass, CompiledClassHash, FlattenedSierraClass};
use katana_primitives::contract::{ContractAddress, Nonce, StorageKey, StorageValue};
use katana_primitives::conversion::rpc::{
    compiled_class_hash_from_flattened_sierra_class, flattened_sierra_to_compiled_class,
    legacy_rpc_to_compiled_class,
};
use katana_primitives::Felt;
use parking_lot::Mutex;
use starknet::core::types::{BlockId, ContractClass as RpcContractClass, StarknetError};
use starknet::providers::{Provider, ProviderError as StarknetProviderError};
use tracing::{error, trace};

use crate::error::ProviderError;
use crate::providers::in_memory::cache::CacheStateDb;
use crate::traits::contract::ContractClassProvider;
use crate::traits::state::StateProvider;
use crate::ProviderResult;

const LOG_TARGET: &str = "forking::backend";

type BackendResult<T> = Result<T, BackendError>;

/// The types of response from [`Backend`].
#[derive(Debug, Clone)]
enum BackendResponse {
    Nonce(BackendResult<Nonce>),
    Storage(BackendResult<StorageValue>),
    ClassHashAt(BackendResult<ClassHash>),
    ClassAt(BackendResult<RpcContractClass>),
}

#[derive(Debug, thiserror::Error, Clone)]
pub enum BackendError {
    #[error("failed to send request to backend: {0}")]
    FailedSendRequest(#[from] SendError),
    #[error("failed to receive result from backend: {0}")]
    FailedReceiveResult(#[from] RecvError),
    #[error("compute class hash error: {0}")]
    ComputeClassHashError(Arc<anyhow::Error>),
    #[error("failed to spawn backend thread: {0}")]
    BackendThreadInit(#[from] Arc<io::Error>),
    #[error("rpc provider error: {0}")]
    StarknetProvider(#[from] Arc<starknet::providers::ProviderError>),
    #[error("unexpected received result: {0}")]
    UnexpectedReceiveResult(Arc<anyhow::Error>),
}

struct Request<P> {
    payload: P,
    sender: OneshotSender<BackendResponse>,
}

/// The types of request that can be sent to [`Backend`].
///
/// Each request consists of a payload and the sender half of a oneshot channel that will be used
/// to send the result back to the backend handle.
enum BackendRequest {
    Nonce(Request<ContractAddress>),
    Class(Request<ClassHash>),
    ClassHash(Request<ContractAddress>),
    Storage(Request<(ContractAddress, StorageKey)>),
    // Test-only request kind for requesting the backend stats
    #[cfg(test)]
    Stats(OneshotSender<usize>),
}

impl BackendRequest {
    /// Create a new request for fetching the nonce of a contract.
    fn nonce(address: ContractAddress) -> (BackendRequest, OneshotReceiver<BackendResponse>) {
        let (sender, receiver) = oneshot();
        (BackendRequest::Nonce(Request { payload: address, sender }), receiver)
    }

    /// Create a new request for fetching the class definitions of a contract.
    fn class(hash: ClassHash) -> (BackendRequest, OneshotReceiver<BackendResponse>) {
        let (sender, receiver) = oneshot();
        (BackendRequest::Class(Request { payload: hash, sender }), receiver)
    }

    /// Create a new request for fetching the class hash of a contract.
    fn class_hash(address: ContractAddress) -> (BackendRequest, OneshotReceiver<BackendResponse>) {
        let (sender, receiver) = oneshot();
        (BackendRequest::ClassHash(Request { payload: address, sender }), receiver)
    }

    /// Create a new request for fetching the storage value of a contract.
    fn storage(
        address: ContractAddress,
        key: StorageKey,
    ) -> (BackendRequest, OneshotReceiver<BackendResponse>) {
        let (sender, receiver) = oneshot();
        (BackendRequest::Storage(Request { payload: (address, key), sender }), receiver)
    }

    #[cfg(test)]
    fn stats() -> (BackendRequest, OneshotReceiver<usize>) {
        let (sender, receiver) = oneshot();
        (BackendRequest::Stats(sender), receiver)
    }
}

type BackendRequestFuture = BoxFuture<'static, BackendResponse>;

// Identifier for pending requests.
// This is used for request deduplication.
#[derive(Eq, Hash, PartialEq, Clone, Copy, Debug)]
enum BackendRequestIdentifier {
    Nonce(ContractAddress),
    Class(ClassHash),
    ClassHash(ContractAddress),
    Storage((ContractAddress, StorageKey)),
}

/// The backend for the forked provider.
///
/// It is responsible for processing [requests](BackendRequest) to fetch data from the remote
/// provider.
#[allow(missing_debug_implementations)]
pub struct Backend<P> {
    /// The Starknet RPC provider that will be used to fetch data from.
    provider: Arc<P>,
    // HashMap that keep track of current requests, for dedup purposes.
    request_dedup_map: HashMap<BackendRequestIdentifier, Vec<OneshotSender<BackendResponse>>>,
    /// Requests that are currently being poll.
    pending_requests: Vec<(BackendRequestIdentifier, BackendRequestFuture)>,
    /// Requests that are queued to be polled.
    queued_requests: VecDeque<BackendRequest>,
    /// A channel for receiving requests from the [BackendHandle]s.
    incoming: Receiver<BackendRequest>,
    /// Pinned block id for all requests.
    block: BlockId,
}

impl<P> Backend<P>
where
    P: Provider + Send + Sync + 'static,
{
    // TODO(kariy): create a `.start()` method start running the backend logic and let the users
    // choose which thread to running it on instead of spawning the thread ourselves.
    /// Create a new [Backend] with the given provider and block id, and returns a handle to it. The
    /// backend will start processing requests immediately upon creation.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(provider: P, block_id: BlockHashOrNumber) -> Result<BackendHandle, BackendError> {
        let (handle, backend) = Self::new_inner(provider, block_id);

        thread::Builder::new()
            .name("forking-backend".into())
            .spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to create tokio runtime")
                    .block_on(backend);
            })
            .map_err(|e| BackendError::BackendThreadInit(Arc::new(e)))?;

        trace!(target: LOG_TARGET, "Forking backend started.");

        Ok(handle)
    }

    fn new_inner(provider: P, block_id: BlockHashOrNumber) -> (BackendHandle, Backend<P>) {
        let block = match block_id {
            BlockHashOrNumber::Hash(hash) => BlockId::Hash(hash),
            BlockHashOrNumber::Num(number) => BlockId::Number(number),
        };

        // Create async channel to receive requests from the handle.
        let (tx, rx) = async_channel(100);
        let backend = Backend {
            block,
            incoming: rx,
            provider: Arc::new(provider),
            request_dedup_map: HashMap::new(),
            pending_requests: Vec::new(),
            queued_requests: VecDeque::new(),
        };

        (BackendHandle(Mutex::new(tx)), backend)
    }

    /// This method is responsible for transforming the incoming request
    /// sent from a [BackendHandle] into a RPC request to the remote network.
    fn handle_requests(&mut self, request: BackendRequest) {
        let block = self.block;
        let provider = self.provider.clone();

        // Check if there are similar requests in the queue before sending the request
        match request {
            BackendRequest::Nonce(Request { payload, sender }) => {
                let req_key = BackendRequestIdentifier::Nonce(payload);

                self.dedup_request(
                    req_key,
                    sender,
                    Box::pin(async move {
                        let res = provider
                            .get_nonce(block, Felt::from(payload))
                            .await
                            .map_err(|e| BackendError::StarknetProvider(Arc::new(e)));
                        BackendResponse::Nonce(res)
                    }),
                );
            }

            BackendRequest::Storage(Request { payload: (addr, key), sender }) => {
                let req_key = BackendRequestIdentifier::Storage((addr, key));

                self.dedup_request(
                    req_key,
                    sender,
                    Box::pin(async move {
                        let res = provider
                            .get_storage_at(Felt::from(addr), key, block)
                            .await
                            .map_err(|e| BackendError::StarknetProvider(Arc::new(e)));

                        BackendResponse::Storage(res)
                    }),
                );
            }

            BackendRequest::ClassHash(Request { payload, sender }) => {
                let req_key = BackendRequestIdentifier::ClassHash(payload);

                self.dedup_request(
                    req_key,
                    sender,
                    Box::pin(async move {
                        let res = provider
                            .get_class_hash_at(block, Felt::from(payload))
                            .await
                            .map_err(|e| BackendError::StarknetProvider(Arc::new(e)));

                        BackendResponse::ClassHashAt(res)
                    }),
                );
            }

            BackendRequest::Class(Request { payload, sender }) => {
                let req_key = BackendRequestIdentifier::Class(payload);

                self.dedup_request(
                    req_key,
                    sender,
                    Box::pin(async move {
                        let res = provider
                            .get_class(block, payload)
                            .await
                            .map_err(|e| BackendError::StarknetProvider(Arc::new(e)));

                        BackendResponse::ClassAt(res)
                    }),
                );
            }

            #[cfg(test)]
            BackendRequest::Stats(sender) => {
                let total_ongoing_request = self.pending_requests.len();
                sender.send(total_ongoing_request).expect("failed to send backend stats");
            }
        }
    }

    fn dedup_request(
        &mut self,
        req_key: BackendRequestIdentifier,
        sender: OneshotSender<BackendResponse>,
        rpc_call_future: BoxFuture<'static, BackendResponse>,
    ) {
        if let Entry::Vacant(e) = self.request_dedup_map.entry(req_key) {
            self.pending_requests.push((req_key, rpc_call_future));
            e.insert(vec![sender]);
        } else {
            match self.request_dedup_map.get_mut(&req_key) {
                Some(sender_vec) => {
                    sender_vec.push(sender);
                }
                None => {
                    // Log this and do nothing here, as this should never happen.
                    // If this does happen it is an unexpected bug.
                    error!(target: LOG_TARGET, "failed to get current request dedup vector");
                }
            }
        }
    }
}

impl<P> Future for Backend<P>
where
    P: Provider + Send + Sync + 'static,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let pin = self.get_mut();
        loop {
            // convert all queued requests into futures to be polled
            while let Some(req) = pin.queued_requests.pop_front() {
                pin.handle_requests(req);
            }

            loop {
                match Pin::new(&mut pin.incoming).poll_next(cx) {
                    Poll::Ready(Some(req)) => {
                        pin.queued_requests.push_back(req);
                    }
                    // Resolve if stream is exhausted.
                    Poll::Ready(None) => {
                        return Poll::Ready(());
                    }
                    Poll::Pending => {
                        break;
                    }
                }
            }

            // poll all pending requests
            for n in (0..pin.pending_requests.len()).rev() {
                let (fut_key, mut fut) = pin.pending_requests.swap_remove(n);
                // poll the future and if the future is still pending, push it back to the
                // pending requests so that it will be polled again
                match fut.poll_unpin(cx) {
                    Poll::Pending => {
                        pin.pending_requests.push((fut_key, fut));
                    }
                    Poll::Ready(res) => {
                        let sender_vec = pin
                            .request_dedup_map
                            .get(&fut_key)
                            .expect("failed to get sender vector");

                        // Send the response to all the senders waiting on the same request
                        sender_vec.iter().for_each(|sender| {
                            sender.send(res.clone()).unwrap_or_else(|error| {
                            	error!(target: LOG_TARGET, key = ?fut_key, %error, "Failed to send result.")
                            });
                        });

                        pin.request_dedup_map.remove(&fut_key);
                    }
                }
            }

            // if no queued requests, then yield
            if pin.queued_requests.is_empty() {
                return Poll::Pending;
            }
        }
    }
}

/// A thread safe handler to [`Backend`].
///
/// This is the primary interface for sending request to the backend to fetch data from the remote
/// network.
#[derive(Debug)]
pub struct BackendHandle(Mutex<Sender<BackendRequest>>);

impl Clone for BackendHandle {
    fn clone(&self) -> Self {
        Self(Mutex::new(self.0.lock().clone()))
    }
}

impl BackendHandle {
    pub fn get_nonce(&self, address: ContractAddress) -> Result<Nonce, BackendError> {
        trace!(target: LOG_TARGET, %address, "Requesting contract nonce.");
        let (req, rx) = BackendRequest::nonce(address);
        self.request(req)?;
        match rx.recv()? {
            BackendResponse::Nonce(res) => res,
            response => {
                Err(BackendError::UnexpectedReceiveResult(Arc::new(anyhow!("{:?}", response))))
            }
        }
    }

    pub fn get_storage(
        &self,
        address: ContractAddress,
        key: StorageKey,
    ) -> Result<StorageValue, BackendError> {
        trace!(target: LOG_TARGET, %address, key = %format!("{key:#x}"), "Requesting contract storage.");
        let (req, rx) = BackendRequest::storage(address, key);
        self.request(req)?;
        match rx.recv()? {
            BackendResponse::Storage(res) => res,
            response => {
                Err(BackendError::UnexpectedReceiveResult(Arc::new(anyhow!("{:?}", response))))
            }
        }
    }

    pub fn get_class_hash_at(&self, address: ContractAddress) -> Result<ClassHash, BackendError> {
        trace!(target: LOG_TARGET, %address, "Requesting contract class hash.");
        let (req, rx) = BackendRequest::class_hash(address);
        self.request(req)?;
        match rx.recv()? {
            BackendResponse::ClassHashAt(res) => res,
            response => {
                Err(BackendError::UnexpectedReceiveResult(Arc::new(anyhow!("{:?}", response))))
            }
        }
    }

    pub fn get_class_at(&self, class_hash: ClassHash) -> Result<RpcContractClass, BackendError> {
        trace!(target: LOG_TARGET, class_hash = %format!("{class_hash:#x}"), "Requesting class.");
        let (req, rx) = BackendRequest::class(class_hash);
        self.request(req)?;
        match rx.recv()? {
            BackendResponse::ClassAt(res) => res,
            response => {
                Err(BackendError::UnexpectedReceiveResult(Arc::new(anyhow!("{:?}", response))))
            }
        }
    }

    pub fn get_compiled_class_hash(
        &self,
        class_hash: ClassHash,
    ) -> Result<CompiledClassHash, BackendError> {
        trace!(target: LOG_TARGET, class_hash = %format!("{class_hash:#x}"), "Requesting compiled class hash.");
        let class = self.get_class_at(class_hash)?;
        // if its a legacy class, then we just return back the class hash
        // else if sierra class, then we have to compile it and compute the compiled class hash.
        match class {
            RpcContractClass::Legacy(_) => Ok(class_hash),
            RpcContractClass::Sierra(sierra_class) => {
                compiled_class_hash_from_flattened_sierra_class(&sierra_class)
                    .map_err(|e| BackendError::ComputeClassHashError(Arc::new(e)))
            }
        }
    }

    /// Send a request to the backend thread.
    fn request(&self, req: BackendRequest) -> Result<(), BackendError> {
        self.0.lock().try_send(req).map_err(|e| e.into_send_error())?;
        Ok(())
    }

    #[cfg(test)]
    fn stats(&self) -> Result<usize, BackendError> {
        let (req, rx) = BackendRequest::stats();
        self.request(req)?;
        Ok(rx.recv()?)
    }
}

/// A shared cache that stores data fetched from the forked network.
///
/// Check in cache first, if not found, then fetch from the forked provider and store it in the
/// cache to avoid fetching it again. This is shared across multiple instances of
/// [`ForkedStateDb`](super::state::ForkedStateDb).
#[derive(Clone, Debug)]
pub struct SharedStateProvider(pub(crate) Arc<CacheStateDb<BackendHandle>>);

impl SharedStateProvider {
    pub(crate) fn new_with_backend(backend: BackendHandle) -> Self {
        Self(Arc::new(CacheStateDb::new(backend)))
    }
}

impl StateProvider for SharedStateProvider {
    fn nonce(&self, address: ContractAddress) -> ProviderResult<Option<Nonce>> {
        // TEMP:
        //
        // The nonce and class hash are stored in the same struct, so if we call either `nonce` or
        // `class_hash_of_contract` first, the other would be filled with the default value.
        // Currently, the data types that we're using doesn't allow us to distinguish between
        // 'not fetched' vs the actual value.
        //
        // Right now, if the nonce value is 0, we couldn't distinguish whether that is the actual
        // value or just the default value. So this filter is a pessimistic approach to always
        // invalidate 0 nonce value in the cache.
        //
        // Meaning, if the nonce is 0, we always fetch the nonce from the forked provider, even if
        // we already fetched it before.
        //
        // Similar story with `class_hash_of_contract`
        //
        if let nonce @ Some(_) = self
            .0
            .contract_state
            .read()
            .get(&address)
            .map(|i| i.nonce)
            .filter(|n| n != &Nonce::ZERO)
        {
            return Ok(nonce);
        }

        if let Some(nonce) = handle_not_found_err(self.0.get_nonce(address)).map_err(|error| {
            error!(target: LOG_TARGET, %address, %error, "Fetching nonce.");
            error
        })? {
            self.0.contract_state.write().entry(address).or_default().nonce = nonce;
            Ok(Some(nonce))
        } else {
            Ok(None)
        }
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>> {
        if let value @ Some(_) =
            self.0.storage.read().get(&address).and_then(|s| s.get(&storage_key))
        {
            return Ok(value.copied());
        }

        let value =
            handle_not_found_err(self.0.get_storage(address, storage_key)).map_err(|error| {
                error!(target: LOG_TARGET, %address, storage_key = %format!("{storage_key:#x}"), %error, "Fetching storage value.");
                error
            })?;

        self.0
            .storage
            .write()
            .entry(address)
            .or_default()
            .insert(storage_key, value.unwrap_or_default());

        Ok(value)
    }

    fn class_hash_of_contract(
        &self,
        address: ContractAddress,
    ) -> ProviderResult<Option<ClassHash>> {
        // See comment at `nonce` for the explanation of this filter.
        if let hash @ Some(_) = self
            .0
            .contract_state
            .read()
            .get(&address)
            .map(|i| i.class_hash)
            .filter(|h| h != &ClassHash::ZERO)
        {
            return Ok(hash);
        }

        if let Some(hash) =
            handle_not_found_err(self.0.get_class_hash_at(address)).map_err(|error| {
                error!(target: LOG_TARGET, %address, %error, "Fetching class hash.");
                error
            })?
        {
            self.0.contract_state.write().entry(address).or_default().class_hash = hash;
            Ok(Some(hash))
        } else {
            Ok(None)
        }
    }
}

impl ContractClassProvider for SharedStateProvider {
    fn sierra_class(&self, hash: ClassHash) -> ProviderResult<Option<FlattenedSierraClass>> {
        if let class @ Some(_) = self.0.shared_contract_classes.sierra_classes.read().get(&hash) {
            return Ok(class.cloned());
        }

        let Some(class) = handle_not_found_err(self.0.get_class_at(hash)).map_err(|error| {
            error!(target: LOG_TARGET, hash = %format!("{hash:#x}"), %error, "Fetching sierra class.");
            error
        })?
        else {
            return Ok(None);
        };

        match class {
            starknet::core::types::ContractClass::Legacy(_) => Ok(None),
            starknet::core::types::ContractClass::Sierra(sierra_class) => {
                self.0
                    .shared_contract_classes
                    .sierra_classes
                    .write()
                    .insert(hash, sierra_class.clone());
                Ok(Some(sierra_class))
            }
        }
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> ProviderResult<Option<CompiledClassHash>> {
        if let hash @ Some(_) = self.0.compiled_class_hashes.read().get(&hash) {
            return Ok(hash.cloned());
        }

        if let Some(hash) =
            handle_not_found_err(self.0.get_compiled_class_hash(hash)).map_err(|error| {
                error!(target: LOG_TARGET, hash = %format!("{hash:#x}"), %error, "Fetching compiled class hash.");
                error
            })?
        {
            self.0.compiled_class_hashes.write().insert(hash, hash);
            Ok(Some(hash))
        } else {
            Ok(None)
        }
    }

    fn class(&self, hash: ClassHash) -> ProviderResult<Option<CompiledClass>> {
        if let Some(class) = self.0.shared_contract_classes.compiled_classes.read().get(&hash) {
            return Ok(Some(class.clone()));
        }

        let Some(class) = handle_not_found_err(self.0.get_class_at(hash)).map_err(|error| {
            error!(target: LOG_TARGET, hash = %format!("{hash:#x}"), %error, "Fetching class.");
            error
        })?
        else {
            return Ok(None);
        };

        let (class_hash, compiled_class_hash, casm, sierra) = match class {
            RpcContractClass::Legacy(class) => {
                let (_, compiled_class) = legacy_rpc_to_compiled_class(&class).map_err(|error| {
                    error!(target: LOG_TARGET, hash = %format!("{hash:#x}"), %error, "Parsing legacy class.");
                    ProviderError::ParsingError(error.to_string())
                })?;

                (hash, hash, compiled_class, None)
            }

            RpcContractClass::Sierra(sierra_class) => {
                let (_, compiled_class_hash, compiled_class) =
                    flattened_sierra_to_compiled_class(&sierra_class).map_err(|error| {
                        error!(target: LOG_TARGET, hash = %format!("{hash:#x}"), %error, "Parsing sierra class.");
                        ProviderError::ParsingError(error.to_string())
                    })?;

                (hash, compiled_class_hash, compiled_class, Some(sierra_class))
            }
        };

        self.0.compiled_class_hashes.write().insert(class_hash, compiled_class_hash);

        self.0
            .shared_contract_classes
            .compiled_classes
            .write()
            .entry(class_hash)
            .or_insert(casm.clone());

        if let Some(sierra) = sierra {
            self.0
                .shared_contract_classes
                .sierra_classes
                .write()
                .entry(class_hash)
                .or_insert(sierra);
        }

        Ok(Some(casm))
    }
}

/// A helper function to convert a contract/class not found error returned by the RPC provider into
/// a `Option::None`.
///
/// This is to follow the Katana's provider APIs convention where 'not found'/'non-existent' should
/// be represented as `Option::None`.
fn handle_not_found_err<T>(result: Result<T, BackendError>) -> Result<Option<T>, BackendError> {
    match result {
        Ok(value) => Ok(Some(value)),

        Err(BackendError::StarknetProvider(err_in_arc)) => match err_in_arc.as_ref() {
            StarknetProviderError::StarknetError(
                StarknetError::ContractNotFound | StarknetError::ClassHashNotFound,
            ) => Ok(None),
            _ => Err(BackendError::StarknetProvider(err_in_arc)),
        },

        Err(e) => Err(e),
    }
}

#[cfg(test)]
pub(crate) mod test_utils {

    use std::sync::mpsc::{sync_channel, SyncSender};

    use katana_primitives::block::BlockNumber;
    use starknet::providers::jsonrpc::HttpTransport;
    use starknet::providers::JsonRpcClient;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use url::Url;

    use super::*;

    pub fn create_forked_backend(rpc_url: &str, block_num: BlockNumber) -> BackendHandle {
        let url = Url::parse(rpc_url).expect("valid url");
        let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(url)));
        Backend::new(provider, block_num.into()).unwrap()
    }

    // Starts a TCP server that never close the connection.
    pub fn start_tcp_server(addr: String) {
        use tokio::runtime::Builder;

        let (tx, rx) = sync_channel::<()>(1);
        thread::spawn(move || {
            Builder::new_current_thread().enable_all().build().unwrap().block_on(async move {
                let listener = TcpListener::bind(addr).await.unwrap();
                let mut connections = Vec::new();

                tx.send(()).unwrap();

                loop {
                    let (socket, _) = listener.accept().await.unwrap();
                    connections.push(socket);
                }
            });
        });

        rx.recv().unwrap();
    }

    // Helper function to start a TCP server that returns predefined JSON-RPC responses
    pub fn start_mock_rpc_server(addr: String, response: String) -> SyncSender<()> {
        use tokio::runtime::Builder;
        let (tx, rx) = sync_channel::<()>(1);

        thread::spawn(move || {
            Builder::new_current_thread().enable_all().build().unwrap().block_on(async move {
                let listener = TcpListener::bind(addr).await.unwrap();

                loop {
                    let (mut socket, _) = listener.accept().await.unwrap();

                    // Read the request, so hyper would not close the connection
                    let mut buffer = [0; 1024];
                    let _ = socket.read(&mut buffer).await.unwrap();

                    // Wait for a signal to return the response.
                    rx.recv().unwrap();

                    // After reading, we send the pre-determined response
                    let http_response = format!(
                        "HTTP/1.1 200 OK\r\ncontent-length: {}\r\ncontent-type: \
                         application/json\r\n\r\n{}",
                        response.len(),
                        response
                    );

                    socket.write_all(http_response.as_bytes()).await.unwrap();
                    socket.flush().await.unwrap();
                }
            });
        });

        // Returning the sender to allow controlling the response timing.
        tx
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::Duration;

    use katana_primitives::contract::GenericContractInfo;
    use starknet::macros::felt;

    use super::test_utils::*;
    use super::*;

    const LOCAL_RPC_URL: &str = "http://localhost:5050";

    const STORAGE_KEY: StorageKey = felt!("0x1");
    const ADDR_1: ContractAddress = ContractAddress(felt!("0xADD1"));
    const ADDR_1_NONCE: Nonce = felt!("0x1");
    const ADDR_1_STORAGE_VALUE: StorageKey = felt!("0x8080");
    const ADDR_1_CLASS_HASH: StorageKey = felt!("0x1");

    const ERROR_SEND_REQUEST: &str = "Failed to send request to backend";
    const ERROR_STATS: &str = "Failed to get stats";

    #[test]
    fn handle_incoming_requests() {
        // start a mock remote network
        start_tcp_server("127.0.0.1:8080".to_string());

        let handle = create_forked_backend("http://127.0.0.1:8080", 1);

        // check no pending requests
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 0, "Backend should not have any ongoing requests.");

        // send requests to the backend
        let h1 = handle.clone();
        thread::spawn(move || {
            h1.get_nonce(felt!("0x1").into()).expect(ERROR_SEND_REQUEST);
        });
        let h2 = handle.clone();
        thread::spawn(move || {
            h2.get_class_at(felt!("0x1")).expect(ERROR_SEND_REQUEST);
        });
        let h3 = handle.clone();
        thread::spawn(move || {
            h3.get_compiled_class_hash(felt!("0x2")).expect(ERROR_SEND_REQUEST);
        });
        let h4 = handle.clone();
        thread::spawn(move || {
            h4.get_class_hash_at(felt!("0x1").into()).expect(ERROR_SEND_REQUEST);
        });
        let h5 = handle.clone();
        thread::spawn(move || {
            h5.get_storage(felt!("0x1").into(), felt!("0x1")).expect(ERROR_SEND_REQUEST);
        });

        // wait for the requests to be handled
        thread::sleep(Duration::from_secs(1));

        // check request are handled
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 5, "Backend should have 5 ongoing requests.")
    }

    #[test]
    fn get_nonce_request_should_be_deduplicated() {
        // start a mock remote network
        start_tcp_server("127.0.0.1:8081".to_string());

        let handle = create_forked_backend("http://127.0.0.1:8081", 1);

        // check no pending requests
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 0, "Backend should not have any ongoing requests.");

        // send requests to the backend
        let h1 = handle.clone();
        thread::spawn(move || {
            h1.get_nonce(felt!("0x1").into()).expect(ERROR_SEND_REQUEST);
        });
        let h2 = handle.clone();
        thread::spawn(move || {
            h2.get_nonce(felt!("0x1").into()).expect(ERROR_SEND_REQUEST);
        });

        // check current request count
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 1, "Backend should have 1 ongoing requests.");

        // Different request, should be counted
        let h3 = handle.clone();
        thread::spawn(move || {
            h3.get_nonce(felt!("0x2").into()).expect(ERROR_SEND_REQUEST);
        });

        // wait for the requests to be handled
        thread::sleep(Duration::from_secs(1));

        // check request are handled
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 2, "Backend should only have 2 ongoing requests.")
    }

    #[test]
    fn get_class_at_request_should_be_deduplicated() {
        // start a mock remote network
        start_tcp_server("127.0.0.1:8082".to_string());

        let handle = create_forked_backend("http://127.0.0.1:8082", 1);

        // check no pending requests
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 0, "Backend should not have any ongoing requests.");

        // send requests to the backend
        let h1 = handle.clone();
        thread::spawn(move || {
            h1.get_class_at(felt!("0x1")).expect(ERROR_SEND_REQUEST);
        });
        let h2 = handle.clone();
        thread::spawn(move || {
            h2.get_class_at(felt!("0x1")).expect(ERROR_SEND_REQUEST);
        });

        // check current request count
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 2, "Backend should have 1 ongoing requests.");

        // Different request, should be counted
        let h3 = handle.clone();
        thread::spawn(move || {
            h3.get_class_at(felt!("0x2")).expect(ERROR_SEND_REQUEST);
        });

        // wait for the requests to be handled
        thread::sleep(Duration::from_secs(1));

        // check request are handled
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 2, "Backend should only have 2 ongoing requests.")
    }

    #[test]
    fn get_compiled_class_hash_request_should_be_deduplicated() {
        // start a mock remote network
        start_tcp_server("127.0.0.1:8083".to_string());

        let handle = create_forked_backend("http://127.0.0.1:8083", 1);

        // check no pending requests
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 0, "Backend should not have any ongoing requests.");

        // send requests to the backend
        let h1 = handle.clone();
        thread::spawn(move || {
            h1.get_compiled_class_hash(felt!("0x1")).expect(ERROR_SEND_REQUEST);
        });
        let h2 = handle.clone();
        thread::spawn(move || {
            h2.get_compiled_class_hash(felt!("0x1")).expect(ERROR_SEND_REQUEST);
        });

        // check current request count
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 1, "Backend should have 1 ongoing requests.");

        // Different request, should be counted
        let h3 = handle.clone();
        thread::spawn(move || {
            h3.get_compiled_class_hash(felt!("0x2")).expect(ERROR_SEND_REQUEST);
        });

        // wait for the requests to be handled
        thread::sleep(Duration::from_secs(1));

        // check request are handled
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 2, "Backend should only have 2 ongoing requests.")
    }

    #[test]
    fn get_class_at_and_get_compiled_class_hash_request_should_be_deduplicated() {
        // start a mock remote network
        start_tcp_server("127.0.0.1:8084".to_string());

        let handle = create_forked_backend("http://127.0.0.1:8084", 1);

        // check no pending requests
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 0, "Backend should not have any ongoing requests.");

        // send requests to the backend
        let h1 = handle.clone();
        thread::spawn(move || {
            h1.get_class_at(felt!("0x1")).expect(ERROR_SEND_REQUEST);
        });
        // Since this also calls to the same request as the previous one, it should be deduped
        let h2 = handle.clone();
        thread::spawn(move || {
            h2.get_compiled_class_hash(felt!("0x1")).expect(ERROR_SEND_REQUEST);
        });

        // check current request count
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 1, "Backend should have 1 ongoing requests.");

        // Different request, should be counted
        let h3 = handle.clone();
        thread::spawn(move || {
            h3.get_class_at(felt!("0x2")).expect(ERROR_SEND_REQUEST);
        });

        // wait for the requests to be handled
        thread::sleep(Duration::from_secs(1));

        // check request are handled
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 1, "Backend should only have 2 ongoing requests.")
    }

    #[test]
    fn get_class_hash_at_request_should_be_deduplicated() {
        // start a mock remote network
        start_tcp_server("127.0.0.1:8085".to_string());

        let handle = create_forked_backend("http://127.0.0.1:8085", 1);

        // check no pending requests
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 0, "Backend should not have any ongoing requests.");

        // send requests to the backend
        let h1 = handle.clone();
        thread::spawn(move || {
            h1.get_class_hash_at(felt!("0x1").into()).expect(ERROR_SEND_REQUEST);
        });
        let h2 = handle.clone();
        thread::spawn(move || {
            h2.get_class_hash_at(felt!("0x1").into()).expect(ERROR_SEND_REQUEST);
        });

        // check current request count
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 1, "Backend should have 1 ongoing requests.");

        // Different request, should be counted
        let h3 = handle.clone();
        thread::spawn(move || {
            h3.get_class_hash_at(felt!("0x2").into()).expect(ERROR_SEND_REQUEST);
        });

        // wait for the requests to be handled
        thread::sleep(Duration::from_secs(1));

        // check request are handled
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 2, "Backend should only have 2 ongoing requests.")
    }

    #[test]
    fn get_storage_request_should_be_deduplicated() {
        // start a mock remote network
        start_tcp_server("127.0.0.1:8086".to_string());

        let handle = create_forked_backend("http://127.0.0.1:8086", 1);

        // check no pending requests
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 0, "Backend should not have any ongoing requests.");

        // send requests to the backend
        let h1 = handle.clone();
        thread::spawn(move || {
            h1.get_storage(felt!("0x1").into(), felt!("0x1")).expect(ERROR_SEND_REQUEST);
        });
        let h2 = handle.clone();
        thread::spawn(move || {
            h2.get_storage(felt!("0x1").into(), felt!("0x1")).expect(ERROR_SEND_REQUEST);
        });

        // check current request count
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 1, "Backend should have 1 ongoing requests.");

        // Different request, should be counted
        let h3 = handle.clone();
        thread::spawn(move || {
            h3.get_storage(felt!("0x2").into(), felt!("0x3")).expect(ERROR_SEND_REQUEST);
        });

        // wait for the requests to be handled
        thread::sleep(Duration::from_secs(1));

        // check request are handled
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 2, "Backend should only have 2 ongoing requests.")
    }

    #[test]
    fn get_storage_request_on_same_address_with_different_key_should_be_deduplicated() {
        // start a mock remote network
        start_tcp_server("127.0.0.1:8087".to_string());

        let handle = create_forked_backend("http://127.0.0.1:8087", 1);

        // check no pending requests
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 0, "Backend should not have any ongoing requests.");

        // send requests to the backend
        let h1 = handle.clone();
        thread::spawn(move || {
            h1.get_storage(felt!("0x1").into(), felt!("0x1")).expect(ERROR_SEND_REQUEST);
        });
        let h2 = handle.clone();
        thread::spawn(move || {
            h2.get_storage(felt!("0x1").into(), felt!("0x1")).expect(ERROR_SEND_REQUEST);
        });

        // check current request count
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 1, "Backend should have 1 ongoing requests.");

        // Different request, should be counted
        let h3 = handle.clone();
        thread::spawn(move || {
            h3.get_storage(felt!("0x1").into(), felt!("0x3")).expect(ERROR_SEND_REQUEST);
        });
        // Different request, should be counted
        let h4 = handle.clone();
        thread::spawn(move || {
            h4.get_storage(felt!("0x1").into(), felt!("0x6")).expect(ERROR_SEND_REQUEST);
        });

        // check current request count
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 3, "Backend should have 3 ongoing requests.");

        // Same request as the last one, shouldn't be counted
        let h5 = handle.clone();
        thread::spawn(move || {
            h5.get_storage(felt!("0x1").into(), felt!("0x6")).expect(ERROR_SEND_REQUEST);
        });

        // wait for the requests to be handled
        thread::sleep(Duration::from_secs(1));

        // check request are handled
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 3, "Backend should only have 3 ongoing requests.")
    }

    #[test]
    fn get_from_cache_if_exist() {
        // setup
        let backend = create_forked_backend(LOCAL_RPC_URL, 1);
        let state_db = CacheStateDb::new(backend);

        state_db
            .storage
            .write()
            .entry(ADDR_1)
            .or_default()
            .insert(STORAGE_KEY, ADDR_1_STORAGE_VALUE);

        state_db.contract_state.write().insert(
            ADDR_1,
            GenericContractInfo { nonce: ADDR_1_NONCE, class_hash: ADDR_1_CLASS_HASH },
        );

        let provider = SharedStateProvider(Arc::new(state_db));

        assert_eq!(StateProvider::nonce(&provider, ADDR_1).unwrap(), Some(ADDR_1_NONCE));
        assert_eq!(
            StateProvider::storage(&provider, ADDR_1, STORAGE_KEY).unwrap(),
            Some(ADDR_1_STORAGE_VALUE)
        );
        assert_eq!(
            StateProvider::class_hash_of_contract(&provider, ADDR_1).unwrap(),
            Some(ADDR_1_CLASS_HASH)
        );
    }

    #[test]
    fn test_deduplicated_request_should_return_similar_results() {
        // Start mock server with a predefined nonce response
        let response = r#"{"jsonrpc":"2.0","result":"0x123","id":1}"#;
        let sender = start_mock_rpc_server("127.0.0.1:8090".to_string(), response.to_string());

        let handle = create_forked_backend("http://127.0.0.1:8090", 1);
        let addr = ContractAddress(felt!("0x1"));

        // Collect results from multiple identical nonce requests
        let results: Arc<Mutex<Vec<Result<Nonce, BackendError>>>> =
            Arc::new(Mutex::new(Vec::new()));
        let handles: Vec<_> = (0..5)
            .map(|_| {
                let h = handle.clone();
                let results = results.clone();
                thread::spawn(move || {
                    let res = h.get_nonce(addr);
                    results.lock().unwrap().push(res);
                })
            })
            .collect();

        // wait for the requests to be sent to the rpc server
        thread::sleep(Duration::from_secs(1));

        // Check that there's only one request, meaning it is deduplicated.
        let stats = handle.stats().expect(ERROR_STATS);
        assert_eq!(stats, 1, "Backend should only have 1 ongoing requests.");

        // Send the signal to tell the mock rpc server to return the response
        sender.send(()).unwrap();

        // Join all request threads
        handles.into_iter().for_each(|h| h.join().unwrap());

        // Verify all results are identical
        let results = results.lock().unwrap();
        for result in results.iter() {
            assert_eq!(
                "0x123",
                format!("{:#x}", result.as_ref().unwrap()),
                "All deduplicated nonce requests should return the same result"
            );
        }
    }

    // TODO: unignore this once we have separate the spawning of the backend thread from the backend
    // creation
    #[test]
    #[ignore]
    fn fetch_from_fork_will_err_if_backend_thread_not_running() {
        let backend = create_forked_backend(LOCAL_RPC_URL, 1);
        let provider = SharedStateProvider(Arc::new(CacheStateDb::new(backend)));
        assert!(StateProvider::nonce(&provider, ADDR_1).is_err())
    }

    const FORKED_URL: &str =
        "https://starknet-goerli.infura.io/v3/369ce5ac40614952af936e4d64e40474";

    const GOERLI_CONTRACT_ADDR: ContractAddress = ContractAddress(felt!(
        "0x02b92ec12cA1e308f320e99364d4dd8fcc9efDAc574F836C8908de937C289974"
    ));
    const GOERLI_CONTRACT_STORAGE_KEY: StorageKey =
        felt!("0x3b459c3fadecdb1a501f2fdeec06fd735cb2d93ea59779177a0981660a85352");

    #[test]
    #[ignore]
    fn fetch_from_fork_if_not_in_cache() {
        let backend = create_forked_backend(FORKED_URL, 908622);
        let provider = SharedStateProvider(Arc::new(CacheStateDb::new(backend)));

        // fetch from remote

        let class_hash =
            StateProvider::class_hash_of_contract(&provider, GOERLI_CONTRACT_ADDR).unwrap();
        let storage_value =
            StateProvider::storage(&provider, GOERLI_CONTRACT_ADDR, GOERLI_CONTRACT_STORAGE_KEY)
                .unwrap();
        let nonce = StateProvider::nonce(&provider, GOERLI_CONTRACT_ADDR).unwrap();

        // fetch from cache

        let class_hash_in_cache =
            provider.0.contract_state.read().get(&GOERLI_CONTRACT_ADDR).map(|i| i.class_hash);
        let storage_value_in_cache = provider
            .0
            .storage
            .read()
            .get(&GOERLI_CONTRACT_ADDR)
            .and_then(|s| s.get(&GOERLI_CONTRACT_STORAGE_KEY))
            .copied();
        let nonce_in_cache =
            provider.0.contract_state.read().get(&GOERLI_CONTRACT_ADDR).map(|i| i.nonce);

        // check

        assert_eq!(nonce, nonce_in_cache, "value must be stored in cache");
        assert_eq!(class_hash, class_hash_in_cache, "value must be stored in cache");
        assert_eq!(storage_value, storage_value_in_cache, "value must be stored in cache");
    }
}
