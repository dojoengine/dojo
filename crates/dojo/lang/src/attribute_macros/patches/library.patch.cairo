#[starknet::contract]
pub mod $name$ {
    use dojo::contract::IContract;
    use dojo::meta::IDeployedResource;

    #[abi(embed_v0)]
    pub impl $name$__DeployedContractImpl of IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "$name$"
        }
    }

    $body$
}
