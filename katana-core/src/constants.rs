pub const DEFAULT_GAS_PRICE: u128 = 100 * u128::pow(10, 9); // Given in units of wei.

pub const SEQUENCER_ADDRESS: &str = "0x69";

pub const FEE_ERC20_CONTRACT_ADDRESS: &str =
    "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7";

pub const UNIVERSAL_DEPLOYER_CONTRACT_ADDRESS: &str =
    "0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf";

pub const ACCOUNT_CONTRACT_CLASS_HASH: &str = "0x100";

pub const ERC20_CONTRACT_CLASS_HASH: &str = "0x200";

pub const UNIVERSAL_DEPLOYER_CLASS_HASH: &str = "0x300";

pub const DEFAULT_PREFUNDED_ACCOUNT_BALANCE: &str = "0x3635c9adc5dea00000"; // 10^21

pub const ACCOUNT_CONTRACT_PATH: &str = "./contracts/compiled/account.json";

pub const TEST_ACCOUNT_CONTRACT_PATH: &str = "./contracts/compiled/account_without_validation.json";

pub const ERC20_CONTRACT_PATH: &str = "./contracts/compiled/erc20.json";

pub const UNIVERSAL_DEPLOYER_CONTRACT_PATH: &str = "./contracts/compiled/universal_deployer.json";
