use starknet::ContractAddress;

pub const DELETE_ENTITY_MEMBER: felt252 = 'Cannot delete entity member';

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
    format!("Resource `{}-{}` is already registered", namespace, name)
}

pub fn event_not_registered(namespace: @ByteArray, name: @ByteArray) -> ByteArray {
    format!("Resource `{}-{}` is not registered", namespace, name)
}

pub fn model_already_registered(namespace: @ByteArray, name: @ByteArray) -> ByteArray {
    format!("Resource `{}-{}` is already registered", namespace, name)
}

pub fn contract_already_registered(namespace: @ByteArray, name: @ByteArray) -> ByteArray {
    format!("Resource `{}-{}` is already registered", namespace, name)
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
    format!("Resource `{}` is registered but not as a {}", name, expected_type)
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
