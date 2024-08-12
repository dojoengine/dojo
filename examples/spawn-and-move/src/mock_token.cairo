#[dojo::contract]
pub mod mock_token {
    use dojo_examples::models::{MockToken};
    use starknet::{ContractAddress, get_caller_address};

    fn dojo_init(
        world: @IWorldDispatcher
    ) { // TODO: Dojo init are called before the authorization are given.
    // And the resource must be registered to actually give the authorization.
    // So we ends up in dojo_init function not able to set any models...
    //
    // We will change the order for the migration:
    // 1. Declare models + multicall register.
    // 2. Declare contracts + multicall deploy.
    // 3. Run authorizations.
    // 4. Run Dojo init. To avoid front-run, the world should check that the
    //    account is owner of the contract.
    //
    // For now, we will just declare the models and the multicall register.
    // We will deploy the contracts after the fact.
    //
    // let account: ContractAddress = get_caller_address();
    // set!(world, MockToken { account, amount: 1000 });
    }
}
