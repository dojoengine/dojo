//! ABI handling utilities for Dojo resources.

use dojo_world::local::{ResourceLocal, WorldLocal};
use rmcp::Error as McpError;
use serde_json::json;

/// Get contract ABI as JSON string
pub fn get_contract_abi(world: &WorldLocal, contract_name: &str) -> Result<String, McpError> {
    let contract = world
        .resources
        .values()
        .find_map(|r| r.as_contract())
        .filter(|c| c.common.name == contract_name)
        .ok_or_else(|| {
            McpError::resource_not_found(
                "contract_not_found",
                Some(json!({ "contract_name": contract_name })),
            )
        })?;

    serde_json::to_string_pretty(&contract.common.class.abi).map_err(|e| {
        McpError::internal_error(
            "abi_serialization_failed",
            Some(json!({ "reason": format!("Failed to serialize contract ABI: {}", e) })),
        )
    })
}

/// Get model ABI as JSON string
pub fn get_model_abi(world: &WorldLocal, model_name: &str) -> Result<String, McpError> {
    let model = world
        .resources
        .values()
        .find_map(|r| match r {
            ResourceLocal::Model(m) => Some(m),
            _ => None,
        })
        .filter(|m| m.common.name == model_name)
        .ok_or_else(|| {
            McpError::resource_not_found(
                "model_not_found",
                Some(json!({ "model_name": model_name })),
            )
        })?;

    serde_json::to_string_pretty(&model.common.class.abi).map_err(|e| {
        McpError::internal_error(
            "abi_serialization_failed",
            Some(json!({ "reason": format!("Failed to serialize model ABI: {}", e) })),
        )
    })
}

/// Get event ABI as JSON string
pub fn get_event_abi(world: &WorldLocal, event_name: &str) -> Result<String, McpError> {
    let event = world
        .resources
        .values()
        .find_map(|r| match r {
            ResourceLocal::Event(e) => Some(e),
            _ => None,
        })
        .filter(|e| e.common.name == event_name)
        .ok_or_else(|| {
            McpError::resource_not_found(
                "event_not_found",
                Some(json!({ "event_name": event_name })),
            )
        })?;

    serde_json::to_string_pretty(&event.common.class.abi).map_err(|e| {
        McpError::internal_error(
            "abi_serialization_failed",
            Some(json!({ "reason": format!("Failed to serialize event ABI: {}", e) })),
        )
    })
}
