#[starknet::contract]
pub mod sn_c1 {
    #[storage]
    struct Storage {}
}

#[derive(Introspect, Serde, Drop, DojoStore)]
#[dojo::model]
pub struct M {
    #[key]
    pub k: felt252,
    pub v: felt252,
}

#[dojo::event]
pub struct E {
    #[key]
    pub k: felt252,
    pub v: u32,
}

#[derive(Introspect, Serde, Drop)]
#[dojo::event]
pub struct EH {
    #[key]
    pub k: felt252,
    pub v: u32,
}

#[derive(Introspect, Serde, Drop, DojoStore)]
pub struct TypeTest {
    pub f1: felt252,
    pub f2: u32,
    pub f3: Option<felt252>,
    pub f4: u256,
}

#[dojo::model]
pub struct ModelTest {
    #[key]
    pub k: felt252,
    pub v: TypeTest,
}

#[derive(Serde, Drop)]
pub struct OtherType {
    pub f1: felt252,
    pub f2: u32,
    pub f3: Option<felt252>,
    pub f4: Option<M>,
    pub f5: u256,
}

// Types to test a type without being a model with Value as suffix.
// Those types also don't belong to any model or event, they are pulled
// from the contract ABI since they are present as function inputs or outputs.
#[derive(Serde, Drop, Introspect)]
pub struct SocialPlatform {
    pub platform: felt252,
    pub username: felt252,
}

#[derive(Serde, Drop, Introspect)]
pub enum PlayerSetting {
    Undefined,
    OptOutNotifications: SocialPlatform,
}
#[derive(Serde, Drop, Introspect)]
pub enum PlayerSettingValue {
    Undefined,
    Boolean: bool,
}

#[starknet::interface]
pub trait MyInterface<T> {
    fn system_1(ref self: T, k: felt252, v: felt252);
    fn system_2(ref self: T, k: felt252) -> felt252;
    fn system_3(ref self: T, k: felt252, v: u32);
    fn system_4(ref self: T, k: felt252);
    fn system_5(ref self: T, o: OtherType, m: M);
}

#[dojo::contract]
pub mod c1 {
    use dojo::event::EventStorage;
    use dojo::model::{Model, ModelStorage, ModelValueStorage};
    use super::{E, EH, M, MValue, MyInterface, OtherType, PlayerSetting, PlayerSettingValue};

    fn dojo_init(self: @ContractState, v: felt252) {
        let m = M { k: 0, v };

        let mut world = self.world_default();
        world.write_model(@m);
    }

    #[abi(embed_v0)]
    impl MyInterfaceImpl of MyInterface<ContractState> {
        fn system_1(ref self: ContractState, k: felt252, v: felt252) {
            let mut world = self.world_default();

            let m = M { k, v };

            world.write_model(@m)
        }

        fn system_2(ref self: ContractState, k: felt252) -> felt252 {
            let mut world = self.world_default();

            let m: M = world.read_model(k);

            m.v
        }

        fn system_3(ref self: ContractState, k: felt252, v: u32) {
            let mut world = self.world_default();

            let e = E { k, v };
            world.emit_event(@e);

            let eh = EH { k, v };
            world.emit_event(@eh);
        }

        fn system_4(ref self: ContractState, k: felt252) {
            let mut world = self.world_default();

            let m = M { k, v: 288 };

            let entity_id = Model::<M>::entity_id(@m);

            world.write_model(@m);
            world.erase_model(@m);

            let mut mv: MValue = world.read_value_from_id(entity_id);
            mv.v = 15;
            world.write_value_from_id(entity_id, @mv);

            world.erase_model_ptr(Model::<M>::ptr_from_id(entity_id));
        }

        fn system_5(ref self: ContractState, o: OtherType, m: M) {}
    }

    #[external(v0)]
    fn system_free(
        ref self: ContractState,
        o: Option<felt252>,
        o2: Option<OtherType>,
        o3: u256,
        o4: PlayerSetting,
        o5: PlayerSettingValue,
    ) -> Option<u32> {
        Some(48)
    }

    #[generate_trait]
    impl InternalImpl of InternalTrait {
        // Need a function since byte array can't be const.
        // We could have a self.world with an other function to init from hash, that can be
        // constant.
        fn world_default(self: @ContractState) -> dojo::world::WorldStorage {
            self.world(@"ns")
        }
    }
}

#[dojo::contract]
pub mod c2 {}

#[dojo::contract]
pub mod c3 {}

#[cfg(test)]
mod tests {
    use dojo::model::ModelStorage;
    use dojo::world::world;
    use dojo_cairo_test::{
        ContractDefTrait, NamespaceDef, TestResource, WorldStorageTestTrait, spawn_test_world,
    };
    use super::{M, c1, m_M};

    #[test]
    fn test_1() {
        let ndef = NamespaceDef {
            namespace: "ns",
            resources: [
                TestResource::Model(m_M::TEST_CLASS_HASH),
                TestResource::Contract(c1::TEST_CLASS_HASH),
            ]
                .span(),
        };

        let world = spawn_test_world(world::TEST_CLASS_HASH, [ndef].span());

        let c1_def = ContractDefTrait::new(@"ns", @"c1")
            .with_writer_of([dojo::utils::bytearray_hash(@"ns")].span())
            .with_init_calldata([0xff].span());

        world.sync_perms_and_inits([c1_def].span());

        let m: M = world.read_model(0);
        assert!(m.v == 0xff, "invalid b");
    }
}
