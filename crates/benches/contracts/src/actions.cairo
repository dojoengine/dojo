use benches::models::{Direction};

// define the interface
#[starknet::interface]
trait IActions<TContractState> {
    fn spawn(self: @TContractState);
    fn move(self: @TContractState, direction: Direction);
    fn bench_emit(self: @TContractState, name: felt252);
    fn bench_set(self: @TContractState, name: felt252);
    fn bench_get(self: @TContractState);
    fn bench_set_complex(self: @TContractState);
}

// dojo decorator
#[dojo::contract]
mod actions {
    use starknet::{ContractAddress, get_caller_address};
    use benches::models::{Position, Moves, Direction, Vec2, Alias};
    use benches::utils::next_position;
    use benches::character::{Character, Abilities, Stats, Weapon, Sword};
    use super::IActions;

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        Moved: Moved,
        Aliased: Aliased,
    }

    #[derive(Drop, starknet::Event)]
    struct Moved {
        player: ContractAddress,
        direction: Direction
    }

    #[derive(Drop, starknet::Event)]
    struct Aliased {
        player: ContractAddress,
        name: felt252,
    }


    // impl: implement functions specified in trait
    #[external(v0)]
    impl ActionsImpl of IActions<ContractState> {
        // ContractState is defined by system decorator expansion
        fn spawn(self: @ContractState) {
            // Access the world dispatcher for reading.
            let world = self.world_dispatcher.read();

            // Get the address of the current caller, possibly the player's address.
            let player = get_caller_address();

            // Retrieve the player's current position from the world.
            let position = get!(world, player, (Position));

            // Retrieve the player's move data, e.g., how many moves they have left.
            let moves = get!(world, player, (Moves));

            // Update the world state with the new data.
            // 1. Increase the player's remaining moves by 10.
            // 2. Move the player's position 10 units in both the x and y direction.
            set!(
                world,
                (
                    Moves {
                        player, remaining: moves.remaining + 10, last_direction: Direction::None(())
                    },
                    Position {
                        player, vec: Vec2 { x: position.vec.x + 10, y: position.vec.y + 10 }
                    },
                )
            );
        }

        // Implementation of the move function for the ContractState struct.
        fn move(self: @ContractState, direction: Direction) {
            // Access the world dispatcher for reading.
            let world = self.world_dispatcher.read();

            // Get the address of the current caller, possibly the player's address.
            let player = get_caller_address();

            // Retrieve the player's current position and moves data from the world.
            let (mut position, mut moves) = get!(world, player, (Position, Moves));

            // Deduct one from the player's remaining moves.
            moves.remaining -= 1;

            // Update the last direction the player moved in.
            moves.last_direction = direction;

            // Calculate the player's next position based on the provided direction.
            let next = next_position(position, direction);

            // Update the world state with the new moves data and position.
            set!(world, (moves, next));

            // Emit an event to the world to notify about the player's move.
            emit!(world, Moved { player, direction });
        }

        fn bench_emit(self: @ContractState, name: felt252) {
            let world = self.world_dispatcher.read();
            let player = get_caller_address();

            emit!(world, Aliased { player, name: name });
        }

        fn bench_set(self: @ContractState, name: felt252) {
            let world = self.world_dispatcher.read();
            let player = get_caller_address();

            set!(world, Alias { player, name: name });
        }

        fn bench_get(self: @ContractState) {
            let world = self.world_dispatcher.read();
            let player = get_caller_address();

            get!(world, player, Alias);
        }
        
        fn bench_set_complex(self: @ContractState) {
            let world = self.world_dispatcher.read();
            let caller = get_caller_address();

            set!(world, Character {
                caller,
                heigth: 0x123456789abcdef,
                abilities: Abilities {
                    strength: 0x12,
                    dexterity: 0x34,
                    constitution: 0x56,
                    intelligence: 0x78,
                    wisdom: 0x9a,
                    charisma: 0xbc,
                },
                stats: Stats {
                    kills: 0x123456789abcdef,
                    deaths: 0x1234,
                    rests: 0x12345678,
                    hits: 0x123456789abcdef,
                    blocks: 0x12345678,
                    walked: 0x123456789abcdef,
                    runned: 0x123456789abcdef,
                    finished: true,
                    romances: 0x1234,
                },
                weapon: Weapon::DualWield((
                    Sword {
                        swordsmith: starknet::contract_address_const::<0x69>(),
                        damage: 0x12345678,
                    },
                    Sword {
                        swordsmith: starknet::contract_address_const::<0x69>(),
                        damage: 0x12345678,
                    }
                )),
                gold: 0x12345678,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use starknet::class_hash::Felt252TryIntoClassHash;

    // import world dispatcher
    use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

    // import test utils
    use dojo::test_utils::{spawn_test_world, deploy_contract};

    // import models
    use benches::models::{position, moves};
    use benches::models::{Position, Moves, Direction, Vec2};

    // import actions
    use super::{actions, IActionsDispatcher, IActionsDispatcherTrait};

    #[test]
    #[available_gas(30000000)]
    fn test_move() {
        // caller
        let caller = starknet::contract_address_const::<0x0>();

        // models
        let mut models = array![position::TEST_CLASS_HASH, moves::TEST_CLASS_HASH];

        // deploy world with models
        let world = spawn_test_world(models);

        // deploy systems contract
        let contract_address = world
            .deploy_contract('salt', actions::TEST_CLASS_HASH.try_into().unwrap());
        let actions_system = IActionsDispatcher { contract_address };

        // call spawn()
        actions_system.spawn();

        // call move with direction right
        actions_system.move(Direction::Right(()));

        // Check world state
        let moves = get!(world, caller, Moves);

        // casting right direction
        let right_dir_felt: felt252 = Direction::Right(()).into();

        // check moves
        assert(moves.remaining == 9, 'moves is wrong');

        // check last direction
        assert(moves.last_direction.into() == right_dir_felt, 'last direction is wrong');

        // get new_position
        let new_position = get!(world, caller, Position);

        // check new position x
        assert(new_position.vec.x == 11, 'position x is wrong');

        // check new position y
        assert(new_position.vec.y == 10, 'position y is wrong');
    }
}
