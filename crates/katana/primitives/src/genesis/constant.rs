use lazy_static::lazy_static;
use starknet::core::utils::get_storage_var_address;

use crate::contract::{
    ClassHash, CompiledClass, CompiledClassHash, ContractAddress, SierraClass, StorageKey,
};
use crate::utils::class::{parse_compiled_class, parse_sierra_class};
use crate::FieldElement;

/// The default universal deployer contract address.
/// Corresponds to 0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf
pub const DEFAULT_UDC_ADDRESS: ContractAddress = ContractAddress(FieldElement::from_mont([
    15144800532519055890,
    15685625669053253235,
    9333317513348225193,
    121672436446604875,
]));

/// The default fee token contract address.
/// Corresponds to 0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7
pub const DEFAULT_FEE_TOKEN_ADDRESS: ContractAddress = ContractAddress(FieldElement::from_mont([
    4380532846569209554,
    17839402928228694863,
    17240401758547432026,
    418961398025637529,
]));

/// The standard storage address for `public key` in OpenZeppelin account contract.
/// Corresponds to keccak("Account_public_key") ==
/// 0x1379ac0624b939ceb9dede92211d7db5ee174fe28be72245b0a1a2abd81c98f
pub const OZ_ACCOUNT_CONTRACT_PUBKEY_STORAGE_SLOT: StorageKey = FieldElement::from_mont([
    1846274790623946671,
    4425861538928837650,
    5862944509338977095,
    333410775162302292,
]);

/// The standard storage address for `ERC20_name` in ERC20 contract.
/// Corresponds to keccak("ERC20_name") ==
/// 0x0341c1bdfd89f69748aa00b5742b03adbffd79b8e80cab5c50d91cd8c2a79be1
pub const ERC20_NAME_STORAGE_SLOT: StorageKey = FieldElement::from_mont([
    14245373332046594057,
    5531369627875559154,
    7076950148258849527,
    42006782206471821,
]);

/// The standard storage address for `ERC20_symbol` in ERC20 contract.
/// Corresponds to keccak("ERC20_symbol") ==
/// 0x00b6ce5410fca59d078ee9b2a4371a9d684c530d697c64fbef0ae6d5e8f0ac72
pub const ERC20_SYMBOL_STORAGE_SLOT: StorageKey = FieldElement::from_mont([
    3529993699915368059,
    8508842680170426599,
    11308853324722862885,
    140787116910459578,
]);

/// The standard storage address for `ERC20_decimals` in ERC20 contract.
/// Corresponds to keccak("ERC20_decimals") ==
/// 0x01f0d4aa99431d246bac9b8e48c33e888245b15e9678f64f9bdfc8823dc8f979
pub const ERC20_DECIMAL_STORAGE_SLOT: StorageKey = FieldElement::from_mont([
    1858031281331058897,
    9678267618682904527,
    9433316002840757017,
    17823060228645335,
]);

/// The standard storage address for `ERC20_total_supply` in ERC20 contract.
/// Corresponds to keccak("ERC20_total_supply") ==
/// 0x110e2f729c9c2b988559994a3daccd838cf52faf88e18101373e67dd061455a
pub const ERC20_TOTAL_SUPPLY_STORAGE_SLOT: StorageKey = FieldElement::from_mont([
    700008926920971440,
    9682182591019764224,
    8184857487847920423,
    218835885563775175,
]);

/// The default fee token balance for dev accounts at genesis.
pub const DEFAULT_PREFUNDED_ACCOUNT_BALANCE: u128 = 10 * u128::pow(10, 21);

/// The class hash of DEFAULT_LEGACY_ERC20_CONTRACT_CASM.
/// Corresponds to 0x02a8846878b6ad1f54f6ba46f5f40e11cee755c677f130b2c4b60566c9003f1f
pub const DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH: ClassHash = FieldElement::from_mont([
    5063404709606896214,
    17264546324508274858,
    2617848339092803640,
    396742056646423680,
]);

/// The compiled class hash of DEFAULT_LEGACY_ERC20_CONTRACT_CASM.
pub const DEFAULT_LEGACY_ERC20_CONTRACT_COMPILED_CLASS_HASH: CompiledClassHash =
    DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH;

/// The class hash of DEFAULT_LEGACY_UDC_CASM.
/// Corresponds to 0x07b3e05f48f0c69e4a65ce5e076a66271a527aff2c34ce1083ec6e1526997a69
pub const DEFAULT_LEGACY_UDC_CLASS_HASH: ClassHash = FieldElement::from_mont([
    13364470047046544565,
    11148922744554181574,
    8853940111481564631,
    179653587345909319,
]);

/// The compiled class hash of DEFAULT_LEGACY_UDC_CASM.
pub const DEFAULT_LEGACY_UDC_COMPILED_CLASS_HASH: CompiledClassHash = DEFAULT_LEGACY_UDC_CLASS_HASH;

/// The class hash of DEFAULT_OZ_ACCOUNT_CONTRACT.
/// Corresponds to 0x05400e90f7e0ae78bd02c77cd75527280470e2fe19c54970dd79dc37a9d3645c
pub const DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH: ClassHash = FieldElement::from_mont([
    8460675502047588988,
    17729791148444280953,
    7171298771336181387,
    292243705759714441,
]);

/// The compiled class hash of DEFAULT_OZ_ACCOUNT_CONTRACT.
/// Corresponds to 0x016c6081eb34ad1e0c5513234ed0c025b3c7f305902d291bad534cd6474c85bc
pub const DEFAULT_OZ_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH: CompiledClassHash =
    FieldElement::from_mont([
        18006730038010879891,
        12016093095874787527,
        4539661479059859683,
        190499602541245794,
    ]);

// Pre-compiled contract classes
lazy_static! {

    // Default fee token contract
    // pub static ref DEFAULT_LEGACY_ERC20_CONTRACT_CASM: CompiledContractClass = parse_compiled_class(include_str!("../../contracts/compiled/erc20.json")).unwrap();
    pub static ref DEFAULT_LEGACY_ERC20_CONTRACT_CASM: CompiledClass = read_compiled_class_artifact(include_str!("../../contracts/compiled/erc20.json"));

    // Default universal deployer
    pub static ref DEFAULT_LEGACY_UDC_CASM: CompiledClass = read_compiled_class_artifact(include_str!("../../contracts/compiled/universal_deployer.json"));

    // Default account contract
    pub static ref DEFAULT_OZ_ACCOUNT_CONTRACT: SierraClass = parse_sierra_class(include_str!("../../contracts/compiled/oz_account_080.json")).unwrap();
    pub static ref DEFAULT_OZ_ACCOUNT_CONTRACT_CASM: CompiledClass = read_compiled_class_artifact(include_str!("../../contracts/compiled/oz_account_080.json"));

}

/// A helper function to get the base storage address for the fee token balance of a given account.
///
/// This is to compute the base storage address of the balance because the fee token balance is
/// stored as a U256 value and as such has to be split into two U128 values (low and high).
pub(super) fn get_fee_token_balance_base_storage_address(address: ContractAddress) -> FieldElement {
    get_storage_var_address("ERC20_balances", &[address.into()]).unwrap()
}

fn read_compiled_class_artifact(artifact: &str) -> CompiledClass {
    let value = serde_json::from_str(artifact).unwrap();
    parse_compiled_class(value).unwrap()
}
