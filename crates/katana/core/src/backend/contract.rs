use katana_primitives::class::DeprecatedCompiledClass;
use starknet::core::types::FlattenedSierraClass;

pub enum StarknetContract {
    Legacy(DeprecatedCompiledClass),
    Sierra(FlattenedSierraClass),
}
