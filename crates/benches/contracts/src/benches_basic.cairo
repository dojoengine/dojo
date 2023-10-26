use starknet::ContractAddress;

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Alias {
    #[key]
    player: ContractAddress,
    name: felt252,
}


#[system]
mod bench_emit {
    use starknet::ContractAddress;
    use dojo::world::Context;

    #[event]
    #[derive(Drop, Clone, Serde, PartialEq, starknet::Event)]
    enum Event {
        Alias: Alias,
    }

    #[derive(Drop, Clone, Serde, PartialEq, starknet::Event)]
    struct Alias {
        player: ContractAddress,
        name: felt252,
    }

    fn execute(ctx: Context, name: felt252) {
        emit!(ctx.world, Alias { player: ctx.origin, name: name, });

        return ();
    }
}


#[system]
mod bench_set {
    use starknet::ContractAddress;
    use dojo::world::Context;
    use super::Alias;

    fn execute(ctx: Context, name: felt252) {
        set!(ctx.world, Alias { player: ctx.origin, name: name, });

        return ();
    }
}


#[system]
mod bench_get {
    use starknet::ContractAddress;
    use dojo::world::Context;
    use super::Alias;

    fn execute(ctx: Context) {
        get!(ctx.world, ctx.origin, Alias,);

        return ();
    }
}
