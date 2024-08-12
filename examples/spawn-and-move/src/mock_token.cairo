#[dojo::contract]
pub mod mock_token {
    use dojo_examples::models::{MockToken};
    use starknet::{ContractAddress, get_caller_address};

    fn dojo_init(world: @IWorldDispatcher) {
        let account: ContractAddress = get_caller_address();
        set!(world, MockToken { account, amount: 1000 });
    }
}
