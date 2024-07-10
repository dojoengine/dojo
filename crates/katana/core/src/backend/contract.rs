use katana_primitives::class::DeprecatedCompiledClass;
use starknet::core::types::FlattenedSierraClass;

#[derive(Debug)]
pub enum StarknetContract {
    Legacy(DeprecatedCompiledClass),
    Sierra(FlattenedSierraClass),
}
