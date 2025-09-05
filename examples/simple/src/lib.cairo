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

#[starknet::interface]
pub trait MyInterface<T> {
    fn system_1(ref self: T, k: felt252, v: felt252);
    fn system_2(ref self: T, k: felt252) -> felt252;
    fn system_3(ref self: T, k: felt252, v: u32);
    fn system_4(ref self: T, k: felt252);
}

#[dojo::contract]
pub mod c1 {
    use dojo::event::EventStorage;
    use dojo::model::{Model, ModelStorage, ModelValueStorage};
    use super::{E, EH, M, MValue, MyInterface};

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
            mv.v = 12;
            world.write_value_from_id(entity_id, @mv);

            world.erase_model_ptr(Model::<M>::ptr_from_id(entity_id));
        }
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
    use dojo_cairo_test::{
        ContractDefTrait, NamespaceDef, TestResource, WorldStorageTestTrait, spawn_test_world,
    };
    use super::{M, c1, m_M};

    #[test]
    fn test_1() {
        let ndef = NamespaceDef {
            namespace: "ns",
            resources: [
                TestResource::Model(m_M::TEST_CLASS_HASH.try_into().unwrap()),
                TestResource::Contract(c1::TEST_CLASS_HASH.try_into().unwrap()),
            ]
                .span(),
        };

        let world = spawn_test_world([ndef].span());

        let c1_def = ContractDefTrait::new(@"ns", @"c1")
            .with_writer_of([dojo::utils::bytearray_hash(@"ns")].span())
            .with_init_calldata([0xff].span());

        world.sync_perms_and_inits([c1_def].span());

        let m: M = world.read_model(0);
        assert!(m.v == 0xff, "invalid b");
    }
}
