use katana_primitives::contract::{
    CompiledContractClass, ContractAddress, SierraClass, StorageKey,
};
use katana_primitives::utils::class::{parse_compiled_class, parse_sierra_class};
use katana_primitives::FieldElement;
use lazy_static::lazy_static;
use starknet::macros::felt;

pub const DEFAULT_GAS_PRICE: u64 = 100 * u64::pow(10, 9); // Given in units of wei.

pub const DEFAULT_INVOKE_MAX_STEPS: u32 = 1_000_000;
pub const DEFAULT_VALIDATE_MAX_STEPS: u32 = 1_000_000;

pub const MAX_RECURSION_DEPTH: usize = 1000;

lazy_static! {

    // Predefined contract addresses

    pub static ref SEQUENCER_ADDRESS: ContractAddress = ContractAddress(felt!("0x1"));
    pub static ref UDC_ADDRESS: ContractAddress = ContractAddress(felt!("0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf"));
    pub static ref FEE_TOKEN_ADDRESS: ContractAddress = ContractAddress(felt!("0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7"));

    // Predefined class hashes

    pub static ref OZ_V1_ACCOUNT_CONTRACT_CLASS_HASH: FieldElement = felt!("0x05400e90f7e0ae78bd02c77cd75527280470e2fe19c54970dd79dc37a9d3645c");
    pub static ref ERC20_CONTRACT_CLASS_HASH: FieldElement = felt!("0x02a8846878b6ad1f54f6ba46f5f40e11cee755c677f130b2c4b60566c9003f1f");
    pub static ref UDC_CLASS_HASH: FieldElement = felt!("0x07b3e05f48f0c69e4a65ce5e076a66271a527aff2c34ce1083ec6e1526997a69");

    pub static ref OZ_V1_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH: FieldElement = felt!("0x016c6081eb34ad1e0c5513234ed0c025b3c7f305902d291bad534cd6474c85bc");
    pub static ref ERC20_CONTRACT_COMPILED_CLASS_HASH: FieldElement = felt!("0x02a8846878b6ad1f54f6ba46f5f40e11cee755c677f130b2c4b60566c9003f1f");
    pub static ref UDC_COMPILED_CLASS_HASH: FieldElement = felt!("0x07b3e05f48f0c69e4a65ce5e076a66271a527aff2c34ce1083ec6e1526997a69");

    // Predefined contract classes

    pub static ref ERC20_CONTRACT: CompiledContractClass = parse_compiled_class(include_str!("../contracts/compiled/erc20.json")).unwrap();
    pub static ref UDC_CONTRACT: CompiledContractClass = parse_compiled_class(include_str!("../contracts/compiled/universal_deployer.json")).unwrap();
    pub static ref OZ_V1_ACCOUNT_CONTRACT: SierraClass = parse_sierra_class(include_str!("../contracts/compiled/oz_account_080.json")).unwrap();
    pub static ref OZ_V1_ACCOUNT_CONTRACT_COMPILED: CompiledContractClass = parse_compiled_class(include_str!("../contracts/compiled/oz_account_080.json")).unwrap();

    pub static ref DEFAULT_PREFUNDED_ACCOUNT_BALANCE: FieldElement = felt!("0x3635c9adc5dea00000"); // 10^21

    // Storage slots

    pub static ref ERC20_NAME_STORAGE_SLOT: StorageKey = felt!("0x0341c1bdfd89f69748aa00b5742b03adbffd79b8e80cab5c50d91cd8c2a79be1");
    pub static ref ERC20_SYMBOL_STORAGE_SLOT: StorageKey = felt!("0x00b6ce5410fca59d078ee9b2a4371a9d684c530d697c64fbef0ae6d5e8f0ac72");
    pub static ref ERC20_DECIMALS_STORAGE_SLOT: StorageKey = felt!("0x01f0d4aa99431d246bac9b8e48c33e888245b15e9678f64f9bdfc8823dc8f979");
}
