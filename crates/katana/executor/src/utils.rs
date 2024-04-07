use std::collections::HashMap;

use convert_case::{Case, Casing};
use katana_primitives::receipt::Event;
use tracing::trace;

pub(crate) const LOG_TARGET: &str = "executor";

pub fn log_resources(resources: &HashMap<String, u64>) {
    let mut mapped_strings = resources
        .iter()
        .filter_map(|(k, v)| match k.as_str() {
            "n_steps" => None,
            "ecdsa_builtin" => Some(format!("ECDSA: {v}")),
            "l1_gas_usage" => Some(format!("L1 Gas: {v}")),
            "keccak_builtin" => Some(format!("Keccak: {v}")),
            "bitwise_builtin" => Some(format!("Bitwise: {v}")),
            "pedersen_builtin" => Some(format!("Pedersen: {v}")),
            "range_check_builtin" => Some(format!("Range Checks: {v}")),
            _ => Some(format!("{}: {}", k.to_case(Case::Title), v)),
        })
        .collect::<Vec<String>>();

    // Sort the strings alphabetically
    mapped_strings.sort();

    // Prepend "Steps" if it exists, so it is always first
    if let Some(steps) = resources.get("n_steps") {
        mapped_strings.insert(0, format!("Steps: {}", steps));
    }

    trace!(target: LOG_TARGET, usage = mapped_strings.join(" | "), "Transaction resource usage.");
}

pub fn log_events(events: &[Event]) {
    for e in events {
        trace!(
            target: LOG_TARGET,
            keys = e.keys.iter().map(|key| format!("{key:#x}")).collect::<Vec<_>>().join(", "),
            "Event emitted.",
        );
    }
}
