use katana_primitives::trace::{CallInfo, TxExecInfo};
use katana_primitives::transaction::TxHash;
use serde::{Deserialize, Serialize};
use starknet::core::types::{
    CallType, ComputationResources, EntryPointType, OrderedEvent, OrderedMessage,
};

#[derive(Debug)]
pub struct FunctionInvocation(pub starknet::core::types::FunctionInvocation);

impl From<CallInfo> for FunctionInvocation {
    fn from(info: CallInfo) -> Self {
        let entry_point_type = match info.entry_point_type {
            katana_primitives::trace::EntryPointType::External => EntryPointType::External,
            katana_primitives::trace::EntryPointType::L1Handler => EntryPointType::L1Handler,
            katana_primitives::trace::EntryPointType::Constructor => EntryPointType::Constructor,
        };

        let call_type = match info.call_type {
            katana_primitives::trace::CallType::Call => CallType::Call,
            katana_primitives::trace::CallType::Delegate => CallType::Delegate,
        };

        let calls = info.inner_calls.into_iter().map(|c| FunctionInvocation::from(c).0).collect();

        let events = info
            .events
            .into_iter()
            .map(|e| OrderedEvent { order: e.order, data: e.data, keys: e.keys })
            .collect();

        let messages = info
            .l2_to_l1_messages
            .into_iter()
            .map(|m| OrderedMessage {
                order: m.order,
                payload: m.payload,
                to_address: m.to_address,
                from_address: m.from_address.into(),
            })
            .collect();

        let resources = info.execution_resources;

        // TODO: replace execution resources type in primitive CallInfo with an already defined
        // `TxExecutionResources`
        let execution_resources = ComputationResources {
            steps: resources.n_steps as u64,
            memory_holes: Some(resources.n_memory_holes as u64),
            range_check_builtin_applications: resources.builtin_instance_counter.range_check(),
            pedersen_builtin_applications: resources.builtin_instance_counter.pedersen(),
            poseidon_builtin_applications: resources.builtin_instance_counter.poseidon(),
            ec_op_builtin_applications: resources.builtin_instance_counter.ec_op(),
            ecdsa_builtin_applications: resources.builtin_instance_counter.ecdsa(),
            bitwise_builtin_applications: resources.builtin_instance_counter.bitwise(),
            keccak_builtin_applications: resources.builtin_instance_counter.keccak(),
            segment_arena_builtin: resources.builtin_instance_counter.segment_arena(),
        };

        Self(starknet::core::types::FunctionInvocation {
            calls,
            events,
            messages,
            call_type,
            entry_point_type,
            execution_resources,
            result: info.retdata,
            calldata: info.calldata,
            caller_address: info.caller_address.into(),
            contract_address: info.contract_address.into(),
            entry_point_selector: info.entry_point_selector,
            // See <https://github.com/starkware-libs/blockifier/blob/cb464f5ac2ada88f2844d9f7d62bd6732ceb5b2c/crates/blockifier/src/execution/call_info.rs#L220>
            class_hash: info.class_hash.expect("Class hash mut be set after execution"),
        })
    }
}

/// The type returned by the `saya_getTransactionExecutionsByBlock` RPC method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxExecutionInfo {
    /// The transaction hash.
    pub hash: TxHash,
    /// The transaction execution trace.
    pub trace: TxExecInfo,
}
