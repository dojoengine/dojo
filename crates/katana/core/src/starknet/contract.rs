use blockifier::execution::contract_class::ContractClassV0;
use starknet::core::types::FlattenedSierraClass;

pub enum StarknetContract {
    Legacy(ContractClassV0),
    Sierra(FlattenedSierraClass),
}
