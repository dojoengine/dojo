use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use dojo_types::primitive::Primitive;
use dojo_types::schema::{Member, Struct, Ty};
use dojo_world::contracts::abigen::model::Layout;
use dojo_world::contracts::naming::compute_selector_from_names;
use dojo_world::contracts::world::WorldContractReader;
use sqlx::sqlite::SqlitePool;
use starknet::core::types::{
    BlockHashAndNumber, BlockId, BroadcastedDeclareTransaction,
    BroadcastedDeployAccountTransaction, BroadcastedInvokeTransaction, BroadcastedTransaction,
    ContractClass, DeclareTransactionResult, DeployAccountTransactionResult, Event, EventFilter,
    EventsPage, FeeEstimate, Felt, FunctionCall, InvokeTransactionResult,
    MaybePendingBlockWithReceipts, MaybePendingBlockWithTxHashes, MaybePendingBlockWithTxs,
    MaybePendingStateUpdate, MsgFromL1, SimulatedTransaction, SimulationFlag,
    SimulationFlagForEstimateFee, SyncStatusType, Transaction, TransactionReceiptWithBlockInfo,
    TransactionStatus, TransactionTrace, TransactionTraceWithHash,
};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::{Provider, ProviderError, ProviderRequestData, ProviderResponseData};
use tokio::sync::mpsc::unbounded_channel;
use torii_sqlite::cache::{Model, ModelCache};
use torii_sqlite::Sql;

use crate::processors::store_del_record::StoreDelRecordProcessor;
use crate::processors::store_set_record::StoreSetRecordProcessor;
use crate::processors::{EventProcessor, EventProcessorConfig};

#[derive(Debug)]
pub struct MockSql {
    sql: Sql,
    model_response: Option<Model>,
    model_error: Option<String>,
    set_entity_error: Option<String>,
    delete_entity_error: Option<String>,
}

impl MockSql {
    pub async fn new() -> Self {
        println!("Starting MockSql initialization");

        let pool = match SqlitePool::connect("sqlite::memory:").await {
            Ok(p) => {
                println!("SQLite in-memory pool created successfully");
                p
            }
            Err(e) => {
                eprintln!("Failed to create SQLite pool: {:?}", e);
                panic!("Could not create SQLite pool");
            }
        };

        println!("Attempting to create tables");
        match sqlx::query(
            r#"
                    CREATE TABLE IF NOT EXISTS token_balances (
                        token_id TEXT PRIMARY KEY,
                        balance INTEGER
                    );
                    CREATE TABLE IF NOT EXISTS models (
                        id INTEGER PRIMARY KEY,
                        selector TEXT NOT NULL,
                        name TEXT NOT NULL,
                        namespace TEXT NOT NULL,
                        class_hash TEXT NOT NULL,
                        contract_address TEXT NOT NULL
                    );
                "#,
        )
        .execute(&pool)
        .await
        {
            Ok(_) => println!("Tables created successfully"),
            Err(e) => {
                eprintln!("Failed to create tables: {:?}", e);
                panic!("Could not create tables");
            }
        }

        // Create a channel that won't be dropped immediately
        let (sender, _receiver) = unbounded_channel();

        println!("Creating ModelCache");
        let model_cache = match Arc::new(ModelCache::new(pool.clone())) {
            cache => {
                println!("ModelCache created successfully");
                cache
            }
        };

        println!("Creating Sql");
        let sql = match Sql::new(pool, sender, &[], model_cache).await {
            Ok(s) => {
                println!("Sql created successfully");
                s
            }
            Err(e) => {
                eprintln!("Failed to create Sql: {:?}", e);
                panic!("Could not create Sql");
            }
        };

        Self {
            sql,
            model_response: None,
            model_error: None,
            set_entity_error: None,
            delete_entity_error: None,
        }
    }

    pub fn expect_model(&mut self, result: Result<Model>) {
        match result {
            Ok(model) => self.model_response = Some(model),
            Err(e) => self.model_error = Some(e.to_string()),
        }
    }

    pub fn expect_set_entity(&mut self, result: Result<()>) {
        if let Err(e) = result {
            self.set_entity_error = Some(e.to_string());
        }
    }

    pub fn expect_delete_entity(&mut self, result: Result<()>) {
        if let Err(e) = result {
            self.delete_entity_error = Some(e.to_string());
        }
    }
}

impl std::ops::Deref for MockSql {
    type Target = Sql;

