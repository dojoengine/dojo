#[starknet::interface]
trait IERC4906<TState> {
    fn emit_metadata_update(ref self: TState, token_id: u256);
    fn emit_batch_metadata_update(ref self: TState, from_token_id: u256, to_token_id: u256);
}

#[starknet::component]
pub mod ERC4906Component {
    use super::IERC4906;

    #[storage]
    pub struct Storage {}

    #[event]
    #[derive(Drop, starknet::Event)]
    pub enum Event {
        BatchMetadataUpdate: BatchMetadataUpdate,
        MetadataUpdate: MetadataUpdate,
    }

    #[derive(Drop, starknet::Event)]
    pub struct BatchMetadataUpdate {
        #[key]
        from_token_id: u256,
        #[key]
        to_token_id: u256,
    }

    #[derive(Drop, starknet::Event)]
    pub struct MetadataUpdate {
        #[key]
        token_id: u256,
    }

    #[embeddable_as(ERC4906Implementation)]
    impl ERC4906<
        TContractState, +HasComponent<TContractState>, +Drop<TContractState>,
    > of IERC4906<ComponentState<TContractState>> {
        fn emit_metadata_update(ref self: ComponentState<TContractState>, token_id: u256) {
            self.emit(Event::MetadataUpdate(MetadataUpdate { token_id }));
        }

        fn emit_batch_metadata_update(
            ref self: ComponentState<TContractState>, from_token_id: u256, to_token_id: u256,
        ) {
            assert(from_token_id <= to_token_id, 'Invalid token range');
            self
                .emit(
                    Event::BatchMetadataUpdate(BatchMetadataUpdate { from_token_id, to_token_id }),
                );
        }
    }
}
