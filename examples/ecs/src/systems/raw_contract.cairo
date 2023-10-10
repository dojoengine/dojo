use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
use dojo_examples::models::{Direction};

// trait: specify functions to implement
#[starknet::interface]
trait IPlayerActions<TContractState> {
    fn spawn(self: @TContractState);
    fn move(self: @TContractState, direction: Direction);
}

// exact same functionality as examples/ecs/src/systems/with_decorator.cairo
// requires some additional code without using system decorator
#[starknet::contract]
mod player_actions_external {
    use starknet::{ContractAddress, get_caller_address};
    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
    use dojo_examples::models::{Position, Moves, Direction, Vec2};
    use dojo_examples::utils::next_position;
    use super::IPlayerActions;

    #[storage]
    struct Storage {
        world_dispatcher: IWorldDispatcher,
    }

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        Moved: Moved,
    }

    #[derive(Drop, starknet::Event)]
    struct Moved {
        player: ContractAddress,
        direction: Direction
    }

    // impl: implement functions specified in trait
    #[external(v0)]
    impl PlayerActionsImpl of IPlayerActions<ContractState> {
        fn spawn(self: @ContractState) {
            let world = self.world_dispatcher.read();
            let player = get_caller_address();
            let position = get!(world, player, (Position));
            set!(
                world,
                (
                    Moves { player, remaining: 10, last_direction: Direction::None(()) },
                    Position { player, vec: Vec2 { x: 10, y: 10 } },
                )
            );
        }

        fn move(self: @ContractState, direction: Direction) {
            let world = self.world_dispatcher.read();
            let player = get_caller_address();
            let (mut position, mut moves) = get!(world, player, (Position, Moves));
            moves.remaining -= 1;
            moves.last_direction = direction;
            let next = next_position(position, direction);
            set!(world, (moves, next));
            emit!(world, Moved { player, direction });
            return ();
        }
    }
}

#[cfg(test)]
mod tests {
    use starknet::class_hash::Felt252TryIntoClassHash;

    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
    use dojo::test_utils::{spawn_test_world, deploy_contract};

    use dojo_examples::models::{position, moves};
    use dojo_examples::models::{Position, Moves, Direction, Vec2};

    use super::{
        IPlayerActionsDispatcher, IPlayerActionsDispatcherTrait,
        player_actions_external as player_actions
    };

    #[test]
    #[available_gas(30000000)]
    fn test_move() {
        let caller = starknet::contract_address_const::<0x0>();

        // models
        let mut models = array![position::TEST_CLASS_HASH, moves::TEST_CLASS_HASH,];
        // deploy world with models
        let world = spawn_test_world(models);

        // deploy systems contract
        let contract_address = world
            .deploy_contract('salt', player_actions::TEST_CLASS_HASH.try_into().unwrap());
        let player_actions_system = IPlayerActionsDispatcher { contract_address };

        // System calls
        player_actions_system.spawn();
        player_actions_system.move(Direction::Right(()));

        let moves = get!(world, caller, Moves);
        let right_dir_felt: felt252 = Direction::Right(()).into();

        assert(moves.remaining == 9, 'moves is wrong');
        assert(moves.last_direction.into() == right_dir_felt, 'last direction is wrong');

        let new_position = get!(world, caller, Position);
        assert(new_position.vec.x == 11, 'position x is wrong');
        assert(new_position.vec.y == 10, 'position y is wrong');
    }
}
