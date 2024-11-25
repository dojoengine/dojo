use katana_executor::ExecutorFactory;
use katana_grpc::api::{
    BlockHashAndNumberRequest, BlockHashAndNumberResponse, BlockNumberRequest, BlockNumberResponse,
    CallRequest, CallResponse, ChainIdRequest, ChainIdResponse, EstimateFeeRequest,
    EstimateFeeResponse, EstimateMessageFeeRequest, GetBlockRequest,
    GetBlockTransactionCountResponse, GetBlockWithReceiptsResponse, GetBlockWithTxHashesResponse,
    GetBlockWithTxsResponse, GetClassAtRequest, GetClassAtResponse, GetClassHashAtRequest,
    GetClassHashAtResponse, GetClassRequest, GetClassResponse, GetEventsRequest, GetEventsResponse,
    GetNonceRequest, GetNonceResponse, GetStateUpdateResponse, GetStorageAtRequest,
    GetStorageAtResponse, GetTransactionByBlockIdAndIndexRequest,
    GetTransactionByBlockIdAndIndexResponse, GetTransactionByHashRequest,
    GetTransactionByHashResponse, GetTransactionReceiptRequest, GetTransactionReceiptResponse,
    GetTransactionStatusRequest, GetTransactionStatusResponse, SpecVersionRequest,
    SpecVersionResponse, SyncingRequest, SyncingResponse,
};
use katana_primitives::block::BlockIdOrTag;
use katana_primitives::contract::StorageKey;
use katana_primitives::ContractAddress;
use tonic::{Request, Response, Status};

const RPC_SPEC_VERSION: &str = "0.7.1";

#[tonic::async_trait]
impl<EF: ExecutorFactory> katana_grpc::StarknetApi for super::StarknetApi<EF> {
    async fn spec_version(
        &self,
        _: Request<SpecVersionRequest>,
    ) -> Result<Response<SpecVersionResponse>, Status> {
        let message = SpecVersionResponse { version: RPC_SPEC_VERSION.to_string() };
        Ok(Response::new(message))
    }

    async fn get_block_with_tx_hashes(
        &self,
        _request: Request<GetBlockRequest>,
    ) -> Result<Response<GetBlockWithTxHashesResponse>, Status> {
        todo!()
    }

    async fn get_block_with_txs(
        &self,
        _request: Request<GetBlockRequest>,
    ) -> Result<Response<GetBlockWithTxsResponse>, Status> {
        todo!()
    }

    async fn get_block_with_receipts(
        &self,
        _request: Request<GetBlockRequest>,
    ) -> Result<Response<GetBlockWithReceiptsResponse>, Status> {
        todo!()
    }

    async fn get_state_update(
        &self,
        _request: Request<GetBlockRequest>,
    ) -> Result<Response<GetStateUpdateResponse>, Status> {
        todo!()
    }

    async fn get_storage_at(
        &self,
        request: Request<GetStorageAtRequest>,
    ) -> Result<Response<GetStorageAtResponse>, Status> {
        let GetStorageAtRequest { block_id, contract_address, key } = request.into_inner();

        let block_id: BlockIdOrTag = block_id.unwrap().try_into().unwrap();
        let address: ContractAddress = contract_address.unwrap().try_into().unwrap();
        let key: StorageKey = key.unwrap().try_into().unwrap();

        let value = self.storage_at(address, key, block_id).unwrap();
        let value = katana_grpc::types::Felt::from(value);

        let message = GetStorageAtResponse { value: Some(value) };
        Ok(Response::new(message))
    }

    async fn get_transaction_status(
        &self,
        _request: Request<GetTransactionStatusRequest>,
    ) -> Result<Response<GetTransactionStatusResponse>, Status> {
        todo!()
    }

    async fn get_transaction_by_hash(
        &self,
        _request: Request<GetTransactionByHashRequest>,
    ) -> Result<Response<GetTransactionByHashResponse>, Status> {
        todo!()
    }

    async fn get_transaction_by_block_id_and_index(
        &self,
        _request: Request<GetTransactionByBlockIdAndIndexRequest>,
    ) -> Result<Response<GetTransactionByBlockIdAndIndexResponse>, Status> {
        todo!()
    }

