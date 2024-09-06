use dojo_examples::models::{Direction, Position, Vec2, PlayerItem};

#[dojo::interface]
pub trait IActions {
    fn run(ref world: IWorldDispatcher);
}

#[dojo::contract]
pub mod actions {
    use super::IActions;

    use starknet::{ContractAddress, get_caller_address};
    use dojo_examples::models::{
        Position, Moves, Direction, Vec2, PlayerConfig, PlayerItem, ServerProfile, };

    #[abi(embed_v0)]
    impl ActionsImpl of IActions<ContractState> {
        fn run(ref world: IWorldDispatcher) {
            world.register_model(dojo::model::Model::<PlayerConfig>::definition());
        }
    }
}
