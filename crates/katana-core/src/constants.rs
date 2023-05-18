use blockifier::execution::contract_class::ContractClass;
use lazy_static::lazy_static;
use starknet_api::hash::StarkFelt;
use starknet_api::stark_felt;

use crate::util::get_contract_class;

pub const DEFAULT_GAS_PRICE: u128 = 100 * u128::pow(10, 9); // Given in units of wei.

// Contract artifacts path

pub const ERC20_CONTRACT_PATH: &str = "./contracts/compiled/erc20.json";
pub const UDC_PATH: &str = "./contracts/compiled/universal_deployer.json";
pub const DEFAULT_ACCOUNT_CONTRACT_PATH: &str = "./contracts/compiled/account.json";
pub const TEST_ACCOUNT_CONTRACT_PATH: &str = "./contracts/compiled/account_without_validation.json";

lazy_static! {

    // Predefined contract addresses

    pub static ref SEQUENCER_ADDRESS: StarkFelt = stark_felt!("0x69420");
    pub static ref UDC_ADDRESS: StarkFelt = stark_felt!("0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf");
    pub static ref FEE_TOKEN_ADDRESS: StarkFelt = stark_felt!("0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7");

    // Predefined class hashes

    pub static ref DEFAULT_ACCOUNT_CONTRACT_CLASS_HASH: StarkFelt = stark_felt!("0x04d07e40e93398ed3c76981e72dd1fd22557a78ce36c0515f679e27f0bb5bc5f");
    pub static ref ERC20_CONTRACT_CLASS_HASH: StarkFelt = stark_felt!("0x02a8846878b6ad1f54f6ba46f5f40e11cee755c677f130b2c4b60566c9003f1f");
    pub static ref UDC_CLASS_HASH: StarkFelt = stark_felt!("0x07b3e05f48f0c69e4a65ce5e076a66271a527aff2c34ce1083ec6e1526997a69");

    // Predefined contract classes

    pub static ref DEFAULT_ACCOUNT_CONTRACT: ContractClass = get_contract_class(DEFAULT_ACCOUNT_CONTRACT_PATH);
    pub static ref TEST_ACCOUNT_CONTRACT: ContractClass = get_contract_class(TEST_ACCOUNT_CONTRACT_PATH);
    pub static ref ERC20_CONTRACT: ContractClass = get_contract_class(ERC20_CONTRACT_PATH);
    pub static ref UDC_CONTRACT: ContractClass = get_contract_class(UDC_PATH);

    pub static ref DEFAULT_PREFUNDED_ACCOUNT_BALANCE: StarkFelt = stark_felt!("0x3635c9adc5dea00000"); // 10^21
}
