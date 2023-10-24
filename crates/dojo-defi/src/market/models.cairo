use starknet::ContractAddress;
use dojo::database::schema::{Struct, Ty, SchemaIntrospection, Member};

// Cubit fixed point math library
use cubit::f128::types::fixed::Fixed;

const SCALING_FACTOR: u128 = 10000;

impl SchemaIntrospectionFixed of SchemaIntrospection<Fixed> {
    #[inline(always)]
    fn size() -> usize {
        2
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        layout.append(128);
        layout.append(1);
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Struct(
            Struct {
                name: 'Fixed',
                attrs: array![].span(),
                children: array![
                    Member { name: 'mag', ty: Ty::Primitive('u128'), attrs: array![].span() },
                    Member { name: 'sign', ty: Ty::Primitive('bool'), attrs: array![].span() }
                ]
                    .span()
            }
        )
    }
}

#[derive(Model, Copy, Drop, Serde)]
struct Cash {
    #[key]
    player: ContractAddress,
    amount: u128,
}

#[derive(Model, Copy, Drop, Serde)]
struct Item {
    #[key]
    player: ContractAddress,
    #[key]
    item_id: u32,
    quantity: u128,
}

#[derive(Model, Copy, Drop, Serde)]
struct Liquidity {
    #[key]
    player: ContractAddress,
    #[key]
    item_id: u32,
    shares: Fixed,
}

#[derive(Model, Copy, Drop, Serde)]
struct Market {
    #[key]
    item_id: u32,
    cash_amount: u128,
    item_quantity: u128,
}
