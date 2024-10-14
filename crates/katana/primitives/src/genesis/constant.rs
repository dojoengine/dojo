use lazy_static::lazy_static;
use starknet::core::utils::get_storage_var_address;
use starknet::macros::felt;

use crate::class::{ClassHash, CompiledClass, CompiledClassHash, SierraClass};
use crate::contract::{ContractAddress, StorageKey};
use crate::utils::class::{parse_compiled_class, parse_sierra_class};
use crate::Felt;

/// The default universal deployer contract address.
/// Corresponds to 0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf
pub const DEFAULT_UDC_ADDRESS: ContractAddress =
    ContractAddress(felt!("0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf"));

/// The default fee token contract address.
/// Corresponds to 0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7
pub const DEFAULT_FEE_TOKEN_ADDRESS: ContractAddress =
    ContractAddress(felt!("0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7"));

/// The standard storage address for `public key` in the default account class.
/// Corresponds to keccak("Account_public_key") ==
/// 0x1379ac0624b939ceb9dede92211d7db5ee174fe28be72245b0a1a2abd81c98f
pub const DEFAULT_ACCOUNT_CLASS_PUBKEY_STORAGE_SLOT: StorageKey =
    felt!("0x1379ac0624b939ceb9dede92211d7db5ee174fe28be72245b0a1a2abd81c98f");

/// The standard storage address for `ERC20_name` in ERC20 contract.
/// Corresponds to keccak("ERC20_name") ==
/// 0x0341c1bdfd89f69748aa00b5742b03adbffd79b8e80cab5c50d91cd8c2a79be1
pub const ERC20_NAME_STORAGE_SLOT: StorageKey =
    felt!("0x0341c1bdfd89f69748aa00b5742b03adbffd79b8e80cab5c50d91cd8c2a79be1");

/// The standard storage address for `ERC20_symbol` in ERC20 contract.
/// Corresponds to keccak("ERC20_symbol") ==
/// 0x00b6ce5410fca59d078ee9b2a4371a9d684c530d697c64fbef0ae6d5e8f0ac72
pub const ERC20_SYMBOL_STORAGE_SLOT: StorageKey =
    felt!("0x00b6ce5410fca59d078ee9b2a4371a9d684c530d697c64fbef0ae6d5e8f0ac72");

/// The standard storage address for `ERC20_decimals` in ERC20 contract.
/// Corresponds to keccak("ERC20_decimals") ==
/// 0x01f0d4aa99431d246bac9b8e48c33e888245b15e9678f64f9bdfc8823dc8f979
pub const ERC20_DECIMAL_STORAGE_SLOT: StorageKey =
    felt!("0x01f0d4aa99431d246bac9b8e48c33e888245b15e9678f64f9bdfc8823dc8f979");

/// The standard storage address for `ERC20_total_supply` in ERC20 contract.
/// Corresponds to keccak("ERC20_total_supply") ==
/// 0x110e2f729c9c2b988559994a3daccd838cf52faf88e18101373e67dd061455a
pub const ERC20_TOTAL_SUPPLY_STORAGE_SLOT: StorageKey =
    felt!("0x110e2f729c9c2b988559994a3daccd838cf52faf88e18101373e67dd061455a");

/// The default fee token balance for dev accounts at genesis.
pub const DEFAULT_PREFUNDED_ACCOUNT_BALANCE: u128 = 10 * u128::pow(10, 21);

/// The class hash of DEFAULT_LEGACY_ERC20_CONTRACT_CASM.
/// Corresponds to 0x02a8846878b6ad1f54f6ba46f5f40e11cee755c677f130b2c4b60566c9003f1f
pub const DEFAULT_LEGACY_ERC20_CLASS_HASH: ClassHash =
    felt!("0x02a8846878b6ad1f54f6ba46f5f40e11cee755c677f130b2c4b60566c9003f1f");

/// The compiled class hash of DEFAULT_LEGACY_ERC20_CONTRACT_CASM.
pub const DEFAULT_LEGACY_ERC20_COMPILED_CLASS_HASH: CompiledClassHash =
    DEFAULT_LEGACY_ERC20_CLASS_HASH;

