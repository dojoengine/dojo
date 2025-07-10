use dojo_examples::models::{Direction, Position};

#[starknet::interface]
pub trait IActions<T> {
    fn spawn(ref self: T);
    fn move(ref self: T, direction: Direction);
    fn set_player_config(ref self: T, name: ByteArray);
    fn update_player_config_items(ref self: T);
    fn update_player_config_name(ref self: T, name: ByteArray);
    fn get_player_position(self: @T) -> Position;
    fn reset_player_config(ref self: T);
    fn set_player_server_profile(ref self: T, server_id: u32, name: ByteArray);
    fn set_models(ref self: T, seed: felt252, n_models: u32);
    #[cfg(feature: 'dungeon')]
    fn enter_dungeon(ref self: T, dungeon_address: starknet::ContractAddress);
}

#[dojo::contract]
pub mod actions {
    #[cfg(feature: 'dungeon')]
    use armory::Flatbow;
    #[cfg(feature: 'dungeon')]
    use bestiary::RiverSkale;
    use dojo::event::EventStorage;
    use dojo::model::{Model, ModelStorage, ModelValueStorage};
    use dojo::world::{WorldStorage, WorldStorageTrait};

    // Features can be used on modules, structs, trait and `use`. Not inside
    // a function.
    #[cfg(feature: 'dungeon')]
    use dojo_examples::dungeon::{IDungeonDispatcher, IDungeonDispatcherTrait};
    use dojo_examples::lib_math::{SimpleMathDispatcherTrait, SimpleMathLibraryDispatcher};
    use dojo_examples::models::{
        Direction, Moves, MovesValue, PlayerConfig, PlayerConfigItems, PlayerItem, Position,
        ServerProfile, Vec2,
    };
    use dojo_examples::utils::next_position;
    use starknet::{ContractAddress, get_caller_address};
    use super::IActions;

    #[derive(Copy, Drop, Serde)]
    #[dojo::event]
    pub struct Moved {
        #[key]
        pub player: ContractAddress,
        pub direction: Direction,
    }

    // impl: implement functions specified in trait
    #[abi(embed_v0)]
    impl ActionsImpl of IActions<ContractState> {
        // Set some models randomly.
        fn set_models(ref self: ContractState, seed: felt252, n_models: u32) {
            let uint: u256 = seed.into();
            let prng: u32 = (uint % 4_294_967_000).try_into().unwrap();
            let byte: u8 = (uint % 255).try_into().unwrap();

            let moves = Moves {
                player: seed.try_into().unwrap(), remaining: byte, last_direction: Direction::None,
            };
            let position = Position {
                player: seed.try_into().unwrap(), vec: Vec2 { x: prng, y: prng },
            };
            let server_profile = ServerProfile {
                player: seed.try_into().unwrap(), server_id: prng, name: "hello",
            };
            let player_config = PlayerConfig {
                player: seed.try_into().unwrap(),
                name: "hello",
                items: array![],
                favorite_item: Option::None,
            };

            let mut world = self.world_default();

            if n_models == 4 {
                world.write_model(@moves);
                world.write_model(@position);
                world.write_model(@server_profile);
                world.write_model(@player_config);
            } else if n_models == 3 {
                world.write_model(@moves);
                world.write_model(@position);
                world.write_model(@server_profile);
            } else if n_models == 2 {
                world.write_model(@moves);
                world.write_model(@position);
            } else {
                world.write_model(@moves);
            }
        }

        // ContractState is defined by system decorator expansion
        fn spawn(ref self: ContractState) {
            let player = get_caller_address();
            self.set_default_position(player);
        }

        fn move(ref self: ContractState, direction: Direction) {
            let player = get_caller_address();
            let mut world = self.world_default();

            // instead of using the `get!` macro, you can directly use
            // the <ModelName>Store::get method
            let mut position: Position = world.read_model(player);

            // You can get the entity ID in different ways.
            // Using the `Model` Model::<YOUR_TYPE>::entity_id(@model).
            // Or using `dojo::utils::entity_id_from_serialized_keys([player].span())`.
            let player_felt: felt252 = player.into();
            let move_id = dojo::utils::entity_id_from_serialized_keys([player_felt].span());

            let mut moves: MovesValue = world.read_value_from_id(move_id);

            let simple_math = self.simple_math_dispatcher(@world);
            moves.remaining = simple_math.decrement_saturating(moves.remaining);
            moves.last_direction = direction;
            world.write_value_from_id(move_id, @moves);

            let next = next_position(position, direction);
            world.write_model(@next);

            world.emit_event(@Moved { player, direction });
        }

        fn set_player_config(ref self: ContractState, name: ByteArray) {
            let mut world = self.world_default();

            let player = get_caller_address();

            let items = array![
                PlayerItem { item_id: 1, quantity: 100, score: 150 },
                PlayerItem { item_id: 2, quantity: 50, score: -32 },
            ];

            let config = PlayerConfig { player, name, items, favorite_item: Option::Some(1) };
            world.write_model(@config);
        }

