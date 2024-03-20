use std::fmt;

use katana_primitives::state::StateUpdates;
use starknet::core::types::FieldElement;

pub struct ProvedStateDiff {
    pub genesis_state_hash: FieldElement,
    pub prev_state_hash: FieldElement,
    pub state_updates: StateUpdates,
}

#[cfg(test)]
pub const EXAMPLE_STATE_DIFF: &str = r#"{
    "genesis_state_hash": 12312321313,
    "prev_state_hash": 34343434343,
    "nonce_updates": {
        "1": 12,
        "2": 1337
    },
    "storage_updates": {
        "1": {
            "123456789": 89,
            "987654321": 98
        },
        "2": {
            "123456789": 899,
            "987654321": 98
        }
    },
    "contract_updates": {
        "3": 437267489
    },
    "declared_classes": {
        "1234": 12345,
        "12345": 123456,
        "123456": 1234567
    }
}"#;

#[cfg(test)]
pub const EXAMPLE_KATANA_DIFF: &str = r#"{
    "genesis_state_hash": 0,
    "prev_state_hash": 0,
    "nonce_updates": {
        "2753027862869584298471002046734263971941226372316454331586763888183773261315": 1
    },
    "storage_updates": {
        "2087021424722619777119509474943472645767659996348769578120564519014510906823": {
            "2080372569135727803323277605537468839623406868880224375222092136867736091483": 9999999366500000000000,
            "3488041066649332616440110253331181934927363442882040970594983370166361489161": 633500000000000
        }
    },
    "contract_updates": {},
    "declared_classes": {
        "2927827620326415540917522810963695348790596370636511605071677066526091865974": 3454128523693959991357220485501659129201494257878487792088502805686335557901
    }
}"#;

/// We need custom implentation because of dynamic keys in json
impl fmt::Display for ProvedStateDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", "{")?;
        write!(f, r#""genesis_state_hash":{}"#, self.genesis_state_hash)?;
        write!(f, r#","prev_state_hash":{}"#, self.prev_state_hash)?;

        write!(f, r#","nonce_updates":{}"#, "{")?;
        let nonce_updates = self
            .state_updates
            .nonce_updates
            .iter()
            .map(|(k, v)| format!(r#""{}":{}"#, k.0, v))
            .collect::<Vec<_>>()
            .join(",");
        write!(f, "{}{}", nonce_updates, "}")?;

        write!(f, r#","storage_updates":{}"#, "{")?;
        let storage_updates = self
            .state_updates
            .storage_updates
            .iter()
            .map(|(k, v)| {
                let storage = v
                    .iter()
                    .map(|(k, v)| format!(r#""{}":{}"#, k, v))
                    .collect::<Vec<_>>()
                    .join(",");

                format!(r#""{}":{{{}}}"#, k.0, storage)
            })
            .collect::<Vec<_>>()
            .join(",");
        write!(f, "{}{}", storage_updates, "}")?;

        write!(f, r#","contract_updates":{}"#, "{")?;
        let contract_updates = self
            .state_updates
            .contract_updates
            .iter()
            .map(|(k, v)| format!(r#""{}":{}"#, k.0, v))
            .collect::<Vec<_>>()
            .join(",");
        write!(f, "{}{}", contract_updates, "}")?;

        write!(f, r#","declared_classes":{}"#, "{")?;
        let declared_classes = self
            .state_updates
            .declared_classes
            .iter()
            .map(|(k, v)| format!(r#""{}":{}"#, k, v))
            .collect::<Vec<_>>()
            .join(",");

        write!(f, "{}{}", declared_classes, "}")?;

        write!(f, "{}", "}")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use katana_primitives::contract::ContractAddress;
    use starknet::macros::felt;

    use super::*;

    #[test]
    fn serialize_state_diff() {
        let mut nonce_updates = HashMap::new();
        nonce_updates.insert(ContractAddress::from(felt!("1")), felt!("12"));
        nonce_updates.insert(ContractAddress::from(felt!("2")), felt!("1337"));

        let state_diff = ProvedStateDiff {
            genesis_state_hash: felt!("12312321313"),
            prev_state_hash: felt!("34343434343"),
            state_updates: StateUpdates {
                nonce_updates,
                storage_updates: vec![
                    (
                        ContractAddress::from(felt!("1")),
                        vec![(felt!("123456789"), felt!("89")), (felt!("987654321"), felt!("98"))]
                            .into_iter()
                            .collect(),
                    ),
                    (
                        ContractAddress::from(felt!("2")),
                        vec![(felt!("123456789"), felt!("899")), (felt!("987654321"), felt!("98"))]
                            .into_iter()
                            .collect(),
                    ),
                ]
                .into_iter()
                .collect(),
                contract_updates: vec![(ContractAddress::from(felt!("3")), felt!("437267489"))]
                    .into_iter()
                    .collect(),
                declared_classes: vec![
                    (felt!("1234"), felt!("12345")),
                    (felt!("12345"), felt!("123456")),
                    (felt!("123456"), felt!("1234567")),
                ]
                .into_iter()
                .collect(),
            },
        };

        // let serialized = serde_json::to_string(&state_diff).unwrap();
        let serialized = state_diff.to_string();

        // remove brackets from values
        let processed = serialized
            .replace(r#":\""#, ":")
            .replace(r#"\","#, ",")
            .replace(r#"\"}"#, "}")
            .replace(r#"\"]"#, "]");

        let unreadable_example = EXAMPLE_STATE_DIFF.replace(" ", "").replace("\n", "");

        assert_eq!(processed, unreadable_example);
    }
}