/// The class hash of DEFAULT_LEGACY_UDC_CASM.
/// Corresponds to 0x07b3e05f48f0c69e4a65ce5e076a66271a527aff2c34ce1083ec6e1526997a69
pub const DEFAULT_LEGACY_UDC_CLASS_HASH: ClassHash =
    felt!("0x07b3e05f48f0c69e4a65ce5e076a66271a527aff2c34ce1083ec6e1526997a69");

/// The compiled class hash of DEFAULT_LEGACY_UDC_CASM.
pub const DEFAULT_LEGACY_UDC_COMPILED_CLASS_HASH: CompiledClassHash = DEFAULT_LEGACY_UDC_CLASS_HASH;

/// The class hash of [`DEFAULT_ACCOUNT_CLASS`].
pub const DEFAULT_ACCOUNT_CLASS_HASH: ClassHash =
    felt!("0x07dc7899aa655b0aae51eadff6d801a58e97dd99cf4666ee59e704249e51adf2");

/// The compiled class hash of [`DEFAULT_ACCOUNT_CLASS`].
pub const DEFAULT_ACCOUNT_COMPILED_CLASS_HASH: CompiledClassHash =
    felt!("0x01b97e0ef7f5c2f2b7483cda252a3accc7f917773fb69d4bd290f92770069aec");

/// Cartridge Controller account class hash.
pub const CONTROLLER_CLASS_HASH: ClassHash =
    felt!("0x024a9edbfa7082accfceabf6a92d7160086f346d622f28741bf1c651c412c9ab");

// Pre-compiled contract classes
lazy_static! {

    // Default fee token contract
    // pub static ref DEFAULT_LEGACY_ERC20_CONTRACT_CASM: CompiledContractClass = parse_compiled_class(include_str!("../../contracts/compiled/erc20.json")).unwrap();
    pub static ref DEFAULT_LEGACY_ERC20_CASM: CompiledClass = read_compiled_class_artifact(include_str!("../../../contracts/build/erc20.json"));

    // Default universal deployer
    pub static ref DEFAULT_LEGACY_UDC_CASM: CompiledClass = read_compiled_class_artifact(include_str!("../../../contracts/build/universal_deployer.json"));

    // Default account contract
    pub static ref DEFAULT_ACCOUNT_CLASS: SierraClass = parse_sierra_class(include_str!("../../../contracts/build/default_account.json")).unwrap();
    pub static ref DEFAULT_ACCOUNT_CLASS_CASM: CompiledClass = read_compiled_class_artifact(include_str!("../../../contracts/build/default_account.json"));
}

#[cfg(feature = "controller")]
lazy_static! {
    // Cartridge Controller account
    pub static ref CONTROLLER_ACCOUNT_CLASS: SierraClass = parse_sierra_class(include_str!("../../../contracts/build/controller_CartridgeAccount.contract_class.json")).unwrap();
    pub static ref CONTROLLER_ACCOUNT_CLASS_CASM: CompiledClass = read_compiled_class_artifact(include_str!("../../../contracts/build/controller_CartridgeAccount.contract_class.json"));
}

/// A helper function to get the base storage address for the fee token balance of a given account.
///
/// This is to compute the base storage address of the balance because the fee token balance is
/// stored as a U256 value and as such has to be split into two U128 values (low and high).
pub(super) fn get_fee_token_balance_base_storage_address(address: ContractAddress) -> Felt {
    get_storage_var_address("ERC20_balances", &[address.into()]).unwrap()
}

fn read_compiled_class_artifact(artifact: &str) -> CompiledClass {
    let value = serde_json::from_str(artifact).unwrap();
    parse_compiled_class(value).unwrap()
}

#[cfg(test)]
mod tests {

    #[cfg(feature = "controller")]
    #[test]
    fn controller_class_hash() {
        use super::{CONTROLLER_ACCOUNT_CLASS, CONTROLLER_CLASS_HASH};

        let hash = CONTROLLER_ACCOUNT_CLASS.class_hash().unwrap();
        assert_eq!(hash, CONTROLLER_CLASS_HASH)
    }
}
