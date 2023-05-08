use dojo_core::integer::u250;
use dojo_core::string::ShortString;
use dojo_core::serde::SpanSerde;

#[abi]
trait IWorld {
    fn component(name: ShortString) -> starknet::ClassHash;
    fn register_component(class_hash: starknet::ClassHash);
    fn system(name: ShortString) -> starknet::ClassHash;
    fn register_system(class_hash: starknet::ClassHash);
    fn uuid() -> usize;
    fn execute(name: ShortString, execute_calldata: Span<felt252>) -> Span<felt252>;
    fn entity(
        component: ShortString, key: dojo_core::storage::query::Query, offset: u8, length: usize
    ) -> Span<felt252>;
    fn set_entity(
        component: ShortString, key: dojo_core::storage::query::Query, offset: u8, value: Span<felt252>
    );
    fn entities(component: ShortString, partition: u250) -> Array::<u250>;
    fn set_executor(contract_address: starknet::ContractAddress);
    fn delete_entity(component: ShortString, query: Query) {};
}

#[abi]
trait IExecutor {
    fn execute(class_hash: starknet::ClassHash, data: Span<felt252>) -> Span<felt252>;
}

#[abi]
trait IComponent {
    fn name() -> ShortString;
    fn len() -> usize;
}

#[abi]
trait ISystem {
    fn name() -> ShortString;
}

#[abi]
trait IWorldFactory {
    fn set_world(class_hash: starknet::ClassHash);
    fn set_executor(class_hash: starknet::ClassHash);
    fn spawn(name: ShortString, components: Array::<starknet::ClassHash>, systems: Array::<starknet::ClassHash>);
    fn world_class_hash() -> starknet::ClassHash;
    fn executor_address() -> starknet::ContractAddress;
}
