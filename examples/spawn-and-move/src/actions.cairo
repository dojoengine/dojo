use dojo_examples::models::{Direction, Position, Vec2};

#[dojo::interface]
trait IActions {
    fn spawn(ref world: IWorldDispatcher);
    fn move(ref world: IWorldDispatcher, direction: Direction);
    fn set_player_config(ref world: IWorldDispatcher, name: ByteArray);
    fn get_player_position(world: @IWorldDispatcher) -> Position;
    fn update_player_name(ref world: IWorldDispatcher, name: ByteArray);
    fn update_player_name_value(ref world: IWorldDispatcher, name: ByteArray);
    fn reset_player_config(ref world: IWorldDispatcher);
    fn set_player_server_profile(ref world: IWorldDispatcher, server_id: u32, name: ByteArray);
    #[cfg(feature: 'dungeon')]
    fn enter_dungeon(ref world: IWorldDispatcher, dungeon_address: starknet::ContractAddress);
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
    use dojo_examples::models::{
        Position, Moves, Direction, Vec2, PlayerConfig, PlayerItem, ServerProfile, PositionTrait,
        MovesTrait, MovesEntityTrait, PlayerConfigTrait, PlayerConfigEntityTrait
    };
    use dojo_examples::utils::next_position;

    // Featues can be used on modules, structs, trait and `use`. Not inside
    // a function.
    #[cfg(feature: 'dungeon')]
    use dojo_examples::dungeon::{IDungeonDispatcher, IDungeonDispatcherTrait};
    #[cfg(feature: 'dungeon')]
    use armory::Flatbow;
    #[cfg(feature: 'dungeon')]
    use bestiary::RiverSkale;

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
        fn spawn(ref world: IWorldDispatcher) {
            let player = get_caller_address();
            self.set_default_position(player, world);
        }

        fn move(ref world: IWorldDispatcher, direction: Direction) {
            let player = get_caller_address();

            // instead of using the `get!` macro, you can directly use
            // the <ModelName>Trait::get method
            let mut position = PositionTrait::get(world, player);

            // you can also get entity values by entity ID with the `<ModelName>EntityTrait` trait.
            // Note that it returns a `<ModelName>Entity` struct which contains
            // model values and the entity ID.
            let move_id = MovesTrait::entity_id_from_keys(player);
            let mut moves = MovesEntityTrait::get(world, move_id);

            moves.remaining -= 1;
            moves.last_direction = direction;
            let next = next_position(position, direction);

            // instead of using the `set!` macro, you can directly use
            // the <ModelName>Trait::set method
            next.set(world);

            // you can also update entity values by entity ID with the `<ModelName>EntityTrait` trait.
            moves.update(world);

            emit!(world, (Moved { player, direction }));
        }

        fn set_player_config(ref world: IWorldDispatcher, name: ByteArray) {
            let player = get_caller_address();

            let items = array![
                PlayerItem { item_id: 1, quantity: 100, score: 10 },
                PlayerItem { item_id: 2, quantity: 50, score: -32 }
            ];

            let config = PlayerConfig { player, name, items, favorite_item: Option::Some(1), };

            set!(world, (config));
        }

        fn reset_player_config(ref world: IWorldDispatcher) {
            let player = get_caller_address();

            let (position, moves) = get!(world, player, (Position, Moves));
            let config = PlayerConfigTrait::get(world, player);

            delete!(world, (position, moves));
            config.delete(world);

            let (position, moves, config) = get!(world, player, (Position, Moves, PlayerConfig));

            assert(moves.remaining == 0, 'bad remaining');
            assert(moves.last_direction == Direction::None, 'bad last direction');

            assert(position.vec.x == 0, 'bad x');
            assert(position.vec.y == 0, 'bad y');

            assert(config.items.len() == 0, 'bad items');
            assert(config.favorite_item == Option::Some(0), 'bad favorite item');
            let empty_string: ByteArray = "";
            assert(config.name == empty_string, 'bad name');
        }

        fn set_player_server_profile(ref world: IWorldDispatcher, server_id: u32, name: ByteArray) {
            let player = get_caller_address();
            set!(world, ServerProfile { player, server_id, name });
        }

        fn get_player_position(world: @IWorldDispatcher) -> Position {
            let player = get_caller_address();
            get!(world, player, (Position))
        }

        #[cfg(feature: 'dungeon')]
        fn enter_dungeon(ref world: IWorldDispatcher, dungeon_address: ContractAddress) {
            let flatbow = Flatbow { id: 1, atk_speek: 2, range: 1 };
            let river_skale = RiverSkale { id: 1, health: 5, armor: 3, attack: 2 };

            set!(world, (flatbow, river_skale));

            IDungeonDispatcher { contract_address: dungeon_address }.enter();
        }

        fn update_player_name(ref world: IWorldDispatcher, name: ByteArray) {
            let player = get_caller_address();
            let config = PlayerConfigTrait::get(world, player);
            config.set_name(world, name.clone());

            let new_name = PlayerConfigTrait::get_name(world, player);
            assert(new_name == name, 'unable to change name');
        }

        fn update_player_name_value(ref world: IWorldDispatcher, name: ByteArray) {
            let player = get_caller_address();
            let config_id = PlayerConfigTrait::entity_id_from_keys(player);

            let config = PlayerConfigEntityTrait::get(world, config_id);
            config.set_name(world, name.clone());

            let new_name = PlayerConfigEntityTrait::get_name(world, config_id);
            assert(new_name == name, 'unable to change name');
        }
    }

    // The `generate_trait` attribute is not compatible with `world` parameter expansion.
    // Hence, the use of `self` to access the contract state.
    #[generate_trait]
    impl InternalImpl of InternalUtils {
        fn set_default_position(
            self: @ContractState, player: ContractAddress, world: IWorldDispatcher
        ) {
            // The world is always accessible from `self` inside a `dojo::contract`.
            // let world = self.world();

            set!(
                world,
                (
                    Moves { player, remaining: 99, last_direction: Direction::None },
                    Position { player, vec: Vec2 { x: 10, y: 10 } },
                )
            );
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
        let world = spawn_test_world("dojo_examples", models);

        // deploy systems contract
        let contract_address = world
            .deploy_contract('salt', actions::TEST_CLASS_HASH.try_into().unwrap(), array![].span());
        let actions_system = IActionsDispatcher { contract_address };

        // System calls
        actions_system.spawn();
        let initial_moves = get!(world, caller, Moves);
        let initial_position = get!(world, caller, Position);

        assert(
            initial_position.vec.x == 10 && initial_position.vec.y == 10, 'wrong initial position'
        );

        actions_system.move(Direction::Right(()));

        let moves = get!(world, caller, Moves);
        let right_dir_felt: felt252 = Direction::Right(()).into();

        assert(moves.remaining == initial_moves.remaining - 1, 'moves is wrong');
        assert(moves.last_direction.into() == right_dir_felt, 'last direction is wrong');

        let new_position = get!(world, caller, Position);
        assert(new_position.vec.x == initial_position.vec.x + 1, 'position x is wrong');
        assert(new_position.vec.y == initial_position.vec.y, 'position y is wrong');
    }
}
