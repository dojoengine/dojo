use starknet::core::types::contract::{SierraClass, SierraClassDebugInfo};
use starknet::core::types::EntryPointsByType;

pub fn empty_sierra_class() -> SierraClass {
    SierraClass {
        abi: vec![],
        sierra_program: vec![],
        sierra_program_debug_info: SierraClassDebugInfo {
            type_names: vec![],
            libfunc_names: vec![],
            user_func_names: vec![],
        },
        contract_class_version: "0".to_string(),
        entry_points_by_type: EntryPointsByType {
            constructor: vec![],
            external: vec![],
            l1_handler: vec![],
        },
    }
}