    fn deref(&self) -> &Self::Target {
        &self.sql
    }
}

impl std::ops::DerefMut for MockSql {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sql
    }
}

// Helper function to create test models
fn create_test_model(name: &str, namespace: &str, model_type: &str) -> Model {
    let schema = match model_type {
        "Position" => Ty::Struct(Struct {
            name: name.to_string(),
            children: vec![
                Member {
                    name: "x".to_string(),
                    ty: Ty::Primitive(Primitive::U32(None)),
                    key: false,
                },
                Member {
                    name: "y".to_string(),
                    ty: Ty::Primitive(Primitive::U32(None)),
                    key: false,
                },
            ],
        }),
        "PlayerConfig" => Ty::Struct(Struct {
            name: name.to_string(),
            children: vec![Member {
                name: "name".to_string(),
                ty: Ty::Primitive(Primitive::Felt252(None)),
                key: false,
            }],
        }),
        _ => panic!("Unknown model type"),
    };

    Model {
        selector: compute_selector_from_names(namespace, name),
        packed_size: 1,
        unpacked_size: 2,
        schema,
        class_hash: Felt::from_hex("0x123").unwrap(),
        contract_address: Felt::from_hex("0x456").unwrap(),
        layout: Layout::Struct(vec![]),
        name: name.to_string(),
        namespace: namespace.to_string(),
    }
}

// Mock Provider implementation
#[derive(Debug)]
struct MockProvider;

#[async_trait]
impl Provider for MockProvider {
    async fn block_hash_and_number(&self) -> Result<BlockHashAndNumber, ProviderError> {
        Ok(BlockHashAndNumber { block_hash: Felt::from_hex("0x1").unwrap(), block_number: 1 })
    }

    async fn batch_requests<R>(
        &self,
        _requests: R,
    ) -> Result<Vec<ProviderResponseData>, ProviderError>
    where
        R: AsRef<[ProviderRequestData]> + Send + Sync,
    {
        Ok(vec![])
    }

