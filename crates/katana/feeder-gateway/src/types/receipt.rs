use std::collections::HashMap;

use katana_primitives::receipt::{Event, MessageToL1};
use katana_primitives::trace::{self, BuiltinCounters, BuiltinName};
use katana_primitives::{eth, Felt};
use serde::Deserialize;

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct ConfirmedReceipt {
    pub transaction_hash: Felt,
    pub transaction_index: u64,
    pub execution_status: Option<ExecutionStatus>,
    pub revert_error: Option<String>,
    pub execution_resources: Option<ExecutionResources>,
    pub l1_to_l2_consumed_message: Option<L1ToL2Message>,
    pub l2_to_l1_messages: Vec<MessageToL1>,
    pub events: Vec<Event>,
    pub actual_fee: Felt,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionStatus {
    Succeeded,
    Reverted,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ExecutionResources {
    pub vm_resources: trace::ExecutionResources,
    pub data_availability: Option<DataAvailabilityResources>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct DataAvailabilityResources {
    pub l1_gas: u64,
    pub l1_data_gas: u64,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct L1ToL2Message {
    /// The address of the Ethereum (L1) contract that sent the message.
    pub from_address: eth::Address,
    pub to_address: Felt,
    pub selector: Felt,
    pub payload: Vec<Felt>,
    pub nonce: Option<Felt>,
}

// The reason why we implement `Deserialize` manually is because we want to avoid defining redundant
// types just because the format is different than the already existing types.
impl<'de> Deserialize<'de> for ExecutionResources {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct BuiltinCounterHelper(BuiltinCounters);

        impl<'de> Deserialize<'de> for BuiltinCounterHelper {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                enum Field {
                    Ignore,
                    Valid(BuiltinName),
                }

                struct FieldVisitor;

                impl<'de> serde::de::Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(
                        &self,
                        formatter: &mut std::fmt::Formatter<'_>,
                    ) -> std::fmt::Result {
                        write!(
                            formatter,
                            "a builtin type name: 'ecdsa_builtin', 'ec_op_builtin', \
                             'keccak_builtin', 'output_builtin', 'bitwise_builtin', \
                             'pedersen_builtin', 'poseidon_builtin', 'range_check_builtin', \
                             'segment_arena_builtin'"
                        )
                    }

                    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                        match v {
                            "ecdsa_builtin" => Ok(Field::Valid(BuiltinName::ecdsa)),
                            "ec_op_builtin" => Ok(Field::Valid(BuiltinName::ec_op)),
                            "keccak_builtin" => Ok(Field::Valid(BuiltinName::keccak)),
                            "output_builtin" => Ok(Field::Valid(BuiltinName::output)),
                            "bitwise_builtin" => Ok(Field::Valid(BuiltinName::bitwise)),
                            "pedersen_builtin" => Ok(Field::Valid(BuiltinName::pedersen)),
                            "poseidon_builtin" => Ok(Field::Valid(BuiltinName::poseidon)),
                            "range_check_builtin" => Ok(Field::Valid(BuiltinName::range_check)),
                            "segment_arena_builtin" => Ok(Field::Valid(BuiltinName::segment_arena)),
                            _ => Ok(Field::Ignore),
                        }
                    }
                }

                impl<'de> serde::Deserialize<'de> for Field {
                    #[inline]
                    fn deserialize<D: serde::Deserializer<'de>>(
                        deserializer: D,
                    ) -> Result<Self, D::Error> {
                        serde::Deserializer::deserialize_identifier(deserializer, FieldVisitor)
                    }
                }

                struct Visitor;

                impl<'de> serde::de::Visitor<'de> for Visitor {
                    type Value = BuiltinCounterHelper;

                    fn expecting(
                        &self,
                        formatter: &mut std::fmt::Formatter<'_>,
                    ) -> std::fmt::Result {
                        write!(
                            formatter,
                            "an JSON object with builtin names as keys and instance counts as \
                             values"
                        )
                    }

                    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
                    where
                        A: serde::de::MapAccess<'de>,
                    {
                        let mut builtins: HashMap<BuiltinName, usize> = HashMap::new();
                        while let Some(key) = map.next_key::<Field>()? {
                            match key {
                                Field::Valid(builtin) => {
                                    if builtins.contains_key(&builtin) {
                                        return Err(
                                            <A::Error as serde::de::Error>::duplicate_field(
                                                builtin.to_str_with_suffix(),
                                            ),
                                        );
                                    }

                                    if let Some(counter) = map.next_value::<Option<u64>>()? {
                                        builtins.insert(builtin, counter as usize);
                                    }
                                }

                                Field::Ignore => {
                                    let _ = map.next_value::<serde::de::IgnoredAny>()?;
                                }
                            }
                        }
                        Ok(BuiltinCounterHelper(BuiltinCounters::from(builtins)))
                    }
                }

                deserializer.deserialize_map(Visitor)
            }
        }

        #[derive(Deserialize)]
        pub struct Helper {
            pub n_steps: usize,
            pub n_memory_holes: usize,
            pub builtin_instance_counter: BuiltinCounterHelper,
            pub data_availability: Option<DataAvailabilityResources>,
        }

        let helper = Helper::deserialize(deserializer)?;

        Ok(Self {
            data_availability: helper.data_availability,
            vm_resources: trace::ExecutionResources {
                n_steps: helper.n_steps,
                n_memory_holes: helper.n_memory_holes,
                builtin_instance_counter: helper.builtin_instance_counter.0,
            },
        })
    }
}
