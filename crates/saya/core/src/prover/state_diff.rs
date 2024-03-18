use std::{collections::HashMap, fmt};

use starknet::core::types::FieldElement;

struct StateDiff {
    genesis_state_hash: FieldElement,
    prev_state_hash: FieldElement,
    nonce_updates: HashMap<FieldElement, FieldElement>,
    storage_updates: Vec<(FieldElement, Vec<(FieldElement, FieldElement)>)>,
    contract_updates: Vec<(FieldElement, FieldElement)>,
    declared_classes: Vec<(FieldElement, FieldElement)>,
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

/// We need custom implentation because of dynamic keys in json
impl fmt::Display for StateDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", "{")?;
        write!(f, r#""genesis_state_hash":{}"#, self.genesis_state_hash)?;
        write!(f, r#","prev_state_hash":{}"#, self.prev_state_hash)?;

        write!(f, r#","nonce_updates":{}"#, "{")?;
        let nonce_updates = self
            .nonce_updates
            .iter()
            .map(|(k, v)| format!(r#""{}":{}"#, k, v))
            .collect::<Vec<_>>()
            .join(",");
        write!(f, "{}{}", nonce_updates, "}")?;

        write!(f, r#","storage_updates":{}"#, "{")?;
        let storage_updates = self
            .storage_updates
            .iter()
            .map(|(k, v)| {
                let storage = v
                    .iter()
                    .map(|(k, v)| format!(r#""{}":{}"#, k, v))
                    .collect::<Vec<_>>()
                    .join(",");

                format!(r#""{}":{{{}}}"#, k, storage)
            })
            .collect::<Vec<_>>()
            .join(",");
        write!(f, "{}{}", storage_updates, "}")?;

        write!(f, r#","contract_updates":{}"#, "{")?;
        let contract_updates = self
            .contract_updates
            .iter()
            .map(|(k, v)| format!(r#""{}":{}"#, k, v))
            .collect::<Vec<_>>()
            .join(",");
        write!(f, "{}{}", contract_updates, "}")?;

        write!(f, r#","declared_classes":{}"#, "{")?;
        let declared_classes = self
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

    use starknet::macros::felt;

    use crate::prover::state_diff::{StateDiff, EXAMPLE_STATE_DIFF};

    #[test]
    fn serialize_state_diff() {
        let mut nonce_updates = HashMap::new();
        nonce_updates.insert(felt!("1"), felt!("12"));
        nonce_updates.insert(felt!("2"), felt!("1337"));

        let state_diff = StateDiff {
            genesis_state_hash: felt!("12312321313"),
            prev_state_hash: felt!("34343434343"),
            nonce_updates,
            storage_updates: vec![
                (
                    felt!("1"),
                    vec![(felt!("123456789"), felt!("89")), (felt!("987654321"), felt!("98"))],
                ),
                (
                    felt!("2"),
                    vec![(felt!("123456789"), felt!("899")), (felt!("987654321"), felt!("98"))],
                ),
            ],
            contract_updates: vec![(felt!("3"), felt!("437267489"))],
            declared_classes: vec![
                (felt!("1234"), felt!("12345")),
                (felt!("12345"), felt!("123456")),
                (felt!("123456"), felt!("1234567")),
            ],
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
