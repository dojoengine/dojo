#[dojo::contract]
pub mod others {
    use starknet::{ContractAddress, get_caller_address};
    use dojo::event::EventStorage;

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