    async fn get_transaction_receipt(
        &self,
        _request: Request<GetTransactionReceiptRequest>,
    ) -> Result<Response<GetTransactionReceiptResponse>, Status> {
        todo!()
    }

    async fn get_class(
        &self,
        _request: Request<GetClassRequest>,
    ) -> Result<Response<GetClassResponse>, Status> {
        todo!()
    }

    async fn get_class_hash_at(
        &self,
        request: Request<GetClassHashAtRequest>,
    ) -> Result<Response<GetClassHashAtResponse>, Status> {
        let GetClassHashAtRequest { block_id, contract_address } = request.into_inner();

        let block_id: BlockIdOrTag = block_id.unwrap().try_into().unwrap();
        let address: ContractAddress = contract_address.unwrap().try_into().unwrap();

        let class_hash = self.class_hash_at_address(block_id, address).await.unwrap();
        let class_hash = katana_grpc::types::Felt::from(class_hash);

        let message = GetClassHashAtResponse { class_hash: Some(class_hash) };
        Ok(Response::new(message))
    }

    async fn get_class_at(
        &self,
        _request: Request<GetClassAtRequest>,
    ) -> Result<Response<GetClassAtResponse>, Status> {
        todo!()
    }

    async fn get_block_transaction_count(
        &self,
        request: Request<GetBlockRequest>,
    ) -> Result<Response<GetBlockTransactionCountResponse>, Status> {
        let GetBlockRequest { block_id } = request.into_inner();
        let block_id: BlockIdOrTag = block_id.unwrap().try_into().unwrap();

        let count = self.block_tx_count(block_id).await.unwrap();
        let message = GetBlockTransactionCountResponse { count };

        Ok(Response::new(message))
    }

    async fn call(&self, _request: Request<CallRequest>) -> Result<Response<CallResponse>, Status> {
        todo!()
    }

    async fn estimate_fee(
        &self,
        _request: Request<EstimateFeeRequest>,
    ) -> Result<Response<EstimateFeeResponse>, Status> {
        todo!()
    }

    async fn estimate_message_fee(
        &self,
        _request: Request<EstimateMessageFeeRequest>,
    ) -> Result<Response<EstimateFeeResponse>, Status> {
        todo!()
    }

    async fn block_number(
        &self,
        _: Request<BlockNumberRequest>,
    ) -> Result<Response<BlockNumberResponse>, Status> {
        let block_number = self.latest_block_number().await.unwrap();
        let message = BlockNumberResponse { block_number };
        Ok(Response::new(message))
    }

    async fn block_hash_and_number(
        &self,
        _: Request<BlockHashAndNumberRequest>,
    ) -> Result<Response<BlockHashAndNumberResponse>, Status> {
        let (block_hash, block_number) = self.block_hash_and_number().unwrap();
        let message =
            BlockHashAndNumberResponse { block_number, block_hash: Some(block_hash.into()) };
        Ok(Response::new(message))
    }

    async fn chain_id(
        &self,
        _: Request<ChainIdRequest>,
    ) -> Result<Response<ChainIdResponse>, Status> {
        let id = self.inner.backend.chain_spec.id.id();
        let id = katana_grpc::types::Felt::from(id);
        let message = ChainIdResponse { chain_id: Some(id) };
        Ok(Response::new(message))
    }

    async fn syncing(
        &self,
        _request: Request<SyncingRequest>,
    ) -> Result<Response<SyncingResponse>, Status> {
        todo!()
    }

    async fn get_events(
        &self,
        _request: Request<GetEventsRequest>,
    ) -> Result<Response<GetEventsResponse>, Status> {
        todo!()
    }

    async fn get_nonce(
        &self,
        request: Request<GetNonceRequest>,
    ) -> Result<Response<GetNonceResponse>, Status> {
        let GetNonceRequest { block_id, contract_address } = request.into_inner();
        let block_id: BlockIdOrTag = block_id.unwrap().try_into().unwrap();
        let address: ContractAddress = contract_address.unwrap().try_into().unwrap();

        let nonce = self.nonce_at(block_id, address).await.unwrap();
        let nonce = katana_grpc::types::Felt::from(nonce);

        let message = GetNonceResponse { nonce: Some(nonce) };
        Ok(Response::new(message))
    }
}
