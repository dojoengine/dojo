use starknet::ContractAddress;

use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

use dojo::model::Model;
use dojo::utils::test::{deploy_with_world_address, spawn_test_world};

#[derive(Copy, Drop, Serde, Debug)]
#[dojo::model]
pub struct Foo {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[derive(Copy, Drop, Serde)]
#[dojo::model(namespace: "another_namespace", nomapping: true)]
pub struct Buzz {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[dojo::interface]
pub trait IFooSetter {
    fn set_foo(ref world: IWorldDispatcher, a: felt252, b: u128);
}

#[dojo::contract]
pub mod foo_setter {
    use super::IFooSetter;
    use super::Foo;

    #[abi(embed_v0)]
    impl IFooSetterImpl of super::IFooSetter<ContractState> {
        fn set_foo(ref world: IWorldDispatcher, a: felt252, b: u128) {
            set!(world, (Foo { caller: starknet::get_caller_address(), a, b }));
        }
    }
}

#[dojo::contract]
pub mod test_contract {}

#[dojo::contract]
pub mod test_contract_with_dojo_init_args {
    fn dojo_init(world: @IWorldDispatcher, _arg1: felt252) {
        let _u = world.uuid();
    }
}

#[dojo::contract(namespace: "buzz_namespace", nomapping: true)]
pub mod buzz_contract {}

#[derive(IntrospectPacked, Copy, Drop, Serde)]
pub struct Sword {
    pub swordsmith: ContractAddress,
    pub damage: u32,
}

#[derive(IntrospectPacked, Copy, Drop, Serde)]
#[dojo::model]
pub struct Case {
    #[key]
    pub owner: ContractAddress,
    pub sword: Sword,
    pub material: felt252,
}

#[derive(IntrospectPacked, Copy, Drop, Serde)]
#[dojo::model]
pub struct Character {
    #[key]
    pub caller: ContractAddress,
    pub heigth: felt252,
    pub abilities: Abilities,
    pub stats: Stats,
    pub weapon: Weapon,
    pub gold: u32,
}

#[derive(IntrospectPacked, Copy, Drop, Serde)]
pub struct Abilities {
    pub strength: u8,
    pub dexterity: u8,
    pub constitution: u8,
    pub intelligence: u8,
    pub wisdom: u8,
    pub charisma: u8,
}

#[derive(IntrospectPacked, Copy, Drop, Serde)]
pub struct Stats {
    pub kills: u128,
    pub deaths: u16,
    pub rests: u32,
    pub hits: u64,
    pub blocks: u32,
    pub walked: felt252,
    pub runned: felt252,
    pub finished: bool,
    pub romances: u16,
}

#[derive(IntrospectPacked, Copy, Drop, Serde)]
pub enum Weapon {
    DualWield: (Sword, Sword),
    Fists: (Sword, Sword), // Introspect requires same arms
}

#[starknet::interface]
pub trait Ibar<TContractState> {
    fn set_foo(self: @TContractState, a: felt252, b: u128);
    fn delete_foo(self: @TContractState);
    fn delete_foo_macro(self: @TContractState, foo: Foo);
    fn set_char(self: @TContractState, a: felt252, b: u32);
}

#[starknet::contract]
pub mod bar {
    use core::traits::Into;
    use starknet::{get_caller_address, ContractAddress};
    use starknet::storage::{StoragePointerReadAccess, StoragePointerWriteAccess};
    use dojo::model::{Model, ModelIndex};

    use super::{Foo, IWorldDispatcher, IWorldDispatcherTrait};
    use super::{Character, Abilities, Stats, Weapon, Sword};

    #[storage]
    struct Storage {
        world: IWorldDispatcher,
    }
    #[constructor]
    fn constructor(ref self: ContractState, world: ContractAddress) {
        self.world.write(IWorldDispatcher { contract_address: world })
    }

    #[abi(embed_v0)]
    impl IbarImpl of super::Ibar<ContractState> {
        fn set_foo(self: @ContractState, a: felt252, b: u128) {
            set!(self.world.read(), Foo { caller: get_caller_address(), a, b });
        }

        fn delete_foo(self: @ContractState) {
            self
                .world
                .read()
                .delete_entity(
                    Model::<Foo>::selector(),
                    ModelIndex::Keys([get_caller_address().into()].span()),
                    Model::<Foo>::layout()
                );
        }

        fn delete_foo_macro(self: @ContractState, foo: Foo) {
            delete!(self.world.read(), Foo { caller: foo.caller, a: foo.a, b: foo.b });
        }

        fn set_char(self: @ContractState, a: felt252, b: u32) {
            set!(
                self.world.read(),
                Character {
                    caller: get_caller_address(),
                    heigth: a,
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
                    weapon: Weapon::DualWield(
                        (
                            Sword { swordsmith: get_caller_address(), damage: 0x12345678, },
                            Sword { swordsmith: get_caller_address(), damage: 0x12345678, }
                        )
                    ),
                    gold: b,
                }
            );
        }
    }
}

pub fn deploy_world() -> IWorldDispatcher {
    spawn_test_world(["dojo"].span(), [].span())
}

pub fn deploy_world_and_bar() -> (IWorldDispatcher, IbarDispatcher) {
    // Spawn empty world
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    // System contract
    let contract_address = deploy_with_world_address(bar::TEST_CLASS_HASH, world);
    let bar_contract = IbarDispatcher { contract_address };

    world.grant_writer(Model::<Foo>::selector(), contract_address);

    (world, bar_contract)
}

pub fn drop_all_events(address: ContractAddress) {
    loop {
        match starknet::testing::pop_log_raw(address) {
            core::option::Option::Some(_) => {},
            core::option::Option::None => { break; },
        };
    }
}
