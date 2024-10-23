#[starknet::interface]
trait IGetChainId<TContractState> {
    fn get(self: @TContractState) -> felt252;
}

#[starknet::contract]
pub mod GetChainId {
	use starknet::{SyscallResultTrait, syscalls::get_execution_info_v2_syscall};

    #[storage]
    struct Storage { }

    #[abi(embed_v0)]
 	pub impl GetChainId of super::IGetChainId<ContractState> {
        fn get(self: @ContractState) -> felt252 {
	        let execution_info = get_execution_info_v2_syscall().unwrap_syscall();
			let tx_info = execution_info.tx_info.unbox();
			let chain_id = tx_info.chain_id;
			chain_id
        }
    }
}
