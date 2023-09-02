use clone::Clone;
use serde::Serde;
use traits::PartialEq;
use starknet::ContractAddress;

#[derive(Clone, Drop, Serde, PartialEq, starknet::Event)]
struct Transfer {
    from: ContractAddress,
    to: ContractAddress,
    token_id: u256
}

#[derive(Clone, Drop, Serde, PartialEq, starknet::Event)]
struct Approval {
    owner: ContractAddress,
    to: ContractAddress,
    token_id: u256
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
struct DojoTransfer {
    contract_address: ContractAddress,
    from: ContractAddress,
    to: ContractAddress,
    token_id: u256
}

#[derive(Clone, Drop, Serde, PartialEq, starknet::Event)]
struct DojoApproval {
    contract_address: ContractAddress,
    owner: ContractAddress,
    to: ContractAddress,
    token_id: u256
}

#[derive(Clone, Drop, Serde, PartialEq, starknet::Event)]
struct DojoApprovalForAll {
    contract_address: ContractAddress,
    owner: ContractAddress,
    operator: ContractAddress,
    approved: bool
}
