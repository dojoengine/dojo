use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::mpsc::{channel as oneshot, Sender as OneshotSender};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::thread;

use blockifier::execution::contract_class::ContractClass;
use blockifier::state::errors::StateError;
use blockifier::state::state_api::{StateReader, StateResult};
use futures::channel::mpsc::{channel, Receiver, Sender, TrySendError};
use futures::stream::Stream;
use futures::{Future, FutureExt};
use parking_lot::RwLock;
use starknet::core::types::{BlockId, FieldElement, FlattenedSierraClass, StarknetError};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{
    JsonRpcClient, MaybeUnknownErrorCode, Provider, ProviderError, StarknetErrorWithMessage,
};
use starknet_api::core::{ClassHash, CompiledClassHash, ContractAddress, Nonce};
use starknet_api::hash::StarkFelt;
use starknet_api::state::StorageKey;
use tracing::trace;

use crate::db::cached::CachedDb;
use crate::db::StateExtRef;
use crate::utils::contract::{
    compiled_class_hash_from_flattened_sierra_class, legacy_rpc_to_inner_class, rpc_to_inner_class,
};

type GetNonceResult = Result<Nonce, ForkedBackendError>;
type GetStorageResult = Result<StarkFelt, ForkedBackendError>;
type GetClassHashAtResult = Result<ClassHash, ForkedBackendError>;
type GetClassAtResult = Result<starknet::core::types::ContractClass, ForkedBackendError>;

#[derive(Debug, thiserror::Error)]
pub enum ForkedBackendError {
    #[error(transparent)]
    Send(TrySendError<BackendRequest>),
    #[error("Compute class hash error: {0}")]
    ComputeClassHashError(String),
    #[error(transparent)]
    Provider(ProviderError<<JsonRpcClient<HttpTransport> as Provider>::Error>),
}

pub enum BackendRequest {
    GetClassAt(ClassHash, OneshotSender<GetClassAtResult>),
    GetNonce(ContractAddress, OneshotSender<GetNonceResult>),
    GetClassHashAt(ContractAddress, OneshotSender<GetClassHashAtResult>),
    GetStorage(ContractAddress, StorageKey, OneshotSender<GetStorageResult>),
}

type BackendRequestFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

/// A thread-safe handler for the shared forked backend. This handler is responsible for receiving
/// requests from all instances of the [ForkedBackend], process them, and returns the results back
/// to the request sender.
pub struct BackendHandler {
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

impl BackendHandler {
    /// This function is responsible for transforming the incoming request
    /// into a future that will be polled until completion by the `BackendHandler`.
    ///
    /// Each request is accompanied by the sender-half of a oneshot channel that will be used
    /// to send the result back to the [ForkedBackend] which sent the requests.
    fn handle_requests(&mut self, request: BackendRequest) {
        let block = self.block;
        let provider = self.provider.clone();

        match request {
            BackendRequest::GetNonce(contract_address, sender) => {
                let fut = Box::pin(async move {
                    let contract_address: FieldElement = (*contract_address.0.key()).into();

                    let res = provider
                        .get_nonce(block, contract_address)
                        .await
                        .map(|n| Nonce(n.into()))
                        .map_err(ForkedBackendError::Provider);

                    sender.send(res).expect("failed to send nonce result")
                });

                self.pending_requests.push(fut);
            }

            BackendRequest::GetStorage(contract_address, key, sender) => {
                let fut = Box::pin(async move {
                    let contract_address: FieldElement = (*contract_address.0.key()).into();
                    let key: FieldElement = (*key.0.key()).into();

                    let res = provider
                        .get_storage_at(contract_address, key, block)
                        .await
                        .map(|f| f.into())
                        .map_err(ForkedBackendError::Provider);

                    sender.send(res).expect("failed to send storage result")
                });

                self.pending_requests.push(fut);
            }

            BackendRequest::GetClassHashAt(contract_address, sender) => {
                let fut = Box::pin(async move {
                    let contract_address: FieldElement = (*contract_address.0.key()).into();

                    let res = provider
                        .get_class_hash_at(block, contract_address)
                        .await
                        .map(|f| ClassHash(f.into()))
                        .map_err(ForkedBackendError::Provider);

                    sender.send(res).expect("failed to send class hash result")
                });

                self.pending_requests.push(fut);
            }

            BackendRequest::GetClassAt(class_hash, sender) => {
                let fut = Box::pin(async move {
                    let class_hash: FieldElement = class_hash.0.into();

                    let res = provider
                        .get_class(block, class_hash)
                        .await
                        .map_err(ForkedBackendError::Provider);

                    sender.send(res).expect("failed to send class result")
                });

                self.pending_requests.push(fut);
            }
        }
    }
}

impl Future for BackendHandler {
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

#[derive(Debug, Clone)]
pub struct SharedBackend {
    cache: Arc<RwLock<CachedDb<ForkedBackend>>>,
}

impl SharedBackend {
    pub fn new_with_backend_thread(
        provider: Arc<JsonRpcClient<HttpTransport>>,
        block: BlockId,
    ) -> Self {
        let backend = ForkedBackend::spawn_thread(provider, block);
        Self { cache: Arc::new(RwLock::new(CachedDb::new(backend))) }
    }
}

impl StateReader for SharedBackend {
    fn get_class_hash_at(&mut self, contract_address: ContractAddress) -> StateResult<ClassHash> {
        self.cache.write().get_class_hash_at(contract_address)
    }

