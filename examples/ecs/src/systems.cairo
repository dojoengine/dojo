use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
use dojo_examples::components::{Position, Moves, Direction};
use starknet::{ContractAddress, ClassHash};

#[starknet::interface]
trait IPlayerActions<TContractState> {
    fn spawn(self: @TContractState);
    fn move(self: @TContractState, direction: Direction);
}

#[starknet::contract]
mod player_actions {
    use starknet::{ContractAddress, get_caller_address};
    use super::{IPlayerActions, IWorldDispatcher, IWorldDispatcherTrait};
    use dojo_examples::components::{Position, Moves, Direction};

    #[storage]
    struct Storage {
        world: IWorldDispatcher
    }

    #[constructor]
    fn constructor(ref self: ContractState, world_address: ContractAddress) {
        self.world.write(IWorldDispatcher { contract_address: world_address });
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


    #[external(v0)]
    impl PlayerActionsImpl of super::IPlayerActions<ContractState> {
        fn spawn(self: @ContractState) {
            let player = get_caller_address();
            let position = get!(self.world.read(), player, (Position));
            set!(
                self.world.read(),
                (
                    Moves { player, remaining: 10, last_direction: Direction::None(()) },
                    Position { player, x: position.x + 10, y: position.y + 10 },
                )
            );
        }

        fn move(self: @ContractState, direction: Direction) {
            let player = get_caller_address();
            let (mut position, mut moves) = get!(self.world.read(), player, (Position, Moves));
            moves.remaining -= 1;
            moves.last_direction = direction;
            let next = next_position(position, direction);
            set!(self.world.read(), (moves, next));
            emit!(self.world.read(), Moved { player, direction });
            return ();
        }
    }

    fn next_position(mut position: Position, direction: Direction) -> Position {
        match direction {
            Direction::None(()) => {
                return position;
            },
            Direction::Left(()) => {
                position.x -= 1;
            },
            Direction::Right(()) => {
                position.x += 1;
            },
            Direction::Up(()) => {
                position.y -= 1;
            },
            Direction::Down(()) => {
                position.y += 1;
            },
        };

        position
    }
}

#[cfg(test)]
mod tests {
    use core::traits::Into;
    use array::ArrayTrait;

    use dojo::world::IWorldDispatcherTrait;

    use dojo::test_utils::{spawn_test_world, deploy_with_world_address};

    use dojo_examples::components::position;
    use dojo_examples::components::Position;
    use dojo_examples::components::moves;
    use dojo_examples::components::Moves;
    use super::{player_actions, IPlayerActionsDispatcher, IPlayerActionsDispatcherTrait, Direction};

    #[test]
    #[available_gas(30000000)]
    fn test_move() {
        let caller = starknet::contract_address_const::<0x0>();

        // components
        let mut components = array![position::TEST_CLASS_HASH, moves::TEST_CLASS_HASH,];
        // deploy world with components
        let world = spawn_test_world(components);

        // deploy systems contract
        let contract_address = deploy_with_world_address(player_actions::TEST_CLASS_HASH, world);
        let player_actions_system = IPlayerActionsDispatcher { contract_address };

        // System calls
        player_actions_system.spawn();
        player_actions_system.move(Direction::Right(()).into());

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
