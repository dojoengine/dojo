use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::mpsc::{channel as oneshot, Sender as OneshotSender};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::thread;

use anyhow::Result;
use futures::channel::mpsc::{channel, Receiver, Sender, TrySendError};
use futures::future::BoxFuture;
use futures::stream::Stream;
use futures::{Future, FutureExt};
use katana_primitives::block::BlockHashOrNumber;
use katana_primitives::contract::{
    ClassHash, CompiledClassHash, ContractAddress, Nonce, StorageKey, StorageValue,
};
use katana_primitives::conversion::rpc::{
    compiled_class_hash_from_flattened_sierra_class, legacy_rpc_to_inner_class, rpc_to_inner_class,
};
use katana_primitives::FieldElement;
use parking_lot::Mutex;
use starknet::core::types::BlockId;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider, ProviderError};
use tracing::trace;

use crate::traits::state::{StateProvider, StateProviderExt};

type GetNonceResult = Result<Nonce, ForkedBackendError>;
type GetStorageResult = Result<StorageValue, ForkedBackendError>;
type GetClassHashAtResult = Result<ClassHash, ForkedBackendError>;
type GetClassAtResult = Result<starknet::core::types::ContractClass, ForkedBackendError>;

#[derive(Debug, thiserror::Error)]
pub enum ForkedBackendError {
    #[error(transparent)]
    Send(TrySendError<BackendRequest>),
    #[error("Compute class hash error: {0}")]
    ComputeClassHashError(String),
    #[error(transparent)]
    Provider(ProviderError),
}

pub enum BackendRequest {
    GetClassAt(ClassHash, OneshotSender<GetClassAtResult>),
    GetNonce(ContractAddress, OneshotSender<GetNonceResult>),
    GetClassHashAt(ContractAddress, OneshotSender<GetClassHashAtResult>),
    GetStorage(ContractAddress, StorageKey, OneshotSender<GetStorageResult>),
}

type BackendRequestFuture = BoxFuture<'static, ()>;

/// A thread-safe handler for the shared forked backend. This handler is responsible for receiving
/// requests from all instances of the [ForkedBackend], process them, and returns the results back
/// to the request sender.
pub struct ForkedBackend {
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

impl ForkedBackend {
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
                    let res = provider
                        .get_nonce(block, Into::<FieldElement>::into(contract_address))
                        .await
                        .map_err(ForkedBackendError::Provider);

