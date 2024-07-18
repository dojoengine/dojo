use starknet::ContractAddress;

#[starknet::interface]
pub trait IConfig<T> {
    /// Sets the information of the program that generates the
    /// state transition trace (namely DojoOS).
    ///
    /// # Arguments
    ///
    /// * `program_hash` - The program hash.
    /// * `config_hash` - The program's config hash.
    fn set_differ_program_hash(ref self: T, program_hash: felt252);
    fn set_merger_program_hash(ref self: T, program_hash: felt252);

    /// Gets the information of the program that generates the
    /// state transition trace (namely DojoOS).
    ///
    /// # Returns
    ///
    /// The program hash and it's configuration hash.
    fn get_differ_program_hash(self: @T) -> felt252;
    fn get_merger_program_hash(self: @T) -> felt252;

    /// Sets the facts registry contract address, which is already
    /// initialized with the verifier information.
    ///
    /// # Arguments
    ///
    /// * `address` - The facts registry contract's address.
    fn set_facts_registry(ref self: T, address: ContractAddress);

    /// Gets the facts registry contract address.
    ///
    /// # Returns
    ///
    /// The contract address of the facts registry.
    fn get_facts_registry(self: @T) -> ContractAddress;
}

