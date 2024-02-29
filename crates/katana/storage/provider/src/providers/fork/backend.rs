use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::mpsc::{channel as oneshot, RecvError, Sender as OneshotSender};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::thread;

use futures::channel::mpsc::{channel, Receiver, SendError, Sender};
use futures::future::BoxFuture;
use futures::stream::Stream;
use futures::{Future, FutureExt};
use katana_primitives::block::BlockHashOrNumber;
use katana_primitives::contract::{
    ClassHash, CompiledClass, CompiledClassHash, ContractAddress, FlattenedSierraClass,
    GenericContractInfo, Nonce, StorageKey, StorageValue,
};
use katana_primitives::conversion::rpc::{
    compiled_class_hash_from_flattened_sierra_class, flattened_sierra_to_compiled_class,
    legacy_rpc_to_compiled_class,
};
use katana_primitives::FieldElement;
use parking_lot::Mutex;
use starknet::core::types::{BlockId, ContractClass, StarknetError};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider, ProviderError as StarknetProviderError};
use tracing::{error, trace};

use crate::error::ProviderError;
use crate::providers::in_memory::cache::CacheStateDb;
use crate::traits::contract::{ContractClassProvider, ContractInfoProvider};
use crate::traits::state::StateProvider;
use crate::ProviderResult;

type GetNonceResult = Result<Nonce, ForkedBackendError>;
type GetStorageResult = Result<StorageValue, ForkedBackendError>;
type GetClassHashAtResult = Result<ClassHash, ForkedBackendError>;
type GetClassAtResult = Result<starknet::core::types::ContractClass, ForkedBackendError>;

