#[starknet::contract]
pub mod sn_c1 {
    #[storage]
    struct Storage {}
}

#[derive(Introspect, Drop, Serde)]
#[dojo::model]
pub struct M {
    #[key]
    pub a: felt252,
    pub b: felt252,
}

#[derive(Introspect, Drop, Serde)]
#[dojo::model]
pub struct M2 {
    #[key]
    pub a: u32,
    pub b: u256,
}

#[derive(Introspect, Drop, Serde)]
#[dojo::event]
pub struct E {
    #[key]
    pub a: felt252,
    pub b: u32,
}

#[starknet::interface]
pub trait MyInterface<T> {
    fn system_1(ref self: T, a: felt252, b: felt252);
    fn system_2(self: @T, a: felt252) -> felt252;
    fn system_3(self: @T, a: felt252, b: u32);
}

#[dojo::contract]
pub mod c1 {
    use super::{MyInterface, M, E};
    use dojo::model::ModelStorage;
    use dojo::event::EventStorage;

    fn dojo_init(self: @ContractState, arg1: felt252) {
        let m = M { a: 0, b: arg1, };

        let mut world = self.world("ns");
        world.write_model(@m);
    }

    #[abi(embed_v0)]
    impl MyInterfaceImpl of MyInterface<ContractState> {
        fn system_1(ref self: ContractState, a: felt252, b: felt252) {
            let mut world = self.world("ns");

            let m = M { a, b, };

            world.write_model(@m)
        }

        fn system_2(self: @ContractState, a: felt252) -> felt252 {
            let world = self.world("ns");

            let m: M = world.read_model(a);

            m.b
        }

        fn system_3(self: @ContractState, a: felt252, b: u32) {
            let mut world = self.world("ns");

            let e = E { a, b, };

            world.emit_event(@e);
        }
    }
}

#[dojo::contract]
pub mod c2 {}

#[cfg(test)]
mod tests {
    use dojo::world::WorldStorageTrait;
    use dojo::model::{ModelStorage, ModelStorageTest};
    use dojo_cairo_test::{spawn_test_world, NamespaceDef, TestResource, ContractDefTrait};
    use super::{c1, m, M};

    #[test]
    fn test_1() {
        let ndef = NamespaceDef {
            namespace: "ns", resources: [
                TestResource::Model(m::TEST_CLASS_HASH.try_into().unwrap()),
                TestResource::Contract(
                    ContractDefTrait::new(c1::TEST_CLASS_HASH, "c1")
                        .with_init_calldata([0xff].span())
                        .with_writer_of([dojo::utils::bytearray_hash(@"ns")].span())
                )
            ].span()
        };

        let world = spawn_test_world([ndef].span());

        let mut world = WorldStorageTrait::new(world, "ns");

        let m: M = world.read_model(0);
        assert!(m.b == 0xff, "invalid b");

        let m2 = M { a: 120, b: 244, };

        // `write_model_test` goes over permissions checks.
        starknet::testing::set_contract_address(123.try_into().unwrap());
        world.write_model_test(@m2);
    }
}
