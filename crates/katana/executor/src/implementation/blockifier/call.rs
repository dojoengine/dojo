use std::sync::Arc;

use blockifier::context::{BlockContext, TransactionContext};
use blockifier::execution::call_info::CallInfo;
use blockifier::execution::entry_point::{
    CallEntryPoint, EntryPointExecutionContext, EntryPointExecutionResult,
};
use blockifier::state::cached_state::CachedState;
use blockifier::state::state_api::StateReader;
use blockifier::transaction::objects::{DeprecatedTransactionInfo, TransactionInfo};
use katana_cairo::cairo_vm::vm::runners::cairo_runner::{ExecutionResources, RunResources};
use katana_cairo::starknet_api::core::EntryPointSelector;
use katana_cairo::starknet_api::transaction::Calldata;
use katana_primitives::Felt;

use super::utils::to_blk_address;
use crate::{EntryPointCall, ExecutionError};

/// Perform a function call on a contract and retrieve the return values.
pub fn execute_call<S: StateReader>(
    request: EntryPointCall,
    state: S,
    block_context: &BlockContext,
    max_gas: u64,
) -> Result<Vec<Felt>, ExecutionError> {
    let mut state = CachedState::new(state);
    let res = execute_call_inner(request, &mut state, block_context, max_gas)?;
    Ok(res.execution.retdata.0)
}

fn execute_call_inner<S: StateReader>(
    request: EntryPointCall,
    state: &mut CachedState<S>,
    block_context: &BlockContext,
    max_gas: u64,
) -> EntryPointExecutionResult<CallInfo> {
    let call = CallEntryPoint {
        initial_gas: max_gas,
        calldata: Calldata(Arc::new(request.calldata)),
        storage_address: to_blk_address(request.contract_address),
        entry_point_selector: EntryPointSelector(request.entry_point_selector),
        ..Default::default()
    };

    // The run resources for a call execution will either be constraint ONLY by the block context
    // limits OR based on the tx fee and gas prices. As can be seen here, the upper bound will
    // always be limited the block max invoke steps https://github.com/dojoengine/blockifier/blob/5f58be8961ddf84022dd739a8ab254e32c435075/crates/blockifier/src/execution/entry_point.rs#L253
    // even if the it's max steps is derived from the tx fees.
    //
    // This if statement here determines how execution will be constraint to <https://github.com/dojoengine/blockifier/blob/5f58be8961ddf84022dd739a8ab254e32c435075/crates/blockifier/src/execution/entry_point.rs#L206-L208>.
    // This basically means, we can not set an arbitrary gas limit for this call without modifying
    // the block context. So, we just set the run resources here manually to bypass that.

    // The values for these parameters are essentially useless as we manually set the run resources
    // later anyway.
    let limit_steps_by_resources = true;
    let tx_info = DeprecatedTransactionInfo::default();

    let mut ctx = EntryPointExecutionContext::new_invoke(
        Arc::new(TransactionContext {
            block_context: block_context.clone(),
            tx_info: TransactionInfo::Deprecated(tx_info),
        }),
        limit_steps_by_resources,
    )
    .unwrap();

    // manually override the run resources
    ctx.vm_run_resources = RunResources::new(max_gas as usize);
    call.execute(state, &mut ExecutionResources::default(), &mut ctx)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use blockifier::context::BlockContext;
    use blockifier::state::cached_state::{self};
    use katana_primitives::class::ContractClass;
    use katana_primitives::{address, felt, ContractAddress};
    use katana_provider::test_utils;
    use katana_provider::traits::contract::ContractClassWriter;
    use katana_provider::traits::state::{StateFactoryProvider, StateWriter};
    use starknet::macros::selector;

    use super::execute_call_inner;
    use crate::implementation::blockifier::state::StateProviderDb;
    use crate::EntryPointCall;

    #[test]
    fn max_steps() {
        // -------------------- Preparations -------------------------------

        let json = include_str!("../../../tests/fixtures/call_test.json");
        let class = ContractClass::from_str(json).unwrap();
        let class_hash = class.class_hash().unwrap();
        let casm_hash = class.clone().compile().unwrap().class_hash().unwrap();

        // Initialize provider with the test contract
        let provider = test_utils::test_provider();
        // Declare test contract
        provider.set_class(class_hash, class).unwrap();
        provider.set_compiled_class_hash_of_class_hash(class_hash, casm_hash).unwrap();
        // Deploy test contract
        let address = address!("0x1337");
        provider.set_class_hash_of_contract(address, class_hash).unwrap();

        let state = provider.latest().unwrap();
        let state = StateProviderDb::new(state, Default::default());

        // ---------------------------------------------------------------

        let mut state = cached_state::CachedState::new(state);
        let ctx = BlockContext::create_for_testing();

        let mut req = EntryPointCall {
            calldata: Vec::new(),
            contract_address: address,
            entry_point_selector: selector!("bounded_call"),
        };

        // all the values for the calldata are handpicked as it's difficult to write a function that
        // consumes a specific number of gas

        let max_gas_1 = 1_000_000;
        {
            // ~900,000 gas
            req.calldata = vec![felt!("460")];
            let info = execute_call_inner(req.clone(), &mut state, &ctx, max_gas_1).unwrap();
            assert!(max_gas_1 >= info.execution.gas_consumed);

            req.calldata = vec![felt!("600")];
            let result = execute_call_inner(req.clone(), &mut state, &ctx, max_gas_1);
            assert!(result.is_err(), "should fail due to out of run resources")
        }

        let max_gas_2 = 10_000_000;
        {
            // rougly equivalent to 9,000,000 gas
            req.calldata = vec![felt!("4600")];
            let info = execute_call_inner(req.clone(), &mut state, &ctx, max_gas_2).unwrap();
            assert!(max_gas_2 >= info.execution.gas_consumed);
            assert!(max_gas_1 < info.execution.gas_consumed);

            req.calldata = vec![felt!("5000")];
            let result = execute_call_inner(req.clone(), &mut state, &ctx, max_gas_2);
            assert!(result.is_err(), "should fail due to out of run resources")
        }

        let max_gas_3 = 100_000_000;
        {
            req.calldata = vec![felt!("47000")];
            let info = execute_call_inner(req.clone(), &mut state, &ctx, max_gas_3).unwrap();
            assert!(max_gas_3 >= info.execution.gas_consumed);
            assert!(max_gas_2 < info.execution.gas_consumed);

            req.calldata = vec![felt!("60000")];
            let result = execute_call_inner(req.clone(), &mut state, &ctx, max_gas_3);
            assert!(result.is_err(), "should fail due to out of run resources")
        }

        // Check that 'call' isn't bounded by the block context max invoke steps
        assert!(max_gas_3 > ctx.versioned_constants().invoke_tx_max_n_steps as u64);
    }
}