        fn update_player_config_items(ref self: ContractState) {
            let mut world = self.world_default();
            let player = get_caller_address();
            let items = array![
                PlayerItem { item_id: 1, quantity: 12, score: 8 },
                PlayerItem { item_id: 2, quantity: 42, score: -123 },
            ];
            let favorite_item = Option::Some(2);
            let player_items = PlayerConfigItems { items, favorite_item };

            // Don't need to read the model here, we directly overwrite the member "items".
            world.write_schema(Model::<PlayerConfig>::ptr_from_keys(player), @player_items);
        }

        fn update_player_config_name(ref self: ContractState, name: ByteArray) {
            let mut world = self.world_default();
            let player = get_caller_address();

            // Don't need to read the model here, we directly overwrite the member "name".
            world
                .write_member(
                    Model::<PlayerConfig>::ptr_from_keys(player), selector!("name"), name,
                );
        }

        fn reset_player_config(ref self: ContractState) {
            let player = get_caller_address();
            let mut world = self.world_default();

            let position: Position = world.read_model(player);
            let moves: Moves = world.read_model(player);
            let config: PlayerConfig = world.read_model(player);

            world.erase_model(@position);
            world.erase_model(@moves);
            world.erase_model(@config);

            let position: Position = world.read_model(player);
            let moves: Moves = world.read_model(player);
            let config: PlayerConfig = world.read_model(player);

            assert(moves.remaining == 0, 'bad remaining');
            assert(moves.last_direction == Direction::None, 'bad last direction');

            assert(position.vec.x == 0, 'bad x');
            assert(position.vec.y == 0, 'bad y');

            assert(config.items.len() == 0, 'bad items');
            assert(config.favorite_item == Option::Some(0), 'bad favorite item');
            let empty_string: ByteArray = "";
            assert(config.name == empty_string, 'bad name');
        }

        fn set_player_server_profile(ref self: ContractState, server_id: u32, name: ByteArray) {
            let player = get_caller_address();
            let mut world = self.world_default();

            let profile = ServerProfile { player, server_id, name };
            world.write_model(@profile);
        }

        fn get_player_position(self: @ContractState) -> Position {
            let player = get_caller_address();
            let mut world = self.world_default();
            world.read_model(player)
        }

        #[cfg(feature: 'dungeon')]
        fn enter_dungeon(ref self: ContractState, dungeon_address: ContractAddress) {
            let mut world = self.world_default();

            let flatbow = Flatbow { id: 1, atk_speek: 2, range: 1 };
            let river_skale = RiverSkale { id: 1, health: 5, armor: 3, attack: 2 };

            world.write_model(@flatbow);
            world.write_model(@river_skale);

            IDungeonDispatcher { contract_address: dungeon_address }.enter();
        }
    }

    // The `generate_trait` attribute is not compatible with `world` parameter expansion.
    // Hence, the use of `self` to access the contract state.
    #[generate_trait]
    impl InternalImpl of InternalUtils {
        fn set_default_position(self: @ContractState, player: ContractAddress) {
            let mut world = self.world_default();

            world.write_model(@Moves { player, remaining: 99, last_direction: Direction::None });
            world.write_model(@Position { player, vec: Vec2 { x: 10, y: 10 } });
        }

        /// Use the default namespace "ns". A function is handy since the ByteArray
        /// can't be const.
        fn world_default(self: @ContractState) -> WorldStorage {
            self.world(@"ns")
        }

        /// A gas optimized version of `world_default`, where hash is computed at compile time.
        /// Can make a difference if switching between namespaces is frequent.
        fn world_default_ns_hash(self: @ContractState) -> WorldStorage {
            self.world_ns_hash(bytearray_hash!("ns"))
        }

        /// Example of how to get a dispatcher from a declared dojo library.
        /// `{library_module_name}_v{version}`.
        fn simple_math_dispatcher(
            self: @ContractState, world: @WorldStorage,
        ) -> SimpleMathLibraryDispatcher {
            let (_, class_hash) = world.dns(@"simple_math_v0_1_0").expect('simple_math not found');

            SimpleMathLibraryDispatcher { class_hash }
        }
    }
}

#[cfg(test)]
mod tests {
    use dojo::model::{ModelStorage, ModelStorageTest, ModelValueStorage};
    use dojo::world::WorldStorageTrait;
    use dojo_cairo_test::{
        ContractDef, ContractDefTrait, NamespaceDef, TestResource, WorldStorageTestTrait,
        spawn_test_world,
    };
    use dojo_examples::lib_math::simple_math;
    use dojo_examples::models::{Direction, Moves, Position, PositionValue, m_Moves, m_Position};
    use crate::dungeon::dungeon;
    use super::{IActionsDispatcher, IActionsDispatcherTrait, actions};

    fn namespace_def() -> NamespaceDef {
        let ndef = NamespaceDef {
            namespace: "ns",
            resources: [
                TestResource::Model(m_Position::TEST_CLASS_HASH),
                TestResource::Model(m_Moves::TEST_CLASS_HASH),
                TestResource::Event(actions::e_Moved::TEST_CLASS_HASH),
                TestResource::Contract(actions::TEST_CLASS_HASH),
                TestResource::Library((simple_math::TEST_CLASS_HASH, @"simple_math", @"0_1_0")),
            ]
                .span(),
        };

        ndef
    }

