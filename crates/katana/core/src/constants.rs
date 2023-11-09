use blockifier::execution::contract_class::ContractClass;
use lazy_static::lazy_static;
use starknet_api::hash::StarkFelt;
use starknet_api::stark_felt;
use starknet_api::state::StorageKey;

use crate::utils::contract::{get_contract_class, get_v1_contract_class};

pub const DEFAULT_GAS_PRICE: u128 = 100 * u128::pow(10, 9); // Given in units of wei.

pub const DEFAULT_INVOKE_MAX_STEPS: u32 = 1_000_000;
pub const DEFAULT_VALIDATE_MAX_STEPS: u32 = 1_000_000;

lazy_static! {

    // Predefined contract addresses

    pub static ref SEQUENCER_ADDRESS: StarkFelt = stark_felt!("0x69420");
    pub static ref UDC_ADDRESS: StarkFelt = stark_felt!("0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf");
    pub static ref FEE_TOKEN_ADDRESS: StarkFelt = stark_felt!("0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7");

    // Predefined class hashes

    pub static ref DEFAULT_ACCOUNT_CONTRACT_CLASS_HASH: StarkFelt = stark_felt!("0x00d582780834aefc0a0bf84909d9be00cda0b657d607c7a856a142ce652c23c3");
    pub static ref NO_VALIDATE_ACCOUNT_CONTRACT_CLASS_HASH: StarkFelt = stark_felt!("0x00d582780834aefc0a0bf84909d9be00cda0b657d607c7a856a142ce652c23c3");
    pub static ref ERC20_CONTRACT_CLASS_HASH: StarkFelt = stark_felt!("0x02a8846878b6ad1f54f6ba46f5f40e11cee755c677f130b2c4b60566c9003f1f");
    pub static ref UDC_CLASS_HASH: StarkFelt = stark_felt!("0x07b3e05f48f0c69e4a65ce5e076a66271a527aff2c34ce1083ec6e1526997a69");

    // Predefined contract classes

    pub static ref ERC20_CONTRACT: ContractClass = get_contract_class(include_str!("../contracts/compiled/erc20.json"));
    pub static ref UDC_CONTRACT: ContractClass = get_contract_class(include_str!("../contracts/compiled/universal_deployer.json"));
    pub static ref DEFAULT_ACCOUNT_CONTRACT: ContractClass = get_v1_contract_class(include_str!("../contracts/compiled/openzeppelin_Account.casm.json"));
    pub static ref NO_VALIDATE_ACCOUNT_CONTRACT: ContractClass = get_contract_class(include_str!("../contracts/compiled/account_without_validation.casm.json"));

    pub static ref DEFAULT_PREFUNDED_ACCOUNT_BALANCE: StarkFelt = stark_felt!("0x3635c9adc5dea00000"); // 10^21

    // Storage slots

    pub static ref ERC20_NAME_STORAGE_SLOT: StorageKey = stark_felt!("0x0341c1bdfd89f69748aa00b5742b03adbffd79b8e80cab5c50d91cd8c2a79be1").try_into().unwrap();
    pub static ref ERC20_SYMBOL_STORAGE_SLOT: StorageKey = stark_felt!("0x00b6ce5410fca59d078ee9b2a4371a9d684c530d697c64fbef0ae6d5e8f0ac72").try_into().unwrap();
    pub static ref ERC20_DECIMALS_STORAGE_SLOT: StorageKey = stark_felt!("0x01f0d4aa99431d246bac9b8e48c33e888245b15e9678f64f9bdfc8823dc8f979").try_into().unwrap();
}
