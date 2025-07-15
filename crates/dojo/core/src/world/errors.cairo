use starknet::ContractAddress;

pub const DELETE_ENTITY_MEMBER: felt252 = 'Cannot delete entity member';
pub const DELETE_ENTITY_SCHEMA: felt252 = 'Cannot delete entity schema';

pub fn lengths_mismatch(a: @ByteArray, b: @ByteArray, context: @ByteArray) -> ByteArray {
    format!("Length mismatch: `{a}` and `{b}` in `{context}`")
}

pub fn not_writer(contract_tag: @ByteArray, on_type: @ByteArray, on_tag: @ByteArray) -> ByteArray {
    format!("Caller `{}` has no write access on {} `{}`", contract_tag, on_type, on_tag)
}

pub fn contract_already_initialized(contract_tag: @ByteArray) -> ByteArray {
    format!("Contract `{}` has already been initialized", contract_tag)
}

pub fn namespace_already_registered(namespace: @ByteArray) -> ByteArray {
    format!("Namespace `{}` is already registered", namespace)
}

pub fn namespace_not_registered(namespace: @ByteArray) -> ByteArray {
    format!("Namespace `{}` is not registered", namespace)
}

pub fn no_namespace_write_access(caller: ContractAddress, namespace: @ByteArray) -> ByteArray {
    format!("Caller `{:?}` has no write access on namespace `{}`", caller, namespace)
}

pub fn event_already_registered(namespace: @ByteArray, name: @ByteArray) -> ByteArray {
    format!("Resource (Event) `{}-{}` is already registered", namespace, name)
}

pub fn model_already_registered(namespace: @ByteArray, name: @ByteArray) -> ByteArray {
    format!("Resource (Model) `{}-{}` is already registered", namespace, name)
}

pub fn contract_already_registered(namespace: @ByteArray, name: @ByteArray) -> ByteArray {
    format!("Resource (Contract) `{}-{}` is already registered", namespace, name)
}

pub fn external_contract_already_registered(
    namespace: @ByteArray, contract_name: @ByteArray, instance_name: @ByteArray,
) -> ByteArray {
    format!(
        "Resource (External Contract) `{}-{} ({})` is already registered",
        namespace,
        instance_name,
        contract_name,
    )
}

pub fn library_already_registered(namespace: @ByteArray, name: @ByteArray) -> ByteArray {
    format!(
        "Resource (Library) `{}-{}` is already registered. Libraries can't be updated, increment the version in the Dojo configuration file instead.",
        namespace,
        name,
    )
}

pub fn resource_not_registered_details(namespace: @ByteArray, name: @ByteArray) -> ByteArray {
    format!("Resource `{}-{}` is not registered", namespace, name)
}

pub fn resource_not_registered(resource: felt252) -> ByteArray {
    format!("Resource `{}` is not registered", resource)
}

pub fn not_owner(caller: ContractAddress, resource: felt252) -> ByteArray {
    format!("Caller `{:?}` is not the owner of the resource `{}`", caller, resource)
}

pub fn not_owner_upgrade(caller: ContractAddress, resource: felt252) -> ByteArray {
    format!("Caller `{:?}` cannot upgrade the resource `{}` (not owner)", caller, resource)
}

pub fn caller_not_account(caller: ContractAddress) -> ByteArray {
    format!("Caller `{:?}` is not an account", caller)
}

pub fn invalid_resource_selector(selector: felt252) -> ByteArray {
    format!("Invalid resource selector `{}`", selector)
}

pub fn resource_conflict(name: @ByteArray, expected_type: @ByteArray) -> ByteArray {
    format!("Resource `{}` is registered but not as {}", name, expected_type)
}

pub fn no_model_write_access(tag: @ByteArray, caller: ContractAddress) -> ByteArray {
    format!("Caller `{:?}` has no write access on model `{}`", caller, tag)
}

pub fn no_world_owner(caller: ContractAddress, target: @ByteArray) -> ByteArray {
    format!("Caller `{:?}` can't {} (not world owner)", caller, target)
}

pub fn invalid_naming(kind: ByteArray, what: @ByteArray) -> ByteArray {
    format!("{kind} `{what}` is invalid according to Dojo naming rules: ^[a-zA-Z0-9_]+$")
}

pub fn invalid_resource_schema_upgrade(namespace: @ByteArray, name: @ByteArray) -> ByteArray {
    format!("Invalid new schema to upgrade the resource `{}-{}`", namespace, name)
}

pub fn packed_layout_cannot_be_upgraded(namespace: @ByteArray, name: @ByteArray) -> ByteArray {
    format!("Packed layout cannot be upgraded `{}-{}`", namespace, name)
}

pub fn invalid_resource_layout_upgrade(namespace: @ByteArray, name: @ByteArray) -> ByteArray {
    format!("Invalid new layout to upgrade the resource `{}-{}`", namespace, name)
}

pub fn invalid_resource_version_upgrade(
    namespace: @ByteArray, name: @ByteArray, expected_version: u8,
) -> ByteArray {
    format!("The new resource version of `{}-{}` should be {}", namespace, name, expected_version)
}

pub fn inconsistent_namespaces(old_hash: felt252, new_hash: felt252) -> ByteArray {
    format!("Inconsistent namespaces (old: {old_hash} new: {new_hash}")
}