#[derive(Debug, thiserror::Error)]
pub enum ForkedBackendError {
    #[error("Failed to send request to the forked backend: {0}")]
    Send(#[from] SendError),
    #[error("Failed to receive result from the forked backend: {0}")]
    Receive(#[from] RecvError),
    #[error("Compute class hash error: {0}")]
    ComputeClassHashError(String),
    #[error("Failed to spawn forked backend thread: {0}")]
    BackendThreadInit(#[from] std::io::Error),
    #[error(transparent)]
    StarknetProvider(#[from] starknet::providers::ProviderError),
}

/// The request types that is processed by [`Backend`].
///
/// Each request is accompanied by the sender-half of a oneshot channel that will be used
/// to send the [`ProviderResult`] back to the backend client, [`ForkedBackend`], which sent the
/// requests.
pub enum BackendRequest {
    GetClassAt(ClassHash, OneshotSender<GetClassAtResult>),
    GetNonce(ContractAddress, OneshotSender<GetNonceResult>),
    GetClassHashAt(ContractAddress, OneshotSender<GetClassHashAtResult>),
    GetStorage(ContractAddress, StorageKey, OneshotSender<GetStorageResult>),
}

type BackendRequestFuture = BoxFuture<'static, ()>;

/// The backend for the forked provider. It processes all requests from the [ForkedBackend]'s
/// and sends the ProviderResults back to it.
///
/// It is responsible it fetching the data from the forked provider.
pub struct Backend {
    provider: Arc<JsonRpcClient<HttpTransport>>,
    /// Requests that are currently being poll.
    pending_requests: Vec<BackendRequestFuture>,
    /// Requests that are queued to be polled.
    queued_requests: VecDeque<BackendRequest>,
    /// A channel for receiving requests from the [ForkedBackend]'s.
    incoming: Receiver<BackendRequest>,
    /// Pinned block id for all requests.
    block: BlockId,
}

impl Backend {
    /// This function is responsible for transforming the incoming request
    /// into a future that will be polled until completion by the `BackendHandler`.
    ///
    /// Each request is accompanied by the sender-half of a oneshot channel that will be used
    /// to send the ProviderResult back to the [ForkedBackend] which sent the requests.
    fn handle_requests(&mut self, request: BackendRequest) {
        let block = self.block;
        let provider = self.provider.clone();

        match request {
            BackendRequest::GetNonce(contract_address, sender) => {
                let fut = Box::pin(async move {
                    let res = provider
                        .get_nonce(block, Into::<FieldElement>::into(contract_address))
                        .await
                        .map_err(ForkedBackendError::StarknetProvider);

                    sender.send(res).expect("failed to send nonce result")
                });

                self.pending_requests.push(fut);
            }

            BackendRequest::GetStorage(contract_address, key, sender) => {
                let fut = Box::pin(async move {
                    let res = provider
                        .get_storage_at(Into::<FieldElement>::into(contract_address), key, block)
                        .await
                        .map_err(ForkedBackendError::StarknetProvider);

                    sender.send(res).expect("failed to send storage result")
                });

                self.pending_requests.push(fut);
            }

            BackendRequest::GetClassHashAt(contract_address, sender) => {
                let fut = Box::pin(async move {
                    let res = provider
                        .get_class_hash_at(block, Into::<FieldElement>::into(contract_address))
                        .await
                        .map_err(ForkedBackendError::StarknetProvider);

                    sender.send(res).expect("failed to send class hash result")
                });

                self.pending_requests.push(fut);
            }

            BackendRequest::GetClassAt(class_hash, sender) => {
                let fut = Box::pin(async move {
                    let res = provider
                        .get_class(block, class_hash)
                        .await
                        .map_err(ForkedBackendError::StarknetProvider);

                    sender.send(res).expect("failed to send class result")
                });

                self.pending_requests.push(fut);
            }
        }
    }
}

impl Future for Backend {
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
                let mut fut = pin.pending_requests.swap_remove(n);
                // poll the future and if the future is still pending, push it back to the
                // pending requests so that it will be polled again
                if fut.poll_unpin(cx).is_pending() {
                    pin.pending_requests.push(fut);
                }
            }

            // if no queued requests, then yield
            if pin.queued_requests.is_empty() {
                return Poll::Pending;
            }
        }
    }
}

/// A thread safe handler to the [`Backend`]. This is the primary interface for sending
/// request to the backend thread to fetch data from the forked provider.
pub struct ForkedBackend(Mutex<Sender<BackendRequest>>);

impl Clone for ForkedBackend {
    fn clone(&self) -> Self {
        Self(Mutex::new(self.0.lock().clone()))
    }
}

impl ForkedBackend {
    /// Create a new [`ForkedBackend`] with a dedicated backend thread.
    ///
    /// This method will spawn a new thread that will run the [`Backend`].
    pub fn new_with_backend_thread(
        provider: Arc<JsonRpcClient<HttpTransport>>,
        block_id: BlockHashOrNumber,
    ) -> Result<Self, ForkedBackendError> {
        let (handler, backend) = Self::new(provider, block_id);

        thread::Builder::new().spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create tokio runtime")
                .block_on(backend);
        })?;

        trace!(target: "forked_backend", "fork backend thread spawned");

