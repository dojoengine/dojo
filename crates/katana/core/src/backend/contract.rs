use katana_primitives::class::LegacyContractClass;
use starknet::core::types::FlattenedSierraClass;

#[derive(Debug)]
pub enum StarknetContract {
    Legacy(LegacyContractClass),
    Sierra(FlattenedSierraClass),
}
