use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
use dojo_examples::components::{Position, Moves, Direction};
use starknet::{ContractAddress, ClassHash};

// trait: specify functions to implement
#[starknet::interface]
trait IPlayerActions<TContractState> {
    fn spawn(self: @TContractState, world: IWorldDispatcher);
    fn move(self: @TContractState, world: IWorldDispatcher, direction: Direction);
}

#[system]
mod player_actions {
    use starknet::{ContractAddress, get_caller_address};
    use dojo_examples::components::{Position, Moves, Direction, Vec2};
    use dojo_examples::utils::next_position;
    use super::IPlayerActions;

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
        // ContractState is defined by system decorator expansion
        fn spawn(self: @ContractState, world: IWorldDispatcher) {
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
    use dojo_examples::components::{Position, Moves, Direction, Vec2};
    use super::{player_actions, IPlayerActionsDispatcher, IPlayerActionsDispatcherTrait};

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

        let moves = get!(world, caller, Moves);
        let right_dir_felt: felt252 = Direction::Right(()).into();

        assert(moves.remaining == 9, 'moves is wrong');
        assert(moves.last_direction.into() == right_dir_felt, 'last direction is wrong');

        let new_position = get!(world, caller, Position);
        assert(new_position.vec.x == 11, 'position x is wrong');
        assert(new_position.vec.y == 10, 'position y is wrong');
    }
}
