use dojo_examples::models::{Direction, Position, Vec2, PlayerItem};

#[dojo::interface]
pub trait IActions {
    fn spawn(ref world: IWorldDispatcher);
    fn move(ref world: IWorldDispatcher, direction: Direction);
    fn set_player_config(ref world: IWorldDispatcher, name: ByteArray);
    fn get_player_position(world: @IWorldDispatcher) -> Position;
    fn update_player_name(ref world: IWorldDispatcher, name: ByteArray);
    fn update_player_items(ref world: IWorldDispatcher, items: Array<PlayerItem>);
    fn reset_player_config(ref world: IWorldDispatcher);
    fn set_player_server_profile(ref world: IWorldDispatcher, server_id: u32, name: ByteArray);
    fn set_models(ref world: IWorldDispatcher, seed: felt252, n_models: u32);
    #[cfg(feature: 'dungeon')]
    fn enter_dungeon(ref world: IWorldDispatcher, dungeon_address: starknet::ContractAddress);
}

#[dojo::contract]
pub mod actions {
    use super::IActions;

    use starknet::{ContractAddress, get_caller_address};
    use dojo_examples::models::{
        Position, Moves, Direction, Vec2, PlayerConfig, PlayerItem, ServerProfile, PositionStore,
        MovesStore, MovesEntityStore, PlayerConfigStore, PlayerConfigEntityStore,
    };
    use dojo_examples::utils::next_position;

    // Features can be used on modules, structs, trait and `use`. Not inside
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
    pub struct Moved {
        #[key]
        pub player: ContractAddress,
        pub direction: Direction,
    }

    // impl: implement functions specified in trait
    #[abi(embed_v0)]
    impl ActionsImpl of IActions<ContractState> {
        // Set some models randomly.
        fn set_models(ref world: IWorldDispatcher, seed: felt252, n_models: u32) {
            let uint: u256 = seed.into();
            let prng: u32 = (uint % 4_294_967_000).try_into().unwrap();
            let byte: u8 = (uint % 255).try_into().unwrap();

            let moves = Moves {
                player: seed.try_into().unwrap(), remaining: byte, last_direction: Direction::None
            };
            let position = Position {
                player: seed.try_into().unwrap(), vec: Vec2 { x: prng, y: prng }
            };
            let server_profile = ServerProfile {
                player: seed.try_into().unwrap(), server_id: prng, name: "hello"
            };
            let player_config = PlayerConfig {
                player: seed.try_into().unwrap(),
                name: "hello",
                items: array![],
                favorite_item: Option::None
            };

            if n_models == 4 {
                set!(world, (moves, position, server_profile, player_config));
            } else if n_models == 3 {
                set!(world, (moves, position, server_profile));
            } else if n_models == 2 {
                set!(world, (moves, position));
            } else {
                set!(world, (moves));
            }
        }

        // ContractState is defined by system decorator expansion
        fn spawn(ref world: IWorldDispatcher) {
            let player = get_caller_address();
            self.set_default_position(player, world);
        }

        fn move(ref world: IWorldDispatcher, direction: Direction) {
            let player = get_caller_address();

            // instead of using the `get!` macro, you can directly use
            // the <ModelName>Store::get method
            let mut position = PositionStore::get(world, player);

            // you can also get entity values by entity ID with the `<ModelName>EntityStore` trait.
            // Note that it returns a `<ModelName>Entity` struct which contains
            // model values and the entity ID.
            let move_id = MovesStore::entity_id_from_keys(player);
            let mut moves = MovesEntityStore::get(world, move_id);

            moves.remaining -= 1;
            moves.last_direction = direction;
            let next = next_position(position, direction);

            // instead of using the `set!` macro, you can directly use
            // the <ModelName>Store::set method
            next.set(world);

            // you can also update entity values by entity ID with the `<ModelName>EntityStore`
            // trait.
            moves.update(world);

            emit!(world, (Moved { player, direction }));
        }

        fn set_player_config(ref world: IWorldDispatcher, name: ByteArray) {
            let player = get_caller_address();

            let items = array![
                PlayerItem { item_id: 1, quantity: 100, score: 150 },
                PlayerItem { item_id: 2, quantity: 50, score: -32 }
            ];

            let config = PlayerConfig { player, name, items, favorite_item: Option::Some(1), };

            set!(world, (config));
        }

        fn reset_player_config(ref world: IWorldDispatcher) {
            let player = get_caller_address();

            let (position, moves) = get!(world, player, (Position, Moves));
            let config = PlayerConfigStore::get(world, player);

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
            let config = PlayerConfigStore::get(world, player);
            config.set_name(world, name.clone());

            let new_name = PlayerConfigStore::get_name(world, player);
            assert(new_name == name, 'unable to change name');
        }

        fn update_player_items(ref world: IWorldDispatcher, items: Array<PlayerItem>) {
            let player = get_caller_address();
            let config_id = PlayerConfigStore::entity_id_from_keys(player);

            let items_clone = items.clone();

            let config = PlayerConfigEntityStore::get(world, config_id);
            config.set_items(world, items);

            let new_items = PlayerConfigEntityStore::get_items(world, config_id);
            let mut size = items_clone.len();

            while size > 0 {
                assert(new_items.at(size - 1) == items_clone.at(size - 1), 'item not found');
                size -= 1;
            }
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
    use dojo::model::{Model, ModelTest, ModelIndex, ModelEntityTest};
    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

    use dojo::utils::test::deploy_contract;

    use super::{actions, IActionsDispatcher, IActionsDispatcherTrait};
    use armory::flatbow;
    use dojo_examples::models::{
        Position, position, PositionStore, PositionEntityStore, Moves, moves, Direction, Vec2
    };

    #[test]
    fn test_world_test_set() {
        let caller = starknet::contract_address_const::<0x0>();

        let world = spawn_test_world!();

        // Without having the permission, we can set data into the dojo database for the given
        // models.
        let mut position = PositionStore::get(world, caller);
        assert(position.vec.x == 0 && position.vec.y == 0, 'bad x');

        position.vec.x = 122;
        // `set_test` and `delete_test` are available on `Model`.
        // `update_test` and `delete_test` are available on `ModelEntity`.
        position.set_test(world);

        let id = PositionStore::entity_id_from_keys(caller);
        let mut position = PositionEntityStore::get(world, id);
        assert(position.vec.x == 122, 'bad x');

        position.vec.y = 88;
        position.update_test(world);

        let mut position = PositionStore::get(world, caller);
        assert(position.vec.y == 88, 'bad y');

        position.delete_test(world);

        let position = PositionStore::get(world, caller);
        assert(position.vec.x == 0 && position.vec.y == 0, 'bad delete');
    }

    #[test]
    #[available_gas(30000000)]
    fn test_move() {
        let caller = starknet::contract_address_const::<0x0>();

        // deploy world with only the models for the given namespaces.
        let world = spawn_test_world!(["dojo_examples", "dojo_examples_weapons"]);

        // deploy systems contract
        let contract_address = world
            .deploy_contract('salt', actions::TEST_CLASS_HASH.try_into().unwrap());
        let actions_system = IActionsDispatcher { contract_address };

        // set authorizations
        world.grant_writer(Model::<Moves>::selector(), contract_address);
        world.grant_writer(Model::<Position>::selector(), contract_address);

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
