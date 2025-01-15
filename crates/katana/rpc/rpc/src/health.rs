use jsonrpsee::core::server::rpc_module::Methods;
use jsonrpsee::server::middleware::proxy_get_request::ProxyGetRequestLayer;
use jsonrpsee::RpcModule;
use serde_json::json;

/// Simple health check endpoint.
#[derive(Debug)]
pub struct HealthCheck;

impl HealthCheck {
    const METHOD: &'static str = "health";
    const PROXY_PATH: &'static str = "/";

    pub(crate) fn proxy() -> ProxyGetRequestLayer {
        Self::proxy_with_path(Self::PROXY_PATH)
    }

    fn proxy_with_path(path: impl Into<String>) -> ProxyGetRequestLayer {
        ProxyGetRequestLayer::new(path, Self::METHOD).expect("path starts with /")
    }
}

impl From<HealthCheck> for Methods {
    fn from(_: HealthCheck) -> Self {
        let mut module = RpcModule::new(());
        module.register_method(HealthCheck::METHOD, |_, _| Ok(json!({ "health": true }))).unwrap();
        module.into()
    }
}