    fn contract_defs() -> Span<ContractDef> {
        [
            ContractDefTrait::new(@"ns", @"actions")
                .with_writer_of([dojo::utils::bytearray_hash(@"ns")].span())
        ]
            .span()
    }

    #[test]
    fn test_world_test_set() {
        let caller = starknet::contract_address_const::<0xb0b>();

        let ndef = namespace_def();
        let mut world = spawn_test_world([ndef].span());

        world.sync_perms_and_inits(contract_defs());

        // Without having the permission, we can set data into the dojo database for the given
        // models.
        let mut position: Position = world.read_model(caller);
        assert(position.vec.x == 0 && position.vec.y == 0, 'bad x');

        position.vec.x = 122;
        // `write_model_test` and `erase_model_test` are available to bypass permissions.
        world.write_model_test(@position);

        // Example using the entity id.
        let caller_felt: felt252 = caller.into();
        let id = dojo::utils::entity_id_from_serialized_keys([caller_felt].span());
        let mut position: PositionValue = world.read_value_from_id(id);
        assert(position.vec.x == 122, 'bad x');

        position.vec.y = 88;
        world.write_value_from_id(id, @position);

        let mut position: Position = world.read_model(caller);
        assert(position.vec.y == 88, 'bad y');

        world.erase_model(@position);

        let position: Position = world.read_model(caller);
        assert(position.vec.x == 0 && position.vec.y == 0, 'bad delete');
    }

    #[test]
    #[available_gas(30000000)]
    fn test_move() {
        let caller = starknet::contract_address_const::<0xb0b>();

        let ndef = namespace_def();
        let mut world = spawn_test_world([ndef].span());
        world.sync_perms_and_inits(contract_defs());

        let (actions_system_addr, _) = world.dns(@"actions").unwrap();
        let actions_system = IActionsDispatcher { contract_address: actions_system_addr };

        starknet::testing::set_contract_address(caller);

        actions_system.spawn();
        let initial_moves: Moves = world.read_model(caller);
        let initial_position: Position = world.read_model(caller);

        assert(
            initial_position.vec.x == 10 && initial_position.vec.y == 10, 'wrong initial position',
        );
        assert(initial_moves.remaining == 99, 'wrong initial moves');

        actions_system.move(Direction::Right(()));

        let moves: Moves = world.read_model(caller);
        let right_dir_felt: felt252 = Direction::Right(()).into();

        assert(moves.remaining == initial_moves.remaining - 1, 'moves is wrong');
        assert(moves.last_direction.into() == right_dir_felt, 'last direction is wrong');

        let new_position: Position = world.read_model(caller);
        assert(new_position.vec.x == initial_position.vec.x + 1, 'position x is wrong');
        assert(new_position.vec.y == initial_position.vec.y, 'position y is wrong');
    }

    #[test]
    #[available_gas(30000000)]
    fn test_world_from_hash() {
        let ndef = namespace_def();
        let mut world = spawn_test_world([ndef].span());
        world.sync_perms_and_inits(contract_defs());
        let hash: felt252 = bytearray_hash!("ns");
        let storage = dojo::world::WorldStorageTrait::new_from_hash(world.dispatcher, hash);
        assert_eq!(storage.namespace_hash, world.namespace_hash);
        assert_eq!(storage.dispatcher.contract_address, world.dispatcher.contract_address);
    }

    #[test]
    #[available_gas(30000000)]
    #[cfg(feature: 'dungeon')]
    fn test_feature_dungeon() {
        let ndef = NamespaceDef {
            namespace: "ns",
            resources: [
                TestResource::Model(armory::m_Flatbow::TEST_CLASS_HASH),
                TestResource::Model(bestiary::m_RiverSkale::TEST_CLASS_HASH),
                TestResource::Contract(actions::TEST_CLASS_HASH),
                TestResource::Contract(dungeon::TEST_CLASS_HASH),
            ]
                .span(),
        };

        let contract_defs = [
            ContractDefTrait::new(@"ns", @"actions")
                .with_writer_of([dojo::utils::bytearray_hash(@"ns")].span()),
            ContractDefTrait::new(@"ns", @"dungeon")
                .with_writer_of([dojo::utils::bytearray_hash(@"ns")].span()),
        ]
            .span();

        let mut world = spawn_test_world([ndef].span());
        world.sync_perms_and_inits(contract_defs);

        let other = starknet::contract_address_const::<0x1234>();
        starknet::testing::set_contract_address(other);

        let (dungeon_addr, _) = world.dns(@"dungeon").unwrap();

        let (actions_system_addr, _) = world.dns(@"actions").unwrap();
        let actions_system = IActionsDispatcher { contract_address: actions_system_addr };

        actions_system.enter_dungeon(dungeon_addr);
    }
}
