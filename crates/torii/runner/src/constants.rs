use starknet::macros::felt;
use starknet_crypto::Felt;

pub(crate) const LOG_TARGET: &str = "torii:runner";

pub(crate) const CARTRIDGE_PAYMASTER_EUWEST3_ADDRESS: Felt = felt!("0x359a81f67140632ec91c7f9af3fc0b5bca0a898ae0be3f7682585b0f40119a7");
pub(crate) const CARTRIDGE_PAYMASTER_SEA1_ADDRESS: Felt = felt!("0x07a0f23c43a291282d093e85f7fb7c0e23a66d02c10fead324ce4c3d56c4bd67");
pub(crate) const CARTRIDGE_PAYMASTER_USEAST4_ADDRESS: Felt = felt!("0x2d2e564dd4faa14277fefd0d8cb95e83b13c0353170eb6819ec35bf1bee8e2a");
