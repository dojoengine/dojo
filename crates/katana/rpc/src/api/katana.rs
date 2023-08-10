use jsonrpsee::core::Error;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::error::CallError;
use jsonrpsee::types::ErrorObject;
use katana_core::accounts::Account;
use starknet::core::types::FieldElement;
use starknet_api::hash::StarkFelt;

#[derive(thiserror::Error, Clone, Copy, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum KatanaApiError {
    #[error("Failed to change next block timestamp.")]
    FailedToChangeNextBlockTimestamp = 1,
    #[error("Failed to dump state.")]
    FailedToDumpState = 2,
    #[error("Failed to update storage.")]
    FailedToUpdateStorage = 3,
}

impl From<KatanaApiError> for Error {
    fn from(err: KatanaApiError) -> Self {
        Error::Call(CallError::Custom(ErrorObject::owned(err as i32, err.to_string(), None::<()>)))
    }
}

pub type ContractAddressFieldElement = FieldElement;
pub type StorageKeyFieldElement = FieldElement;

#[rpc(server, namespace = "katana")]
pub trait KatanaApi {
    #[method(name = "generateBlock")]
    async fn generate_block(&self) -> Result<(), Error>;

    #[method(name = "nextBlockTimestamp")]
    async fn next_block_timestamp(&self) -> Result<u64, Error>;

    #[method(name = "setNextBlockTimestamp")]
    async fn set_next_block_timestamp(&self, timestamp: u64) -> Result<(), Error>;

    #[method(name = "increaseNextBlockTimestamp")]
    async fn increase_next_block_timestamp(&self, timestamp: u64) -> Result<(), Error>;

    #[method(name = "predeployedAccounts")]
    async fn predeployed_accounts(&self) -> Result<Vec<Account>, Error>;

    #[method(name = "getStorageAtMultiple")]
    async fn get_storage_multiple(
        &self,
        storage_addresses: Vec<(ContractAddressFieldElement, Vec<StorageKeyFieldElement>)>,
    ) -> Result<Vec<Vec<StarkFelt>>, Error>;

    #[method(name = "setStorageAt")]
    async fn set_storage_at(
        &self,
        contract_address: FieldElement,
        key: FieldElement,
        value: FieldElement,
    ) -> Result<(), Error>;
}