                    sender.send(res).expect("failed to send nonce result")
                });

                self.pending_requests.push(fut);
            }

            BackendRequest::GetStorage(contract_address, key, sender) => {
                let fut = Box::pin(async move {
                    let res = provider
                        .get_storage_at(Into::<FieldElement>::into(contract_address), key, block)
                        .await
                        .map(|f| f.into())
                        .map_err(ForkedBackendError::Provider);

                    sender.send(res).expect("failed to send storage result")
                });

                self.pending_requests.push(fut);
            }

            BackendRequest::GetClassHashAt(contract_address, sender) => {
                let fut = Box::pin(async move {
                    let res = provider
                        .get_class_hash_at(block, Into::<FieldElement>::into(contract_address))
                        .await
                        .map_err(ForkedBackendError::Provider);

                    sender.send(res).expect("failed to send class hash result")
                });

                self.pending_requests.push(fut);
            }

            BackendRequest::GetClassAt(class_hash, sender) => {
                let fut = Box::pin(async move {
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

impl Future for ForkedBackend {
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

/// Handler for the [`ForkedBackend`].
#[derive(Debug)]
pub struct ForkedBackendHandler(Mutex<Sender<BackendRequest>>);

impl Clone for ForkedBackendHandler {
    fn clone(&self) -> Self {
        Self(Mutex::new(self.0.lock().clone()))
    }
}

impl ForkedBackendHandler {
    pub fn new_with_backend_thread(
        provider: Arc<JsonRpcClient<HttpTransport>>,
        block_id: BlockHashOrNumber,
    ) -> Self {
        let (backend, handler) = Self::new(provider, block_id);

        thread::Builder::new()
            .spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to create fork backend thread tokio runtime")
                    .block_on(handler);
            })
            .expect("failed to spawn fork backend thread");

        trace!(target: "forked_backend", "fork backend thread spawned");

        backend
    }

    fn new(
        provider: Arc<JsonRpcClient<HttpTransport>>,
        block_id: BlockHashOrNumber,
    ) -> (Self, ForkedBackend) {
        let block = match block_id {
            BlockHashOrNumber::Hash(hash) => BlockId::Hash(hash),
            BlockHashOrNumber::Num(number) => BlockId::Number(number),
        };

        let (sender, rx) = channel(1);
        let backend = ForkedBackend {
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
        trace!(target: "forked_backend", "request nonce for contract address {contract_address}");
        tokio::task::block_in_place(|| {
            let (sender, rx) = oneshot();
            self.0
                .lock()
                .try_send(BackendRequest::GetNonce(contract_address, sender))
                .map_err(ForkedBackendError::Send)?;
            rx.recv().expect("failed to receive nonce result")
        })
    }

    pub fn do_get_storage(
        &self,
        contract_address: ContractAddress,
        key: StorageKey,
    ) -> Result<StorageValue, ForkedBackendError> {
        trace!(target: "forked_backend", "request storage for address {contract_address} at key {key:#x}" );
        tokio::task::block_in_place(|| {
            let (sender, rx) = oneshot();
            self.0
                .lock()
                .try_send(BackendRequest::GetStorage(contract_address, key, sender))
                .map_err(ForkedBackendError::Send)?;
            rx.recv().expect("failed to receive storage result")
        })
    }

    pub fn do_get_class_hash_at(
        &self,
        contract_address: ContractAddress,
    ) -> Result<ClassHash, ForkedBackendError> {
        trace!(target: "forked_backend", "request class hash at address {contract_address}");
        tokio::task::block_in_place(|| {
            let (sender, rx) = oneshot();
            self.0
                .lock()
                .try_send(BackendRequest::GetClassHashAt(contract_address, sender))
                .map_err(ForkedBackendError::Send)?;
            rx.recv().expect("failed to receive class hash result")
        })
    }

    pub fn do_get_class_at(
        &self,
        class_hash: ClassHash,
    ) -> Result<starknet::core::types::ContractClass, ForkedBackendError> {
        trace!(target: "forked_backend", "request class at hash {class_hash:#x}");
        tokio::task::block_in_place(|| {
            let (sender, rx) = oneshot();
            self.0
                .lock()
                .try_send(BackendRequest::GetClassAt(class_hash, sender))
                .map_err(ForkedBackendError::Send)?;
            rx.recv().expect("failed to receive class result")
        })
    }

    pub fn do_get_compiled_class_hash(
        &self,
        class_hash: ClassHash,
    ) -> Result<CompiledClassHash, ForkedBackendError> {
        trace!(target: "forked_backend", "request compiled class hash at class {class_hash:#x}");
        let class = self.do_get_class_at(class_hash)?;
        // if its a legacy class, then we just return back the class hash
        // else if sierra class, then we have to compile it and compute the compiled class hash.
        match class {
            starknet::core::types::ContractClass::Legacy(_) => Ok(class_hash),

            starknet::core::types::ContractClass::Sierra(sierra_class) => {
                tokio::task::block_in_place(|| {
                    compiled_class_hash_from_flattened_sierra_class(&sierra_class)
                })
                .map_err(|e| ForkedBackendError::ComputeClassHashError(e.to_string()))
            }
        }
    }
}

// impl StateProvider for ForkedBackendHandler {
//     fn nonce(&self, address: ContractAddress) -> Result<Option<Nonce>> {
//         let nonce = self.do_get_nonce(address).unwrap();
//         Ok(Some(nonce))
//     }

//     fn class(
//         &self,
//         hash: ClassHash,
//     ) -> Result<Option<katana_primitives::contract::CompiledContractClass>> {
//         let class = self.do_get_class_at(hash).unwrap();
//         match class {
//             starknet::core::types::ContractClass::Legacy(legacy_class) => {
//                 Ok(Some(legacy_rpc_to_inner_class(&legacy_class).map(|(_, class)| class)?))
//             }

//             starknet::core::types::ContractClass::Sierra(sierra_class) => {
//                 Ok(Some(rpc_to_inner_class(&sierra_class).map(|(_, class)| class)?))
//             }
//         }
//     }

//     fn storage(
//         &self,
//         address: ContractAddress,
//         storage_key: StorageKey,
//     ) -> Result<Option<StorageValue>> {
//         let storage = self.do_get_storage(address, storage_key).unwrap();
//         Ok(Some(storage))
//     }

//     fn class_hash_of_contract(&self, address: ContractAddress) -> Result<Option<ClassHash>> {
//         let class_hash = self.do_get_class_hash_at(address).unwrap();
//         Ok(Some(class_hash))
//     }
// }

// impl StateProviderExt for ForkedBackendHandler {
//     fn sierra_class(
//         &self,
//         hash: ClassHash,
//     ) -> Result<Option<katana_primitives::contract::SierraClass>> {
//         let class = self.do_get_class_at(hash).unwrap();
//         match class {
//             starknet::core::types::ContractClass::Sierra(sierra_class) => Ok(Some(sierra_class)),
//             starknet::core::types::ContractClass::Legacy(_) => Ok(None),
//         }
//     }

//     fn compiled_class_hash_of_class_hash(
//         &self,
//         hash: ClassHash,
//     ) -> Result<Option<CompiledClassHash>> {
//         let hash = self.do_get_compiled_class_hash(hash).unwrap();
//         Ok(Some(hash))
//     }
// }
