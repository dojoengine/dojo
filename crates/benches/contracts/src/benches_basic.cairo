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
