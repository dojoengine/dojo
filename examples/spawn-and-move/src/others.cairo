#[dojo::contract]
pub mod others {
    use dojo::event::EventStorage;
    use starknet::{ContractAddress, get_caller_address};

    #[derive(Copy, Drop, Serde)]
    #[dojo::event]
    struct MyInit {
        #[key]
        caller: ContractAddress,
        value: u8,
    }

    fn dojo_init(self: @ContractState, value: u8) {
        let mut world = self.world(@"ns");

        world.emit_event(@MyInit { caller: get_caller_address(), value });
    }
}
