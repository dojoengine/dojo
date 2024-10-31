#[dojo::contract]
pub mod mock_token {
    use dojo_examples::models::{MockToken};
    use dojo::model::ModelStorage;
    use starknet::{ContractAddress, get_caller_address};

    fn dojo_init(self: @ContractState) {
        let account: ContractAddress = get_caller_address();
        let mut world = self.world(@"ns");
        world.write_model(@MockToken { account, amount: 1000 });
    }
}
