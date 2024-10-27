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

#[dojo::interface]
pub trait MyInterface {
    fn system_1(ref self: T, a: felt252, b: felt252);
    fn system_2(self: @T, a: felt252) -> felt252;
}

#[dojo::contract]
pub mod c1 {
    use super::{MyInterface, M, M2};
    use dojo::model::ModelStorage;

    fn dojo_init(self: @ContractState, arg1: felt252) {
        let _arg1 = arg1;
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

