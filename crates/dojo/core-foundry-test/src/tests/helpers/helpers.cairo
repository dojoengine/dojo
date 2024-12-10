use starknet::{ContractAddress};

use dojo::world::{IWorldDispatcher, WorldStorage, WorldStorageTrait};
use dojo::model::Model;

use crate::world::{
    spawn_test_world, NamespaceDef, TestResource, ContractDefTrait, WorldStorageTestTrait
};

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

#[derive(Drop, Serde, Debug)]
#[dojo::model]
pub struct NotCopiable {
    #[key]
    pub caller: ContractAddress,
    pub a: Array<felt252>,
    pub b: ByteArray,
}

#[starknet::contract]
pub mod m_FooInvalidName {
    use dojo::model::IModel;

    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    pub impl DeployedModelImpl of dojo::meta::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "foo-bis"
        }
    }

    #[abi(embed_v0)]
    pub impl StoredModelImpl of dojo::meta::interface::IStoredResource<ContractState> {
        fn layout(self: @ContractState) -> dojo::meta::Layout {
            dojo::meta::Layout::Fixed([].span())
        }

        fn schema(self: @ContractState) -> dojo::meta::introspect::Struct {
            dojo::meta::introspect::Struct { name: 'foo', attrs: [].span(), children: [].span() }
        }
    }

    #[abi(embed_v0)]
    pub impl ModelImpl of IModel<ContractState> {
        fn unpacked_size(self: @ContractState) -> Option<usize> {
            Option::None
        }

        fn packed_size(self: @ContractState) -> Option<usize> {
            Option::None
        }

        fn definition(self: @ContractState) -> dojo::model::ModelDef {
            dojo::model::ModelDef {
                name: DeployedModelImpl::dojo_name(self),
                layout: StoredModelImpl::layout(self),
                schema: StoredModelImpl::schema(self),
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
            let mut world = self.world(@"dojo");
            world.write_model(@Foo { caller: starknet::get_caller_address(), a, b });
        }
    }
}

#[dojo::contract]
pub mod dojo_caller_contract {}

#[starknet::contract]
pub mod non_dojo_caller_contract {
    #[storage]
    struct Storage {}
}

#[dojo::contract]
pub mod test_contract {}

#[dojo::contract]
pub mod another_test_contract {}

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
}

#[dojo::contract]
pub mod bar {
    use core::traits::Into;
    use starknet::{get_caller_address};
    use dojo::model::{ModelStorage, ModelPtr};

    use super::{Foo, IWorldDispatcher};

    #[storage]
    struct Storage {
        world: IWorldDispatcher,
    }

    #[abi(embed_v0)]
    impl IbarImpl of super::Ibar<ContractState> {
        fn set_foo(self: @ContractState, a: felt252, b: u128) {
            let mut world = self.world(@"dojo");
            world.write_model(@Foo { caller: get_caller_address(), a, b });
        }

        fn delete_foo(self: @ContractState) {
            let mut world = self.world(@"dojo");
            let ptr = ModelPtr::<
                Foo
            > { id: core::poseidon::poseidon_hash_span([get_caller_address().into()].span()) };
            world.erase_model_ptr(ptr);
        }
    }
}

#[starknet::contract]
pub mod malicious_contract {
    #[storage]
    struct Storage {}
}

/// Deploys an empty world with the `dojo` namespace.
pub fn deploy_world() -> WorldStorage {
    let namespace_def = NamespaceDef { namespace: "dojo", resources: [].span(), };

    spawn_test_world([namespace_def].span())
}

/// Deploys an empty world with the `dojo` namespace and registers the `foo` model.
/// No permissions are granted.
pub fn deploy_world_and_foo() -> (WorldStorage, felt252) {
    let namespace_def = NamespaceDef {
        namespace: "dojo", resources: [
            TestResource::Model("Foo"), TestResource::Model("NotCopiable"),
        ].span(),
    };

    (spawn_test_world([namespace_def].span()), Model::<Foo>::selector(DOJO_NSH))
}

/// Deploys an empty world with the `dojo` namespace and registers the `foo` model.
/// Grants the `bar` contract writer permissions to the `foo` model.
pub fn deploy_world_and_bar() -> (WorldStorage, IbarDispatcher) {
    let namespace_def = NamespaceDef {
        namespace: "dojo", resources: [
            TestResource::Model("Foo"), TestResource::Contract("bar"),
        ].span(),
    };

    let bar_def = ContractDefTrait::new(@"dojo", @"bar")
        .with_writer_of([Model::<Foo>::selector(DOJO_NSH)].span());

    let mut world = spawn_test_world([namespace_def].span());
    world.sync_perms_and_inits([bar_def].span());

    let (bar_address, _) = world.dns(@"bar").unwrap();
    let bar_contract = IbarDispatcher { contract_address: bar_address };

    (world, bar_contract)
}