    fn get_compiled_class_hash(&mut self, class_hash: ClassHash) -> StateResult<CompiledClassHash> {
        self.cache.write().get_compiled_class_hash(class_hash)
    }

    fn get_compiled_contract_class(
        &mut self,
        class_hash: &ClassHash,
    ) -> StateResult<ContractClass> {
        self.cache.write().get_compiled_contract_class(class_hash)
    }

    fn get_nonce_at(&mut self, contract_address: ContractAddress) -> StateResult<Nonce> {
        self.cache.write().get_nonce_at(contract_address)
    }

    fn get_storage_at(
        &mut self,
        contract_address: ContractAddress,
        key: StorageKey,
    ) -> StateResult<StarkFelt> {
        self.cache.write().get_storage_at(contract_address, key)
    }
}

impl StateExtRef for SharedBackend {
    fn get_sierra_class(&mut self, class_hash: &ClassHash) -> StateResult<FlattenedSierraClass> {
        self.cache.write().get_sierra_class(class_hash)
    }
}

/// An interface for interacting with a forked backend handler. This interface will be cloned into
/// multiple instances of the [ForkedBackend] and will be used to send requests to the handler.
#[derive(Debug, Clone)]
pub struct ForkedBackend {
    handler: Sender<BackendRequest>,
}

impl ForkedBackend {
    pub fn spawn_thread(provider: Arc<JsonRpcClient<HttpTransport>>, block: BlockId) -> Self {
        let (backend, handler) = Self::new(provider, block);

        thread::Builder::new()
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to create fork backend thread tokio runtime");

                rt.block_on(handler);
            })
            .expect("failed to spawn fork backend thread");

        trace!(target: "forked_backend", "fork backend thread spawned");

        backend
    }

    pub fn new(
        provider: Arc<JsonRpcClient<HttpTransport>>,
        block: BlockId,
    ) -> (Self, BackendHandler) {
        let (sender, rx) = channel(1);
        let handler = BackendHandler {
            incoming: rx,
            provider,
            block,
            queued_requests: VecDeque::new(),
            pending_requests: Vec::new(),
        };
        (Self { handler: sender }, handler)
    }

    pub fn do_get_nonce(
        &mut self,
        contract_address: ContractAddress,
    ) -> Result<Nonce, ForkedBackendError> {
        trace!(target: "forked_backend", "request nonce for contract address {}", contract_address.0.key());
        tokio::task::block_in_place(|| {
            let (sender, rx) = oneshot();
            self.handler
                .try_send(BackendRequest::GetNonce(contract_address, sender))
                .map_err(ForkedBackendError::Send)?;
            rx.recv().expect("failed to receive nonce result")
        })
    }

    pub fn do_get_storage(
        &mut self,
        contract_address: ContractAddress,
        key: StorageKey,
    ) -> Result<StarkFelt, ForkedBackendError> {
        trace!(target: "forked_backend", "request storage for address {} at key {}", contract_address.0.key(), key.0.key());
        tokio::task::block_in_place(|| {
            let (sender, rx) = oneshot();
            self.handler
                .try_send(BackendRequest::GetStorage(contract_address, key, sender))
                .map_err(ForkedBackendError::Send)?;
            rx.recv().expect("failed to receive storage result")
        })
    }

    pub fn do_get_class_hash_at(
        &mut self,
        contract_address: ContractAddress,
    ) -> Result<ClassHash, ForkedBackendError> {
        trace!(target: "forked_backend", "request class hash at address {}", contract_address.0.key());
        tokio::task::block_in_place(|| {
            let (sender, rx) = oneshot();
            self.handler
                .try_send(BackendRequest::GetClassHashAt(contract_address, sender))
                .map_err(ForkedBackendError::Send)?;
            rx.recv().expect("failed to receive class hash result")
        })
    }

    pub fn do_get_class_at(
        &mut self,
        class_hash: ClassHash,
    ) -> Result<starknet::core::types::ContractClass, ForkedBackendError> {
        trace!(target: "forked_backend", "request class at hash {}", class_hash.0);
        tokio::task::block_in_place(|| {
            let (sender, rx) = oneshot();
            self.handler
                .try_send(BackendRequest::GetClassAt(class_hash, sender))
                .map_err(ForkedBackendError::Send)?;
            rx.recv().expect("failed to receive class result")
        })
    }