    async fn get_block_with_tx_hashes<B>(
        &self,
        _block_id: B,
    ) -> Result<MaybePendingBlockWithTxHashes, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        unimplemented!()
    }

    async fn get_block_with_txs<B>(
        &self,
        _block_id: B,
    ) -> Result<MaybePendingBlockWithTxs, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        unimplemented!()
    }

    async fn get_block_with_receipts<B>(
        &self,
        _block_id: B,
    ) -> Result<MaybePendingBlockWithReceipts, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        unimplemented!()
    }

    async fn get_state_update<B>(
        &self,
        _block_id: B,
    ) -> Result<MaybePendingStateUpdate, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        unimplemented!()
    }

    async fn get_storage_at<A, K, B>(
        &self,
        _contract_address: A,
        _key: K,
        _block_id: B,
    ) -> Result<Felt, ProviderError>
    where
        A: AsRef<Felt> + Send + Sync,
        K: AsRef<Felt> + Send + Sync,
        B: AsRef<BlockId> + Send + Sync,
    {
        unimplemented!()
    }

    async fn get_transaction_status<H>(
        &self,
        _transaction_hash: H,
    ) -> Result<TransactionStatus, ProviderError>
    where
        H: AsRef<Felt> + Send + Sync,
    {
        unimplemented!()
    }

    async fn get_transaction_by_hash<H>(
        &self,
        _transaction_hash: H,
    ) -> Result<Transaction, ProviderError>
    where
        H: AsRef<Felt> + Send + Sync,
    {
        unimplemented!()
    }

    async fn get_transaction_by_block_id_and_index<B>(
        &self,
        _block_id: B,
        _index: u64,
    ) -> Result<Transaction, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        unimplemented!()
    }

    async fn get_transaction_receipt<H>(
        &self,
        _transaction_hash: H,
    ) -> Result<TransactionReceiptWithBlockInfo, ProviderError>
    where
        H: AsRef<Felt> + Send + Sync,
    {
        unimplemented!()
    }

    async fn get_class<B, H>(
        &self,
        _block_id: B,
        _class_hash: H,
    ) -> Result<ContractClass, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        H: AsRef<Felt> + Send + Sync,
    {
        unimplemented!()
    }

    async fn get_class_hash_at<B, A>(
        &self,
        _block_id: B,
        _contract_address: A,
    ) -> Result<Felt, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        A: AsRef<Felt> + Send + Sync,
    {
        unimplemented!()
    }

    async fn get_class_at<B, A>(
        &self,
        _block_id: B,
        _contract_address: A,
    ) -> Result<ContractClass, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        A: AsRef<Felt> + Send + Sync,
    {
        unimplemented!()
    }

    async fn get_block_transaction_count<B>(&self, _block_id: B) -> Result<u64, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        unimplemented!()
    }

    async fn call<R, B>(&self, _request: R, _block_id: B) -> Result<Vec<Felt>, ProviderError>
    where
        R: AsRef<FunctionCall> + Send + Sync,
        B: AsRef<BlockId> + Send + Sync,
    {
        unimplemented!()
    }

    async fn estimate_fee<R, S, B>(
        &self,
        _request: R,
        _simulation_flags: S,
        _block_id: B,
    ) -> Result<Vec<FeeEstimate>, ProviderError>
    where
        R: AsRef<[BroadcastedTransaction]> + Send + Sync,
        S: AsRef<[SimulationFlagForEstimateFee]> + Send + Sync,
        B: AsRef<BlockId> + Send + Sync,
    {
        unimplemented!()
    }

    async fn estimate_message_fee<M, B>(
        &self,
        _message: M,
        _block_id: B,
    ) -> Result<FeeEstimate, ProviderError>
    where
        M: AsRef<MsgFromL1> + Send + Sync,
        B: AsRef<BlockId> + Send + Sync,
    {
        unimplemented!()
    }

    async fn block_number(&self) -> Result<u64, ProviderError> {
        Ok(1)
    }

    async fn chain_id(&self) -> Result<Felt, ProviderError> {
        Ok(Felt::from_hex("0x1").unwrap())
    }

    async fn syncing(&self) -> Result<SyncStatusType, ProviderError> {
        unimplemented!()
    }

    async fn get_events(
        &self,
        _filter: EventFilter,
        _continuation_token: Option<String>,
        _chunk_size: u64,
    ) -> Result<EventsPage, ProviderError> {
        unimplemented!()
    }

    async fn get_nonce<B, A>(
        &self,
        _block_id: B,
        _contract_address: A,
    ) -> Result<Felt, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        A: AsRef<Felt> + Send + Sync,
    {
        unimplemented!()
    }

    async fn add_invoke_transaction<I>(
        &self,
        _invoke_transaction: I,
    ) -> Result<InvokeTransactionResult, ProviderError>
    where
        I: AsRef<BroadcastedInvokeTransaction> + Send + Sync,
    {
        unimplemented!()
    }

    async fn add_declare_transaction<D>(
        &self,
        _declare_transaction: D,
    ) -> Result<DeclareTransactionResult, ProviderError>
    where
        D: AsRef<BroadcastedDeclareTransaction> + Send + Sync,
    {
        unimplemented!()
    }

    async fn add_deploy_account_transaction<D>(
        &self,
        _deploy_account_transaction: D,
    ) -> Result<DeployAccountTransactionResult, ProviderError>
    where
        D: AsRef<BroadcastedDeployAccountTransaction> + Send + Sync,
    {
        unimplemented!()
    }

    async fn trace_transaction<H>(
        &self,
        _transaction_hash: H,
    ) -> Result<TransactionTrace, ProviderError>
    where
        H: AsRef<Felt> + Send + Sync,
    {
        unimplemented!()
    }

    async fn simulate_transactions<B, T, S>(
        &self,
        _block_id: B,
        _transactions: T,
        _simulation_flags: S,
    ) -> Result<Vec<SimulatedTransaction>, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        T: AsRef<[BroadcastedTransaction]> + Send + Sync,
        S: AsRef<[SimulationFlag]> + Send + Sync,
    {
        unimplemented!()
    }
    async fn trace_block_transactions<B>(
        &self,
        _block_id: B,
    ) -> Result<Vec<TransactionTraceWithHash>, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        unimplemented!()
    }

    async fn spec_version(&self) -> Result<String, ProviderError> {
        Ok("mock_version".to_string())
    }
}

impl MockProvider {
    fn new() -> Self {
        Self
    }
}

