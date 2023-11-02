use benches::models::{Direction};
use benches::character::Abilities;

// define the interface
#[starknet::interface]
trait IActions<TContractState> {
    fn spawn(self: @TContractState);
    fn move(self: @TContractState, direction: Direction);
    fn bench_basic_emit(self: @TContractState, name: felt252);
    fn bench_basic_set(self: @TContractState, name: felt252);
    fn bench_basic_get(self: @TContractState);
    fn bench_complex_set_default(self: @TContractState);
    fn bench_complex_set_with_smaller(self: @TContractState, abilities: Abilities);
    fn bench_complex_update_minimal(self: @TContractState, earned: u32);
}

// dojo decorator
#[dojo::contract]
mod actions {
    use starknet::{ContractAddress, get_caller_address};
    use benches::models::{Position, Moves, Direction, Vec2, Alias};
    use benches::utils::next_position;
    use benches::character::{Character, Abilities, Stats, Weapon, Sword};
    use super::IActions;
    use debug::PrintTrait;

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

        fn bench_basic_emit(self: @ContractState, name: felt252) {
            let world = self.world_dispatcher.read();
            let player = get_caller_address();

            emit!(world, Aliased { player, name: name });
        }

        fn bench_basic_set(self: @ContractState, name: felt252) {
            let world = self.world_dispatcher.read();
            let player = get_caller_address();

            set!(world, Alias { player, name: name });
        }

        fn bench_basic_get(self: @ContractState) {
            let world = self.world_dispatcher.read();
            let player = get_caller_address();

            get!(world, player, Alias);
        }
        
        fn bench_complex_set_default(self: @ContractState) {
            let world = self.world_dispatcher.read();
            let caller = get_caller_address();

            // let abi = Abilities {
            //     strength: 1,
            //     dexterity: 2,
            //     constitution: 3,
            //     intelligence: 4,
            //     wisdom: 5,
            //     charisma: 6,
            // };
            // let s = dojo::model::Model::values(@abi);
            // (*s.at(0)).print();

            set!(world, Character {
                caller: get_caller_address(),
                heigth: 170,
                abilities: Abilities {
                    strength: 8,
                    dexterity: 8,
                    constitution: 8,
                    intelligence: 8,
                    wisdom: 8,
                    charisma: 8,
                },
                stats: Stats {
                    kills: 0,
                    deaths: 0,
                    rests: 0,
                    hits: 0,
                    blocks: 0,
                    walked: 0,
                    runned: 0,
                    finished: false,
                    romances: 0,
                },
                weapon: Weapon::Fists((
                    Sword {
                        swordsmith: get_caller_address(),
                        damage: 10,
                    },
                    Sword {
                        swordsmith: get_caller_address(),
                        damage: 10,
                    },
                )),
                gold: 0,
            });
        }

        fn bench_complex_set_with_smaller(self: @ContractState, abilities: Abilities) {
            let world = self.world_dispatcher.read();
            let caller = get_caller_address();

            set!(world, Character {
                caller: get_caller_address(),
                heigth: 170,
                abilities,
                stats: Stats {
                    kills: 0,
                    deaths: 0,
                    rests: 0,
                    hits: 0,
                    blocks: 0,
                    walked: 0,
                    runned: 0,
                    finished: false,
                    romances: 0,
                },
                weapon: Weapon::Fists((
                    Sword {
                        swordsmith: get_caller_address(),
                        damage: 10,
                    },
                    Sword {
                        swordsmith: get_caller_address(),
                        damage: 10,
                    },
                )),
                gold: 0,
            });
        }
        
        fn bench_complex_update_minimal(self: @ContractState, earned: u32) {
            let world = self.world_dispatcher.read();
            let caller = get_caller_address();

            let char = get!(world, caller, Character);

            set!(world, Character {
                caller: get_caller_address(),
                heigth: char.heigth,
                abilities: char.abilities,
                stats: char.stats,
                weapon: char.weapon,
                gold: char.gold + earned,
            });
        }
    }
}