        Ok(handler)
    }

    fn new(
        provider: Arc<JsonRpcClient<HttpTransport>>,
        block_id: BlockHashOrNumber,
    ) -> (Self, Backend) {
        let block = match block_id {
            BlockHashOrNumber::Hash(hash) => BlockId::Hash(hash),
            BlockHashOrNumber::Num(number) => BlockId::Number(number),
        };

        let (sender, rx) = channel(1);
        let backend = Backend {
            incoming: rx,
            provider,
            block,
            queued_requests: VecDeque::new(),
            pending_requests: Vec::new(),
        };

        (Self(Mutex::new(sender)), backend)
    }

    pub fn do_get_nonce(
        &self,
        contract_address: ContractAddress,
    ) -> Result<Nonce, ForkedBackendError> {
        trace!(target: "forked_backend", "requesting nonce for contract address {contract_address}");
        let (sender, rx) = oneshot();
        self.0
            .lock()
            .try_send(BackendRequest::GetNonce(contract_address, sender))
            .map_err(|e| e.into_send_error())?;
        rx.recv()?
    }

    pub fn do_get_storage(
        &self,
        contract_address: ContractAddress,
        key: StorageKey,
    ) -> Result<StorageValue, ForkedBackendError> {
        trace!(target: "forked_backend", "requesting storage for address {contract_address} at key {key:#x}" );
        let (sender, rx) = oneshot();
        self.0
            .lock()
            .try_send(BackendRequest::GetStorage(contract_address, key, sender))
            .map_err(|e| e.into_send_error())?;
        rx.recv()?
    }

    pub fn do_get_class_hash_at(
        &self,
        contract_address: ContractAddress,
    ) -> Result<ClassHash, ForkedBackendError> {
        trace!(target: "forked_backend", "requesting class hash at address {contract_address}");
        let (sender, rx) = oneshot();
        self.0
            .lock()
            .try_send(BackendRequest::GetClassHashAt(contract_address, sender))
            .map_err(|e| e.into_send_error())?;
        rx.recv()?
    }

    pub fn do_get_class_at(
        &self,
        class_hash: ClassHash,
    ) -> Result<starknet::core::types::ContractClass, ForkedBackendError> {
        trace!(target: "forked_backend", "requesting class at hash {class_hash:#x}");
        let (sender, rx) = oneshot();
        self.0
            .lock()
            .try_send(BackendRequest::GetClassAt(class_hash, sender))
            .map_err(|e| e.into_send_error())?;
        rx.recv()?
    }

    pub fn do_get_compiled_class_hash(
        &self,
        class_hash: ClassHash,
    ) -> Result<CompiledClassHash, ForkedBackendError> {
        trace!(target: "forked_backend", "requesting compiled class hash at class {class_hash:#x}");
        let class = self.do_get_class_at(class_hash)?;
        // if its a legacy class, then we just return back the class hash
        // else if sierra class, then we have to compile it and compute the compiled class hash.
        match class {
            starknet::core::types::ContractClass::Legacy(_) => Ok(class_hash),
            starknet::core::types::ContractClass::Sierra(sierra_class) => {
                compiled_class_hash_from_flattened_sierra_class(&sierra_class)
                    .map_err(|e| ForkedBackendError::ComputeClassHashError(e.to_string()))
            }
        }
    }
}

/// A shared cache that stores data fetched from the forked network.
///
/// Check in cache first, if not found, then fetch from the forked provider and store it in the
/// cache to avoid fetching it again. This is shared across multiple instances of
/// [`ForkedStateDb`](super::state::ForkedStateDb).
#[derive(Clone)]
pub struct SharedStateProvider(Arc<CacheStateDb<ForkedBackend>>);

impl SharedStateProvider {
    pub(crate) fn new_with_backend(backend: ForkedBackend) -> Self {
        Self(Arc::new(CacheStateDb::new(backend)))
    }
}

impl ContractInfoProvider for SharedStateProvider {
    fn contract(&self, address: ContractAddress) -> ProviderResult<Option<GenericContractInfo>> {
        let info = self.0.contract_state.read().get(&address).cloned();
        Ok(info)
    }
}

