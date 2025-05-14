pub const fn default_address() -> starknet::ContractAddress {
    0.try_into().unwrap()
}

pub const fn default_class_hash() -> starknet::ClassHash {
    0.try_into().unwrap()
}

/// Implement the Default trait for ContractAddress to be able to use
/// the Default derive attribute on enums/structs which contain
/// a ContractAddress field.
pub impl ContractAddressDefault of Default<starknet::ContractAddress> {
    fn default() -> starknet::ContractAddress {
        default_address()
    }
}

// Implement the Default trait for ClassHash to be able to use
/// the Default derive attribute on enums/structs which contain
/// a ClassHash field.
pub impl ClassHashDefault of Default<starknet::ClassHash> {
    fn default() -> starknet::ClassHash {
        default_class_hash()
    }
}
