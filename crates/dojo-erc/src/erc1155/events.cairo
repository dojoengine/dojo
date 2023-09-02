use clone::Clone;
use serde::Serde;
use traits::PartialEq;
use starknet::ContractAddress;
use dojo_erc::erc_common::utils::PartialEqArray;

#[derive(Clone, Drop, Serde, PartialEq, starknet::Event)]
struct TransferSingle {
    operator: ContractAddress,
    from: ContractAddress,
    to: ContractAddress,
    id: u256,
    value: u256
}

#[derive(Clone, Drop, Serde, PartialEq, starknet::Event)]
struct TransferBatch {
    operator: ContractAddress,
    from: ContractAddress,
    to: ContractAddress,
    ids: Array<u256>,
    values: Array<u256>
}

#[derive(Clone, Drop, Serde, PartialEq, starknet::Event)]
struct ApprovalForAll {
    owner: ContractAddress,
    operator: ContractAddress,
    approved: bool
}

//
// DOJO Events 
//

#[derive(Clone, Drop, Serde, PartialEq, starknet::Event)]
struct DojoTransferSingle {
    contract_address: ContractAddress,
    operator: ContractAddress,
    from: ContractAddress,
    to: ContractAddress,
    id: u256,
    value: u256
}

#[derive(Clone, Drop, Serde, PartialEq, starknet::Event)]
struct DojoTransferBatch {
    contract_address: ContractAddress,
    operator: ContractAddress,
    from: ContractAddress,
    to: ContractAddress,
    ids: Array<u256>,
    values: Array<u256>
}

#[derive(Clone, Drop, Serde, PartialEq, starknet::Event)]
struct DojoApprovalForAll {
    contract_address: ContractAddress,
    owner: ContractAddress,
    operator: ContractAddress,
    approved: bool
}
