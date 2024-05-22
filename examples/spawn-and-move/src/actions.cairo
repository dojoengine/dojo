use dojo_examples::models::{Direction, Position, Vec2};

#[dojo::interface]
trait IActions {
    fn spawn();
    fn move(direction: Direction);
    fn set_player_config(name: ByteArray);
}

#[dojo::interface]
trait IActionsComputed {
    fn tile_terrain(vec: Vec2) -> felt252;
    fn quadrant(pos: Position) -> u8;
}

#[dojo::contract]
mod actions {
    use super::IActions;
    use super::IActionsComputed;

    use starknet::{ContractAddress, get_caller_address};
    use dojo_examples::models::{Position, Moves, Direction, Vec2, PlayerConfig, PlayerItem};
    use dojo_examples::utils::next_position;

    #[derive(Copy, Drop, Serde)]
    #[dojo::event]
    #[dojo::model]
    struct Moved {
        #[key]
        player: ContractAddress,
        direction: Direction,
    }

    #[abi(embed_v0)]
    impl ActionsComputedImpl of IActionsComputed<ContractState> {
        #[computed]
        fn tile_terrain(vec: Vec2) -> felt252 {
            'land'
        }

        #[computed(Position)]
        fn quadrant(pos: Position) -> u8 {
            // 10 is zero
            if pos.vec.x < 10 {
                if pos.vec.y < 10 {
                    3 // Quadrant - -
                } else {
                    4 // Quadrant - +
                }
            } else {
                if pos.vec.y < 10 {
                    2 // Quadrant + -
                } else {
                    1 // Quadrant + +
                }
            }
        }
    }

    // impl: implement functions specified in trait
    #[abi(embed_v0)]
    impl ActionsImpl of IActions<ContractState> {
        // ContractState is defined by system decorator expansion
        fn spawn(world: IWorldDispatcher) {
            let player = get_caller_address();
            let position = get!(world, player, (Position));

            set!(
                world,
                (
                    Moves { player, remaining: 99, last_direction: Direction::None(()) },
                    Position {
                        player, vec: Vec2 { x: position.vec.x + 10, y: position.vec.y + 10 }
                    },
                )
            );
        }

        fn move(world: IWorldDispatcher, direction: Direction) {
            let player = get_caller_address();
            let (mut position, mut moves) = get!(world, player, (Position, Moves));
            moves.remaining -= 1;
            moves.last_direction = direction;
            let next = next_position(position, direction);
            set!(world, (moves, next));
            emit!(world, (Moved { player, direction }));
        }

        fn set_player_config(world: IWorldDispatcher, name: ByteArray) {
            let player = get_caller_address();

            let items = array![
                PlayerItem { item_id: 1, quantity: 100 },
                PlayerItem { item_id: 2, quantity: 50 }
            ];

            let config = PlayerConfig {
                player,
                name,
                items,
                favorite_item: Option::Some(1),
            };

            set!(world, (config));
        }
    }
}

#[cfg(test)]
mod tests {
    use starknet::class_hash::Felt252TryIntoClassHash;

    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

    use dojo::test_utils::{spawn_test_world, deploy_contract};

    use super::{actions, IActionsDispatcher, IActionsDispatcherTrait};
    use dojo_examples::models::{Position, position, Moves, moves, Direction, Vec2};

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
            .deploy_contract('salt', actions::TEST_CLASS_HASH.try_into().unwrap());
        let actions_system = IActionsDispatcher { contract_address };

        // System calls
        actions_system.spawn();
        let initial_moves = get!(world, caller, Moves);
        actions_system.move(Direction::Right(()));

        let moves = get!(world, caller, Moves);
        let right_dir_felt: felt252 = Direction::Right(()).into();

        assert(moves.remaining == initial_moves.remaining - 1, 'moves is wrong');
        assert(moves.last_direction.into() == right_dir_felt, 'last direction is wrong');

        let new_position = get!(world, caller, Position);
        assert(new_position.vec.x == 11, 'position x is wrong');
        assert(new_position.vec.y == 10, 'position y is wrong');
    }
}
