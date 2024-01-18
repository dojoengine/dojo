use async_trait::async_trait;

use crate::error::BindgenResult;
use crate::DojoData;

pub mod typescript;
pub mod unity;

#[derive(Debug)]
pub enum BuiltinPlugins {
    Typescript,
    Unity,
}

#[async_trait]
pub trait BuiltinPlugin {
    /// Generates code by executing the plugin.
    ///
    /// # Arguments
    ///
    /// * `data` - Dojo data gathered from the compiled project.
    async fn generate_code(&self, data: &DojoData) -> BindgenResult<()>;
}
