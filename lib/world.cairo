#[contract]
mod World {
    struct Storage {}

    #[event]
    fn ComponentValueSet(component_id: felt, entity_id: felt, data: Array::<T>) {}

    #[external]
    fn register(class_hash: felt) {
        
    }
}
