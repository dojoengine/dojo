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

#[starknet::interface]
pub trait MyInterface<T> {
    fn system_1(ref self: T, a: felt252, b: felt252);
    fn system_2(self: @T, a: felt252) -> felt252;
}

#[dojo::contract]
pub mod c1 {
    use super::{MyInterface, M};
    use dojo::model::ModelStorage;

    fn dojo_init(self: @ContractState, arg1: felt252) {
        let m = M {
            a: 0,
            b: arg1,
        };

        let mut world = self.world("ns");
        world.write_model(@m);
    }

    #[abi(embed_v0)]
    impl MyInterfaceImpl of MyInterface<ContractState> {
        fn system_1(ref self: ContractState, a: felt252, b: felt252) {
            let mut world = self.world("ns");

            let m = M {
                a,
                b,
            };

            world.write_model(@m)
        }

        fn system_2(self: @ContractState, a: felt252) -> felt252 {
            let world = self.world("ns");

            let m: M = world.read_model(a);

            m.b
        }
    }
}

#[dojo::contract]
pub mod c2 {}

#[cfg(test)]
mod tests {
    use dojo::world::{WorldStorage, WorldStorageTrait};
    use dojo_cairo_test::{spawn_test_world, NamespaceDef, TestResource, deploy_with_world_address};
    use super::{c1, c2, m, M};

    #[test]
    fn test_1() {
        let ndef = NamespaceDef {
            namespace: "ns".into(),
            resources: [TestResource::Model(m::TEST_CLASS_HASH.try_into().unwrap())].span(),
        };

        let world = spawn_test_world([ndef].span());
        let world_storage = WorldStorageTrait::new(world, "ns".into());

        let contract = deploy_with_world_address(c1::TEST_CLASS_HASH.try_into().unwrap(), world);
    }
}
