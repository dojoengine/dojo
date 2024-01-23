pub mod account;
pub mod starknet;
pub mod transaction;
pub mod world;

const STARKNET_RPC_URL_ENV_VAR: &str = "STARKNET_RPC_URL";
const DOJO_PRIVATE_KEY_ENV_VAR: &str = "DOJO_PRIVATE_KEY";
const DOJO_KEYSTORE_PASSWORD_ENV_VAR: &str = "DOJO_KEYSTORE_PASSWORD";
const DOJO_ACCOUNT_ADDRESS_ENV_VAR: &str = "DOJO_ACCOUNT_ADDRESS";
