use std::future::IntoFuture;
use std::sync::Arc;

use anyhow::Result;
use dojo_metrics::exporters::prometheus::PrometheusRecorder;
use dojo_metrics::{Report, Server as MetricsServer};
use katana_db::mdbx::DbEnv;
use katana_feeder_gateway::client::SequencerGateway;
use katana_pipeline::stage::{Blocks, Classes};
use katana_pipeline::tip_watcher::ChainTipWatcher;
use katana_pipeline::{Pipeline, PipelineHandle};
use katana_pool::ordering::FiFo;
use katana_pool::pool::Pool;
use katana_pool::validation::NoopValidator;
use katana_primitives::transaction::ExecutableTxWithHash;
use katana_provider::providers::db::DbProvider;
use katana_tasks::TaskManager;
use tracing::info;

pub use crate::config::Config;

type TxPool =
    Pool<ExecutableTxWithHash, NoopValidator<ExecutableTxWithHash>, FiFo<ExecutableTxWithHash>>;

pub struct LaunchedFullNode {
    pub db: DbEnv,
    pub pool: TxPool,
    pub config: Arc<Config>,
    pub task_manager: TaskManager,
    pub pipeline_handle: PipelineHandle,
}

impl LaunchedFullNode {
    pub async fn stop(self) -> Result<()> {
        self.task_manager.shutdown().await;
        Ok(())
    }
}

pub struct FullNode {
    pub db: DbEnv,
    pub pool: TxPool,
    pub config: Arc<Config>,
    pub task_manager: TaskManager,
    pub pipeline: Pipeline<DbProvider>,
}

impl FullNode {
    pub fn build(config: Config) -> Result<Self> {
        if config.metrics.is_some() {
            // Metrics recorder must be initialized before calling any of the metrics macros, in
            // order for it to be registered.
            let _ = PrometheusRecorder::install("katana")?;
        }

        // -- build task manager

        let task_manager = TaskManager::current();

        // -- build db and storage provider

        let path = config.db.dir.clone().expect("database path must exist");
        let db = katana_db::init_db(path)?;
        let provider = DbProvider::new(db.clone());

        // --- build transaction pool

        let pool = TxPool::new(NoopValidator::new(), FiFo::new());

        // --- build pipeline

        let fgw = SequencerGateway::sn_sepolia();
        let (mut pipeline, _) = Pipeline::new(provider.clone(), 64);

        pipeline.add_stage(Blocks::new(provider.clone(), fgw.clone(), 3));
        pipeline.add_stage(Classes::new(provider, fgw.clone(), 3));

        let node = FullNode { pool, config: Arc::new(config), task_manager, pipeline, db };

        Ok(node)
    }

    pub fn launch(self) -> Result<LaunchedFullNode> {
        if let Some(ref cfg) = self.config.metrics {
            let reports: Vec<Box<dyn Report>> = vec![Box::new(self.db.clone()) as Box<dyn Report>];
            let exporter = PrometheusRecorder::current().expect("qed; should exist at this point");

            let addr = cfg.socket_addr();
            let server = MetricsServer::new(exporter).with_process_metrics().with_reports(reports);
            self.task_manager.task_spawner().build_task().spawn(server.start(addr));

            info!(%addr, "Metrics server started.");
        }

        let pipeline_handle = self.pipeline.handle();
        let fgw = SequencerGateway::sn_sepolia();
        let tip_watcher = ChainTipWatcher::new(fgw, pipeline_handle.clone());

        self.task_manager
            .task_spawner()
            .build_task()
            .critical()
            .name("Chain tip watcher")
            .spawn(tip_watcher.into_future());

        self.task_manager
            .task_spawner()
            .build_task()
            .critical()
            .name("Pipeline")
            .spawn(self.pipeline.into_future());

        Ok(LaunchedFullNode {
            db: self.db,
            pipeline_handle,
            pool: self.pool,
            config: self.config,
            task_manager: self.task_manager,
        })
    }
}
