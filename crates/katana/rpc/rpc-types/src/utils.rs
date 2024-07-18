use katana_cairo::cairo_vm::types::builtin_name::BuiltinName;
use katana_primitives::trace::ExecutionResources;

pub fn get_builtin_instance_count(value: &ExecutionResources, name: BuiltinName) -> Option<u64> {
    value.builtin_instance_counter.get(&name).map(|&v| v as u64)
}
