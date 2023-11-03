use benches::models::{Direction};
use benches::character::Abilities;

// define the interface
#[starknet::interface]
trait IActions<TContractState> {
    fn spawn(self: @TContractState);
    fn move(self: @TContractState, direction: Direction);

    fn bench_basic_emit(self: @TContractState, name: felt252);
    fn bench_basic_set(self: @TContractState, name: felt252);
    fn bench_basic_double_set(self: @TContractState, name: felt252);
    fn bench_basic_get(self: @TContractState);

    fn bench_primitive_pass_many(
        self: @TContractState,
        first: felt252,
        second: felt252,
        third: felt252,
        fourth: felt252,
        fifth: felt252,
        sixth: felt252,
        seventh: felt252,
        eighth: felt252,
        ninth: felt252,
    );
    fn bench_primitive_iter(self: @TContractState, n: u32);
    fn bench_primitive_hash(self: @TContractState, a: felt252, b: felt252, c: felt252);

    fn bench_complex_set_default(self: @TContractState);
    fn bench_complex_set_with_smaller(self: @TContractState, abilities: Abilities);
    fn bench_complex_update_minimal(self: @TContractState, earned: u32);
    fn bench_complex_update_minimal_nested(self: @TContractState, which: u8);
    fn bench_complex_get(self: @TContractState);
    fn bench_complex_get_minimal(self: @TContractState) -> u32;
    fn bench_complex_check(self: @TContractState, ability: felt252, threshold: u8) -> bool;
}

// dojo decorator
#[dojo::contract]
mod actions {
    use starknet::{ContractAddress, get_caller_address};
    use benches::models::{Position, Moves, Direction, Vec2, Alias};
    use benches::utils::next_position;
    use benches::character::{Character, Abilities, Stats, Weapon, Sword};
    use super::IActions;
    use array::ArrayTrait;
    use array::SpanTrait;
    use poseidon::poseidon_hash_span;

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


    #[external(v0)]
    impl ActionsImpl of IActions<ContractState> {
        fn spawn(self: @ContractState) {
            let world = self.world_dispatcher.read();
            let player = get_caller_address();

            let position = get!(world, player, (Position));
            let moves = get!(world, player, (Moves));

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

        fn move(self: @ContractState, direction: Direction) {
            let world = self.world_dispatcher.read();
            let player = get_caller_address();
            let (mut position, mut moves) = get!(world, player, (Position, Moves));

            moves.remaining -= 1;
            moves.last_direction = direction;
            let next = next_position(position, direction);

            set!(world, (moves, next));
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

        fn bench_basic_double_set(self: @ContractState, name: felt252) {
            let world = self.world_dispatcher.read();
            let player = get_caller_address();

            set!(world, Alias { player, name: name });
            set!(world, Alias { player, name: name });
        }

        fn bench_basic_get(self: @ContractState) {
            let world = self.world_dispatcher.read();
            let player = get_caller_address();

            get!(world, player, Alias);
        }

        fn bench_primitive_pass_many(self: @ContractState,
            first: felt252,
            second: felt252,
            third: felt252,
            fourth: felt252,
            fifth: felt252,
            sixth: felt252,
            seventh: felt252,
            eighth: felt252,
            ninth: felt252,
        ) {
            let sum = first + second + third + fourth + fifth + sixth + seventh + eighth + ninth;
        }

        fn bench_primitive_iter(self: @ContractState, n: u32) {
            let mut i = 0;
            loop {
                if i == n {
                    break;
                }
                i += 1;
            }
        }

        fn bench_primitive_hash(self: @ContractState, a: felt252, b: felt252, c: felt252) { 
            let hash = poseidon_hash_span(array![a, b, c].span());
        }

        
        fn bench_complex_set_default(self: @ContractState) {
            let world = self.world_dispatcher.read();
            let caller = get_caller_address();

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

        fn bench_complex_update_minimal_nested(self: @ContractState, which: u8) {
            let world = self.world_dispatcher.read();
            let caller = get_caller_address();

            let char = get!(world, caller, Character);

            let stats = Stats {
                kills: char.stats.kills + if which == 0 { 0 } else { 1 },
                deaths: char.stats.deaths + if which == 1 { 0 } else { 1 },
                rests: char.stats.rests + if which == 2 { 0 } else { 1 },
                hits: char.stats.hits + if which == 3 { 0 } else { 1 },
                blocks: char.stats.blocks + if which == 4 { 0 } else { 1 },
                walked: char.stats.walked + if which == 5 { 0 } else { 1 },
                runned: char.stats.runned + if which == 6 { 0 } else { 1 },
                finished: char.stats.finished || if which == 7 { false } else { true },
                romances: char.stats.romances + if which == 8 { 0 } else { 1 },
            };

            set!(world, Character {
                caller: get_caller_address(),
                heigth: char.heigth,
                abilities: char.abilities,
                stats: Stats {
                    kills: char.stats.kills + 1,
                    deaths: char.stats.deaths,
                    rests: char.stats.rests,
                    hits: char.stats.hits,
                    blocks: char.stats.blocks,
                    walked: char.stats.walked,
                    runned: char.stats.runned,
                    finished: char.stats.finished,
                    romances: char.stats.romances,
                },
                weapon: char.weapon,
                gold: char.gold,
            });
        }

        fn bench_complex_get(self: @ContractState) {
            let world = self.world_dispatcher.read();
            let caller = get_caller_address();
            let char = get!(world, caller, Character);
        }

        fn bench_complex_get_minimal(self: @ContractState) -> u32 {
            let world = self.world_dispatcher.read();
            let caller = get_caller_address();

            let char = get!(world, caller, Character);
            char.gold
        }

        fn bench_complex_check(self: @ContractState, ability: felt252, threshold: u8) -> bool {
            let world = self.world_dispatcher.read();
            let caller = get_caller_address();

            let char = get!(world, caller, Character);
            let points = if ability == 0 { 
                char.abilities.strength
            } else if ability == 1 { 
                char.abilities.dexterity
            } else if ability == 2 { 
                char.abilities.constitution
            } else if ability == 3 { 
                char.abilities.intelligence
            } else if ability == 4 { 
                char.abilities.wisdom
            } else if ability == 5 { 
                char.abilities.charisma
            } else { 
                0 
            };
            
            points >= threshold
        }
    }
}