#[tokio::test]
async fn test_store_set_record_processor() {
    let provider = MockProvider::new();
    let world = WorldContractReader::new(Felt::from_hex("0x1").unwrap(), Arc::new(provider));
    let mut mock_sql = MockSql::new().await;

    let model_id = compute_selector_from_names("ns", "Position");
    let test_model = create_test_model("Position", "ns", "Position");

    mock_sql.expect_model(Ok(test_model.clone()));
    mock_sql.expect_set_entity(Ok(()));

    let processor = StoreSetRecordProcessor::default();

    let event = Event {
        from_address: Felt::from_hex("0x1").unwrap(),
        keys: vec![
            get_selector_from_name("StoreSetRecord").unwrap(),
            model_id,
            Felt::from_hex("0x456").unwrap(),
        ],
        data: vec![Felt::from(10), Felt::from(20)],
    };

    let result = processor
        .process(
            &world,
            &mut mock_sql, // This should now work because of Deref
            1,
            1000,
            "test_event_id",
            &event,
            &EventProcessorConfig::default(),
        )
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_store_set_record_processor_invalid_data() {
    let provider = MockProvider::new();
    let world = WorldContractReader::new(Felt::from_hex("0x1").unwrap(), Arc::new(provider));
    let mut mock_sql = MockSql::new().await;

    let model_id = compute_selector_from_names("ns", "Position");
    let test_model = create_test_model("Position", "ns", "Position");

    mock_sql.expect_model(Ok(test_model.clone()));

    let processor = StoreSetRecordProcessor::default();

    let event = Event {
        from_address: Felt::from_hex("0x1").unwrap(),
        keys: vec![
            get_selector_from_name("StoreSetRecord").unwrap(),
            model_id,
            Felt::from_hex("0x456").unwrap(),
        ],
        data: vec![], // Empty data should cause an error
    };

    let result = processor
        .process(
            &world,
            &mut mock_sql,
            1,
            1000,
            "test_event_id",
            &event,
            &EventProcessorConfig::default(),
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_store_del_record_processor() {
    let provider = MockProvider::new();
    let world = WorldContractReader::new(Felt::from_hex("0x1").unwrap(), Arc::new(provider));
    let mut mock_sql = MockSql::new().await;

    let model_id = compute_selector_from_names("ns", "PlayerConfig");
    let test_model = create_test_model("PlayerConfig", "ns", "PlayerConfig");

    mock_sql.expect_model(Ok(test_model.clone()));
    mock_sql.expect_delete_entity(Ok(()));

    let processor = StoreDelRecordProcessor::default();

    let event = Event {
        from_address: Felt::from_hex("0x1").unwrap(),
        keys: vec![
            get_selector_from_name("StoreDelRecord").unwrap(),
            model_id,
            Felt::from_hex("0x456").unwrap(),
        ],
        data: vec![],
    };

    let result = processor
        .process(
            &world,
            &mut mock_sql,
            1,
            1000,
            "test_event_id",
            &event,
            &EventProcessorConfig::default(),
        )
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_store_del_record_processor_missing_keys() {
    let provider = MockProvider::new();
    let world = WorldContractReader::new(Felt::from_hex("0x1").unwrap(), Arc::new(provider));
    let mut mock_sql = MockSql::new().await;

    let processor = StoreDelRecordProcessor::default();

    let event = Event {
        from_address: Felt::from_hex("0x1").unwrap(),
        keys: vec![
            get_selector_from_name("StoreDelRecord").unwrap(),
            // Missing model_id and entity_id
        ],
        data: vec![],
    };

    let result = processor
        .process(
            &world,
            &mut mock_sql,
            1,
            1000,
            "test_event_id",
            &event,
            &EventProcessorConfig::default(),
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_store_del_record_processor_model_not_found() {
    let provider = MockProvider::new();
    let world = WorldContractReader::new(Felt::from_hex("0x1").unwrap(), Arc::new(provider));
    let mut mock_sql = MockSql::new().await;

    let model_id = compute_selector_from_names("ns", "NonexistentModel");

    mock_sql.expect_model(Err(anyhow::anyhow!("Model not found")));

    let processor = StoreDelRecordProcessor::default();

    let event = Event {
        from_address: Felt::from_hex("0x1").unwrap(),
        keys: vec![
            get_selector_from_name("StoreDelRecord").unwrap(),
            model_id,
            Felt::from_hex("0x456").unwrap(),
        ],
        data: vec![],
    };

    let result = processor
        .process(
            &world,
            &mut mock_sql,
            1,
            1000,
            "test_event_id",
            &event,
            &EventProcessorConfig::default(),
        )
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Model not found"));
}