    pub fn do_get_compiled_class_hash(
        &mut self,
        class_hash: ClassHash,
    ) -> Result<CompiledClassHash, ForkedBackendError> {
        trace!(target: "forked_backend", "request compiled class hash at class {}", class_hash.0);
        let class = self.do_get_class_at(class_hash)?;
        // if its a legacy class, then we just return back the class hash
        // else if sierra class, then we have to compile it and compute the compiled class hash.
        match class {
            starknet::core::types::ContractClass::Legacy(_) => Ok(CompiledClassHash(class_hash.0)),

            starknet::core::types::ContractClass::Sierra(sierra_class) => {
                tokio::task::block_in_place(|| {
                    compiled_class_hash_from_flattened_sierra_class(&sierra_class)
                })
                .map(|f| CompiledClassHash(f.into()))
                .map_err(|e| ForkedBackendError::ComputeClassHashError(e.to_string()))
            }
        }
    }
}

impl StateReader for ForkedBackend {
    fn get_compiled_class_hash(&mut self, class_hash: ClassHash) -> StateResult<CompiledClassHash> {
        match self.do_get_compiled_class_hash(class_hash) {
            Ok(compiled_class_hash) => Ok(compiled_class_hash),

            Err(ForkedBackendError::Provider(ProviderError::StarknetError(
                StarknetErrorWithMessage {
                    code: MaybeUnknownErrorCode::Known(StarknetError::ClassHashNotFound),
                    ..
                },
            ))) => Err(StateError::UndeclaredClassHash(class_hash)),
            Err(e) => Err(StateError::StateReadError(e.to_string())),
        }
    }

    fn get_compiled_contract_class(
        &mut self,
        class_hash: &ClassHash,
    ) -> StateResult<ContractClass> {
        match self.do_get_class_at(*class_hash) {
            Ok(class) => match class {
                starknet::core::types::ContractClass::Legacy(legacy_class) => {
                    legacy_rpc_to_inner_class(&legacy_class)
                        .map(|(_, class)| class)
                        .map_err(|e| StateError::StateReadError(e.to_string()))
                }

                starknet::core::types::ContractClass::Sierra(sierra_class) => {
                    rpc_to_inner_class(&sierra_class)
                        .map(|(_, class)| class)
                        .map_err(|e| StateError::StateReadError(e.to_string()))
                }
            },

            Err(ForkedBackendError::Provider(ProviderError::StarknetError(
                StarknetErrorWithMessage {
                    code: MaybeUnknownErrorCode::Known(StarknetError::ClassHashNotFound),
                    ..
                },
            ))) => Err(StateError::UndeclaredClassHash(*class_hash)),

            Err(e) => Err(StateError::StateReadError(e.to_string())),
        }
    }

    fn get_storage_at(
        &mut self,
        contract_address: ContractAddress,
        key: StorageKey,
    ) -> StateResult<StarkFelt> {
        match self.do_get_storage(contract_address, key) {
            Ok(value) => Ok(value),

            Err(ForkedBackendError::Provider(ProviderError::StarknetError(
                StarknetErrorWithMessage {
                    code: MaybeUnknownErrorCode::Known(StarknetError::ContractNotFound),
                    ..
                },
            ))) => Ok(StarkFelt::default()),

            Err(e) => Err(StateError::StateReadError(e.to_string())),
        }
    }

    fn get_nonce_at(&mut self, contract_address: ContractAddress) -> StateResult<Nonce> {
        match self.do_get_nonce(contract_address) {
            Ok(nonce) => Ok(nonce),

            Err(ForkedBackendError::Provider(ProviderError::StarknetError(
                StarknetErrorWithMessage {
                    code: MaybeUnknownErrorCode::Known(StarknetError::ContractNotFound),
                    ..
                },
            ))) => Ok(Nonce::default()),

            Err(e) => Err(StateError::StateReadError(e.to_string())),
        }
    }

    fn get_class_hash_at(&mut self, contract_address: ContractAddress) -> StateResult<ClassHash> {
        match self.do_get_class_hash_at(contract_address) {
            Ok(class_hash) => Ok(class_hash),

            Err(ForkedBackendError::Provider(ProviderError::StarknetError(
                StarknetErrorWithMessage {
                    code: MaybeUnknownErrorCode::Known(StarknetError::ContractNotFound),
                    ..
                },
            ))) => Ok(ClassHash::default()),

            Err(e) => Err(StateError::StateReadError(e.to_string())),
        }
    }
}

impl StateExtRef for ForkedBackend {
    fn get_sierra_class(&mut self, class_hash: &ClassHash) -> StateResult<FlattenedSierraClass> {
        match self.do_get_class_at(*class_hash) {
            Ok(starknet::core::types::ContractClass::Sierra(sierra_class)) => Ok(sierra_class),

            Ok(_) => Err(StateError::StateReadError("Class hash is not a Sierra class".into())),

            Err(ForkedBackendError::Provider(ProviderError::StarknetError(
                StarknetErrorWithMessage {
                    code: MaybeUnknownErrorCode::Known(StarknetError::ClassHashNotFound),
                    ..
                },
            ))) => Err(StateError::UndeclaredClassHash(*class_hash)),

            Err(e) => Err(StateError::StateReadError(e.to_string())),
        }
    }
}
