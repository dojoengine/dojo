use blockifier::execution::contract_class::{ContractClass, ContractClassV0, ContractClassV1};
use lazy_static::lazy_static;
use starknet_api::hash::StarkFelt;
use starknet_api::stark_felt;

pub const DEFAULT_GAS_PRICE: u128 = 100 * u128::pow(10, 9); // Given in units of wei.

pub const DEFAULT_INVOKE_MAX_STEPS: u32 = 1_000_000;
pub const DEFAULT_VALIDATE_MAX_STEPS: u32 = 1_000_000;

lazy_static! {

    // Predefined contract addresses

    pub static ref SEQUENCER_ADDRESS: StarkFelt = stark_felt!("0x69420");
    pub static ref UDC_ADDRESS: StarkFelt = stark_felt!("0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf");
    pub static ref FEE_TOKEN_ADDRESS: StarkFelt = stark_felt!("0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7");
    pub static ref TICKER_CONTRACT_ADDRESS: StarkFelt = stark_felt!("0x71C");

    // Predefined class hashes

    pub static ref DEFAULT_ACCOUNT_CONTRACT_CLASS_HASH: StarkFelt = stark_felt!("0x04d07e40e93398ed3c76981e72dd1fd22557a78ce36c0515f679e27f0bb5bc5f");
    pub static ref ACCOUNT_WITHOUT_VALIDATION_CONTRACT_CLASS_HASH: StarkFelt = stark_felt!("0x00b695fb5dd9a4a11deeee1471657e8515219ec61cbc62382d2078efc504ec64");
    pub static ref ERC20_CONTRACT_CLASS_HASH: StarkFelt = stark_felt!("0x02a8846878b6ad1f54f6ba46f5f40e11cee755c677f130b2c4b60566c9003f1f");
    pub static ref UDC_CLASS_HASH: StarkFelt = stark_felt!("0x07b3e05f48f0c69e4a65ce5e076a66271a527aff2c34ce1083ec6e1526997a69");
    pub static ref TICKER_CONTRACT_CLASS_HASH: StarkFelt = stark_felt!("0x03c11e3b183a22d22c6c99a2c9e5262dce98edd9a7c00f58301aba11cd954935");

    // Predefined contract classes

    pub static ref ERC20_CONTRACT: ContractClass = get_contract_class(include_str!("../contracts/compiled/erc20.json"));
    pub static ref UDC_CONTRACT: ContractClass = get_contract_class(include_str!("../contracts/compiled/universal_deployer.json"));
    pub static ref DEFAULT_ACCOUNT_CONTRACT: ContractClass = get_contract_class(include_str!("../contracts/compiled/account.json"));
    pub static ref ACCOUNT_WITHOUT_VALIDATION_CONTRACT: ContractClass = get_contract_class(include_str!("../contracts/compiled/account_without_validation.json"));
    pub static ref TICKER_CONTRACT: ContractClass = get_contractv1_class(include_str!("../contracts/compiled/ticker_Ticker.casm.json"));

    pub static ref DEFAULT_PREFUNDED_ACCOUNT_BALANCE: StarkFelt = stark_felt!("0x3635c9adc5dea00000"); // 10^21
}

fn get_contract_class(contract_class_str: &str) -> ContractClass {
    let legacy_contract_class: ContractClassV0 = serde_json::from_str(contract_class_str).unwrap();
    ContractClass::V0(legacy_contract_class)
}

fn get_contractv1_class(contract_casm_class_str: &str) -> ContractClass {
    let contract_class = ContractClassV1::try_from_json_string(contract_casm_class_str).unwrap();
    ContractClass::V1(contract_class)
}
