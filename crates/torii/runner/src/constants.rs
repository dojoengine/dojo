use starknet::macros::felt;
use starknet_crypto::Felt;

pub(crate) const LOG_TARGET: &str = "torii:runner";

pub(crate) const UDC_ADDRESS: Felt =
    felt!("0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf");
