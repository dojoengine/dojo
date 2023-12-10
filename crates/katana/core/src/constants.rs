use katana_primitives::contract::{
    CompiledContractClass, CompiledContractClassV0, ContractAddress, StorageKey,
};
use katana_primitives::FieldElement;
use lazy_static::lazy_static;
use starknet::macros::felt;

pub const DEFAULT_GAS_PRICE: u128 = 100 * u128::pow(10, 9); // Given in units of wei.

pub const DEFAULT_INVOKE_MAX_STEPS: u32 = 1_000_000;
pub const DEFAULT_VALIDATE_MAX_STEPS: u32 = 1_000_000;

fn parse_legacy_contract_class(content: impl AsRef<str>) -> CompiledContractClass {
    let class: CompiledContractClassV0 = serde_json::from_str(content.as_ref()).unwrap();
    CompiledContractClass::V0(class)
}

lazy_static! {

    // Predefined contract addresses

    pub static ref SEQUENCER_ADDRESS: ContractAddress = ContractAddress(felt!("0x1"));
    pub static ref UDC_ADDRESS: ContractAddress = ContractAddress(felt!("0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf"));
    pub static ref FEE_TOKEN_ADDRESS: ContractAddress = ContractAddress(felt!("0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7"));

    // Predefined class hashes

    pub static ref OZ_V0_ACCOUNT_CONTRACT_CLASS_HASH: FieldElement = felt!("0x04d07e40e93398ed3c76981e72dd1fd22557a78ce36c0515f679e27f0bb5bc5f");
    pub static ref ERC20_CONTRACT_CLASS_HASH: FieldElement = felt!("0x02a8846878b6ad1f54f6ba46f5f40e11cee755c677f130b2c4b60566c9003f1f");
    pub static ref UDC_CLASS_HASH: FieldElement = felt!("0x07b3e05f48f0c69e4a65ce5e076a66271a527aff2c34ce1083ec6e1526997a69");

    pub static ref OZ_V0_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH: FieldElement = felt!("0x04d07e40e93398ed3c76981e72dd1fd22557a78ce36c0515f679e27f0bb5bc5f");
    pub static ref ERC20_CONTRACT_COMPILED_CLASS_HASH: FieldElement = felt!("0x02a8846878b6ad1f54f6ba46f5f40e11cee755c677f130b2c4b60566c9003f1f");
    pub static ref UDC_COMPILED_CLASS_HASH: FieldElement = felt!("0x07b3e05f48f0c69e4a65ce5e076a66271a527aff2c34ce1083ec6e1526997a69");

    // Predefined contract classes

    pub static ref ERC20_CONTRACT: CompiledContractClass = parse_legacy_contract_class(include_str!("../contracts/compiled/erc20.json"));
    pub static ref UDC_CONTRACT: CompiledContractClass = parse_legacy_contract_class(include_str!("../contracts/compiled/universal_deployer.json"));
    pub static ref OZ_V0_ACCOUNT_CONTRACT: CompiledContractClass = parse_legacy_contract_class(include_str!("../contracts/compiled/account.json"));

    pub static ref DEFAULT_PREFUNDED_ACCOUNT_BALANCE: FieldElement = felt!("0x3635c9adc5dea00000"); // 10^21

    // Storage slots

    pub static ref ERC20_NAME_STORAGE_SLOT: StorageKey = felt!("0x0341c1bdfd89f69748aa00b5742b03adbffd79b8e80cab5c50d91cd8c2a79be1");
    pub static ref ERC20_SYMBOL_STORAGE_SLOT: StorageKey = felt!("0x00b6ce5410fca59d078ee9b2a4371a9d684c530d697c64fbef0ae6d5e8f0ac72");
    pub static ref ERC20_DECIMALS_STORAGE_SLOT: StorageKey = felt!("0x01f0d4aa99431d246bac9b8e48c33e888245b15e9678f64f9bdfc8823dc8f979");
}