impl StateProvider for SharedStateProvider {
    fn nonce(&self, address: ContractAddress) -> ProviderResult<Option<Nonce>> {
        if let nonce @ Some(_) = self.contract(address)?.map(|i| i.nonce) {
            return Ok(nonce);
        }

        if let Some(nonce) = handle_contract_or_class_not_found_err(self.0.do_get_nonce(address)).map_err(|e| {
            error!(target: "forked_backend", "error while fetching nonce of contract {address}: {e}");
            e
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

        let value = handle_contract_or_class_not_found_err(self.0.do_get_storage(address, storage_key)).map_err(|e| {
            error!(target: "forked_backend", "error while fetching storage value of contract {address} at key {storage_key:#x}: {e}");
            e
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
        if let hash @ Some(_) = self.contract(address)?.map(|i| i.class_hash) {
            return Ok(hash);
        }

        if let Some(hash) = handle_contract_or_class_not_found_err(self.0.do_get_class_hash_at(address)).map_err(|e| {
            error!(target: "forked_backend", "error while fetching class hash of contract {address}: {e}");
            e
        })? {
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

        let Some(class) = handle_contract_or_class_not_found_err(self.0.do_get_class_at(hash))
            .map_err(|e| {
                error!(target: "forked_backend", "error while fetching sierra class {hash:#x}: {e}");
                e
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
            handle_contract_or_class_not_found_err(self.0.do_get_compiled_class_hash(hash))
                .map_err(|e| {
                    error!(target: "forked_backend", "error while fetching compiled class hash for class hash {hash:#x}: {e}");
                    e
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

        let Some(class) = handle_contract_or_class_not_found_err(self.0.do_get_class_at(hash))
            .map_err(|e| {
                error!(target: "forked_backend", "error while fetching class {hash:#x}: {e}");
                e
            })?
        else {
            return Ok(None);
        };

        let (class_hash, compiled_class_hash, casm, sierra) = match class {
            ContractClass::Legacy(class) => {
                let (_, compiled_class) = legacy_rpc_to_compiled_class(&class).map_err(|e| {
                    error!(target: "forked_backend", "error while parsing legacy class {hash:#x}: {e}");
                    ProviderError::ParsingError(e.to_string())
                })?;

                (hash, hash, compiled_class, None)
            }

            ContractClass::Sierra(sierra_class) => {
                let (_, compiled_class_hash, compiled_class) = flattened_sierra_to_compiled_class(&sierra_class).map_err(|e|{
                    error!(target: "forked_backend", "error while parsing sierra class {hash:#x}: {e}");
                    ProviderError::ParsingError(e.to_string())
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

fn handle_contract_or_class_not_found_err<T>(
    result: Result<T, ForkedBackendError>,
) -> Result<Option<T>, ForkedBackendError> {
    match result {
        Ok(value) => Ok(Some(value)),

        Err(ForkedBackendError::StarknetProvider(StarknetProviderError::StarknetError(
            StarknetError::ContractNotFound | StarknetError::ClassHashNotFound,
        ))) => Ok(None),

        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use katana_primitives::block::BlockNumber;
    use katana_primitives::contract::GenericContractInfo;
    use starknet::macros::felt;
    use url::Url;

    use super::*;

    const LOCAL_RPC_URL: &str = "http://localhost:5050";

    const STORAGE_KEY: StorageKey = felt!("0x1");
    const ADDR_1: ContractAddress = ContractAddress(felt!("0xADD1"));
    const ADDR_1_NONCE: Nonce = felt!("0x1");
    const ADDR_1_STORAGE_VALUE: StorageKey = felt!("0x8080");
    const ADDR_1_CLASS_HASH: StorageKey = felt!("0x1");

    fn create_forked_backend(rpc_url: String, block_num: BlockNumber) -> (ForkedBackend, Backend) {
        ForkedBackend::new(
            Arc::new(JsonRpcClient::new(HttpTransport::new(
                Url::parse(&rpc_url).expect("valid url"),
            ))),
            BlockHashOrNumber::Num(block_num),
        )
    }

    fn create_forked_backend_with_backend_thread(
        rpc_url: String,
        block_num: BlockNumber,
    ) -> ForkedBackend {
        ForkedBackend::new_with_backend_thread(
            Arc::new(JsonRpcClient::new(HttpTransport::new(
                Url::parse(&rpc_url).expect("valid url"),
            ))),
            BlockHashOrNumber::Num(block_num),
        )
        .unwrap()
    }

    #[test]
    fn get_from_cache_if_exist() {
        // setup
        let (backend, _) = create_forked_backend(LOCAL_RPC_URL.into(), 1);
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
    fn fetch_from_fork_will_err_if_backend_thread_not_running() {
        let (backend, _) = create_forked_backend(LOCAL_RPC_URL.into(), 1);
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
        let backend = create_forked_backend_with_backend_thread(FORKED_URL.into(), 908622);
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
