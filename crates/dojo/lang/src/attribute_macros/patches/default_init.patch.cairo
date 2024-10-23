#[abi(per_item)]
#[generate_trait]
pub impl IDojoInitImpl of IDojoInit {
    #[external(v0)]
    fn $init_name$(self: @ContractState) {
        if starknet::get_caller_address() != self.world_provider.world().contract_address {
            core::panics::panic_with_byte_array(
                @format!("Only the world can init contract `{}`, but caller is `{:?}`",
                self.dojo_name(),
                starknet::get_caller_address(),
            ));
        }
    }
}
