use starknet::ContractAddress;

use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait, IWorldTestDispatcher, IWorldTestDispatcherTrait};
use dojo::model::{Model, ModelStorage};

use crate::world::{deploy_with_world_address, spawn_test_world, NamespaceDef, TestResource, ContractDefTrait};

pub const DOJO_NSH: felt252 = 0x309e09669bc1fdc1dd6563a7ef862aa6227c97d099d08cc7b81bad58a7443fa;

#[derive(Copy, Drop, Serde, Debug)]
#[dojo::event]
pub struct SimpleEvent {
    #[key]
    pub id: u32,
    pub data: (felt252, felt252),
}

#[derive(Copy, Drop, Serde, Debug)]
#[dojo::model]
pub struct Foo {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[starknet::contract]
pub mod foo_invalid_name {
    use dojo::model::IModel;

    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    pub impl ModelImpl of IModel<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "foo-bis"
        }

        fn version(self: @ContractState) -> u8 {
            1
        }

        fn unpacked_size(self: @ContractState) -> Option<usize> {
            Option::None
        }

        fn packed_size(self: @ContractState) -> Option<usize> {
            Option::None
        }

        fn layout(self: @ContractState) -> dojo::meta::Layout {
            dojo::meta::Layout::Fixed([].span())
        }

        fn schema(self: @ContractState) -> dojo::meta::introspect::Ty {
            dojo::meta::introspect::Ty::Struct(
                dojo::meta::introspect::Struct {
                    name: 'foo', attrs: [].span(), children: [].span()
                }
            )
        }

        fn definition(self: @ContractState) -> dojo::model::ModelDef {
            dojo::model::ModelDef {
                name: Self::dojo_name(self),
                version: Self::version(self),
                layout: Self::layout(self),
                schema: Self::schema(self),
                packed_size: Self::packed_size(self),
                unpacked_size: Self::unpacked_size(self),
            }
        }
    }
}

#[starknet::interface]
pub trait IFooSetter<T> {
    fn set_foo(ref self: T, a: felt252, b: u128);
}

#[dojo::contract]
pub mod foo_setter {
    use super::{Foo, IFooSetter};
    use dojo::model::ModelStorage;

    #[abi(embed_v0)]
    impl IFooSetterImpl of IFooSetter<ContractState> {
        fn set_foo(ref self: ContractState, a: felt252, b: u128) {
            let mut world = self.world("dojo");
            world.write_model(@Foo { caller: starknet::get_caller_address(), a, b });
        }
    }
}

#[dojo::contract]
pub mod test_contract {}

#[dojo::contract]
pub mod test_contract_with_dojo_init_args {
    fn dojo_init(ref self: ContractState, arg1: felt252) {
        let _a = arg1;
    }
}

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
    use super::DOJO_NSH;

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
            // set!(self.world.read(), Foo { caller: get_caller_address(), a, b });
        }

        fn delete_foo(self: @ContractState) {
            self
                .world
                .read()
                .delete_entity(
                    Model::<Foo>::selector(DOJO_NSH),
                    ModelIndex::Keys([get_caller_address().into()].span()),
                    Model::<Foo>::layout()
                );
        }

        fn delete_foo_macro(self: @ContractState, foo: Foo) {
            //delete!(self.world.read(), Foo { caller: foo.caller, a: foo.a, b: foo.b });
        }

        fn set_char(self: @ContractState, a: felt252, b: u32) {
        }
    }
}

/// Deploys an empty world with the `dojo` namespace.
pub fn deploy_world() -> IWorldDispatcher {
    let namespace_def = NamespaceDef {
        namespace: "dojo",
        resources: [].span(),
    };

    spawn_test_world([namespace_def].span())
}

/// Deploys an empty world with the `dojo` namespace and registers the `foo` model.
/// No permissions are granted.
pub fn deploy_world_and_foo() -> (IWorldDispatcher, felt252) {
    let world = deploy_world();
    world.register_model("dojo", foo::TEST_CLASS_HASH.try_into().unwrap());
    let foo_selector = Model::<Foo>::selector(DOJO_NSH);

    (world, foo_selector)
}

/// Deploys an empty world with the `dojo` namespace and registers the `foo` model.
/// Grants the `bar` contract writer permissions to the `foo` model.
pub fn deploy_world_and_bar() -> (IWorldDispatcher, IbarDispatcher) {
    let namespace_def = NamespaceDef {
        namespace: "dojo",
        resources: [
            TestResource::Model(foo::TEST_CLASS_HASH.try_into().unwrap()),
            TestResource::Contract(ContractDefTrait::new(bar::TEST_CLASS_HASH, "bar")),
        ].span(),
    };

    let world = spawn_test_world([namespace_def].span());
    let bar_address = IWorldTestDispatcher { contract_address: world.contract_address }.dojo_contract_address(selector_from_tag!("dojo-bar"));

    let bar_contract = IbarDispatcher { contract_address: bar_address };

    world.grant_writer(Model::<Foo>::selector(DOJO_NSH), bar_address);

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
