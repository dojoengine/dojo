use katana_primitives::contract::DeprecatedCompiledClass;
use starknet::core::types::FlattenedSierraClass;

pub enum StarknetContract {
    Legacy(DeprecatedCompiledClass),
    Sierra(FlattenedSierraClass),
}
