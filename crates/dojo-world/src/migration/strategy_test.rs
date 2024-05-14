// use std::{path::PathBuf, str::FromStr};

// use starknet_crypto::FieldElement;

// use crate::migration::contract::{ContractDiff, ContractMigration};

// use super::{generate_salt, MigrationStrategy};

// #[test]
// fn resolving_variable_work_as_expected() {
//     let constructor_calldata = vec![("c1", vec!["$contract_address:dojo::main", "0x0"])];
//     let strategy = migration_strategy(constructor_calldata);
// }

// fn migration_strategy(constructor_calldatas: Vec<(&str, Vec<&str>)>) -> MigrationStrategy {
//     let mut contracts = vec![];
//     for calldata in constructor_calldatas {
//         contracts.push(ContractMigration {
//             salt: generate_salt(calldata.0),
//             diff: ContractDiff {
//                 name: calldata.0.to_string(),
//                 local_class_hash: FieldElement::from_str("0x1").unwrap(),
//                 original_class_hash: FieldElement::from_str("0x2").unwrap(),
//                 base_class_hash: FieldElement::from_str("0x3").unwrap(),
//                 remote_class_hash: None,
//                 constructor_calldata: calldata.1.into_iter().map(|a| a.to_string()).collect(),
//             },
//             artifact_path: PathBuf::new(),
//             contract_address: FieldElement::from_str("0x0").unwrap(),
//         })
//     }
//     MigrationStrategy { world_address: None, world: None, base: None, contracts, models: vec![] }
// }
