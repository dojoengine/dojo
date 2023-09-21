use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
use dojo_examples::components::{Direction};

// trait: specify functions to implement
#[starknet::interface]
trait IPlayerActions<TContractState> {
    fn spawn(self: @TContractState, world: IWorldDispatcher);
    fn move(self: @TContractState, world: IWorldDispatcher, direction: Direction);
}

// exact same functionality as examples/ecs/src/systems/with_decorator.cairo
// requires some additional code without using system decorator
#[starknet::contract]
mod player_actions {
    use starknet::{ContractAddress, get_caller_address};
    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
    use dojo_examples::components::{Position, Moves, Direction};
    use dojo_examples::utils::next_position;
    use super::IPlayerActions;

    #[storage]
    struct Storage {}

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
        fn spawn(self: @ContractState, world: IWorldDispatcher) {
            let player = get_caller_address();
            let position = get!(world, player, (Position));
            set!(
                world,
                (
                    Moves { player, remaining: 10, last_direction: Direction::None(()) },
                    Position { player, x: position.x + 10, y: position.y + 10 },
                )
            );
        }

        fn move(self: @ContractState, world: IWorldDispatcher, direction: Direction) {
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
    use core::traits::Into;
    use array::{ArrayTrait};

    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
    use dojo::test_utils::{spawn_test_world, deploy_contract};

    use dojo_examples::components::{position, moves};
    use dojo_examples::components::{Position, Moves, Direction};

    use super::{IPlayerActionsDispatcher, IPlayerActionsDispatcherTrait, player_actions};

    #[test]
    #[available_gas(30000000)]
    fn test_move() {
        let caller = starknet::contract_address_const::<0x0>();

        // components
        let mut components = array![position::TEST_CLASS_HASH, moves::TEST_CLASS_HASH,];
        // deploy world with components
        let world = spawn_test_world(components);

        // deploy systems contract
        let contract_address = deploy_contract(player_actions::TEST_CLASS_HASH, array![].span());
        let player_actions_system = IPlayerActionsDispatcher { contract_address };

        // System calls
        player_actions_system.spawn(world);
        player_actions_system.move(world, Direction::Right(()));

        let mut keys = array![caller.into()];

        let moves = world
            .entity('Moves', keys.span(), 0, dojo::StorageSize::<Moves>::unpacked_size());
        assert(*moves[0] == 9, 'moves is wrong');
        assert(*moves[1] == Direction::Right(()).into(), 'last direction is wrong');
        let new_position = world
            .entity('Position', keys.span(), 0, dojo::StorageSize::<Position>::unpacked_size());
        assert(*new_position[0] == 11, 'position x is wrong');
        assert(*new_position[1] == 10, 'position y is wrong');
    }
}
