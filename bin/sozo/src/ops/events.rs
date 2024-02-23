use std::collections::{HashMap, VecDeque};

use crate::commands::events::EventsArgs;
use anyhow::Result;
use cainome::parser::tokens::{CompositeInnerKind, CoreBasic, Token};
use dojo_world::metadata::Environment;
use starknet::core::types::FieldElement;
use starknet::core::types::{BlockId, EventFilter};
use starknet::core::utils::{parse_cairo_short_string, starknet_keccak};
use starknet::providers::Provider;

pub async fn execute(
    args: EventsArgs,
    env_metadata: Option<Environment>,
    events_map: Option<HashMap<String, Vec<Token>>>,
) -> Result<()> {
    let EventsArgs {
        chunk_size,
        starknet,
        world,
        from_block,
        to_block,
        events,
        continuation_token,
        ..
    } = args;

    let from_block = from_block.map(BlockId::Number);
    let to_block = to_block.map(BlockId::Number);
    // Currently dojo doesn't use custom keys for events. In future if custom keys are used this
    // needs to be updated for granular queries.
    let keys =
        events.map(|e| vec![e.iter().map(|event| starknet_keccak(event.as_bytes())).collect()]);

    let provider = starknet.provider(env_metadata.as_ref())?;
    let event_filter = EventFilter { from_block, to_block, address: world.world_address, keys };

    let res = provider.get_events(event_filter, continuation_token, chunk_size).await?;

    if let Some(events_map) = events_map {
        parse_and_print_events(res, events_map)?;
    }

    Ok(())
}

fn parse_and_print_events(
    res: starknet::core::types::EventsPage,
    events_map: HashMap<String, Vec<Token>>,
) -> Result<()> {
    println!("Continuation token: {:?}", res.continuation_token);
    println!("----------------------------------------------");
    for event in res.events {
        if let Some(e) = parse_event(event.clone(), &events_map) {
            println!("{e}");
        }
    }
    Ok(())
}

fn parse_core_basic(
    cb: &CoreBasic,
    value: &FieldElement,
    include_felt_string: bool,
) -> Result<String, String> {
    match cb.type_name().as_str() {
        "felt252" => {
            let hex = format!("{:#x}", value);
            match parse_cairo_short_string(value) {
                Ok(parsed) if !parsed.is_empty() && include_felt_string => {
                    Ok(format!("{} \"{}\"", parsed, hex))
                }
                _ => Ok(format!("\"{}\"", hex)),
            }
        }
        "bool" => {
            if *value == FieldElement::ZERO {
                Ok("false".to_string())
            } else {
                Ok("true".to_string())
            }
        }
        "ClassHash" | "ContractAddress" => Ok(format!("{:#x}", value)),
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64" | "i128" => {
            Ok(value.to_string())
        }
        _ => Err(format!("Unsupported CoreBasic type: {}", cb.type_name())),
    }
}

fn parse_event(
    event: starknet::core::types::EmittedEvent,
    events_map: &HashMap<String, Vec<Token>>,
) -> Option<String> {
    let mut data = VecDeque::from(event.data.clone());
    let mut keys = VecDeque::from(event.keys.clone());
    let event_hash = keys.pop_front()?;

    let events = events_map.get(&event_hash.to_string())?;

    'outer: for e in events {
        let mut ret = format!("Event name: {}\n", e.type_path());

        if let Token::Composite(composite) = e {
            for inner in &composite.inners {
                let result: Result<_, &'static str> = match inner.kind {
                    CompositeInnerKind::Data => data.pop_front().ok_or("Missing data value"),
                    CompositeInnerKind::Key => keys.pop_front().ok_or("Missing key value"),
                    _ => Err("Unsupported inner kind encountered"),
                };

                let value = match result {
                    Ok(val) => val,
                    Err(_) => continue 'outer,
                };

                let formatted_value = match &inner.token {
                    Token::CoreBasic(ref cb) => match parse_core_basic(cb, &value, true) {
                        Ok(parsed_value) => parsed_value,
                        Err(_) => continue 'outer,
                    },
                    Token::Array(ref array) => {
                        let length = match value.to_string().parse::<usize>() {
                            Ok(len) => len,
                            Err(_) => continue 'outer,
                        };

                        let cb = if let Token::CoreBasic(ref cb) = *array.inner {
                            cb
                        } else {
                            continue 'outer;
                        };

                        let mut elements = Vec::new();
                        for _ in 0..length {
                            if let Some(element_value) = data.pop_front() {
                                match parse_core_basic(cb, &element_value, false) {
                                    Ok(element_str) => elements.push(element_str),
                                    Err(_) => continue 'outer,
                                };
                            } else {
                                continue 'outer;
                            }
                        }

                        format!("[{}]", elements.join(", "))
                    }
                    _ => continue 'outer,
                };
                ret.push_str(&format!("{}: {}\n", inner.name, formatted_value));
            }
            return Some(ret);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use cainome::parser::tokens::{Array, Composite, CompositeInner, CompositeType};
    use starknet::core::types::EmittedEvent;

    use super::*;

    #[test]
    fn test_core_basic() {
        let composite = Composite {
            type_path: "dojo::world::world::TestEvent".to_string(),
            inners: vec![
                CompositeInner {
                    index: 0,
                    name: "felt252".to_string(),
                    kind: CompositeInnerKind::Data,
                    token: Token::CoreBasic(CoreBasic { type_path: "core::felt252".to_string() }),
                },
                CompositeInner {
                    index: 1,
                    name: "bool".to_string(),
                    kind: CompositeInnerKind::Data,
                    token: Token::CoreBasic(CoreBasic { type_path: "core::bool".to_string() }),
                },
                CompositeInner {
                    index: 2,
                    name: "u8".to_string(),
                    kind: CompositeInnerKind::Data,
                    token: Token::CoreBasic(CoreBasic {
                        type_path: "core::integer::u8".to_string(),
                    }),
                },
                CompositeInner {
                    index: 3,
                    name: "u16".to_string(),
                    kind: CompositeInnerKind::Data,
                    token: Token::CoreBasic(CoreBasic {
                        type_path: "core::integer::u16".to_string(),
                    }),
                },
                CompositeInner {
                    index: 4,
                    name: "u32".to_string(),
                    kind: CompositeInnerKind::Data,
                    token: Token::CoreBasic(CoreBasic {
                        type_path: "core::integer::u32".to_string(),
                    }),
                },
                CompositeInner {
                    index: 5,
                    name: "u64".to_string(),
                    kind: CompositeInnerKind::Data,
                    token: Token::CoreBasic(CoreBasic {
                        type_path: "core::integer::u64".to_string(),
                    }),
                },
                CompositeInner {
                    index: 6,
                    name: "u128".to_string(),
                    kind: CompositeInnerKind::Data,
                    token: Token::CoreBasic(CoreBasic {
                        type_path: "core::integer::u128".to_string(),
                    }),
                },
                CompositeInner {
                    index: 7,
                    name: "usize".to_string(),
                    kind: CompositeInnerKind::Data,
                    token: Token::CoreBasic(CoreBasic {
                        type_path: "core::integer::usize".to_string(),
                    }),
                },
                CompositeInner {
                    index: 8,
                    name: "class_hash".to_string(),
                    kind: CompositeInnerKind::Data,
                    token: Token::CoreBasic(CoreBasic { type_path: "core::ClassHash".to_string() }),
                },
                CompositeInner {
                    index: 9,
                    name: "contract_address".to_string(),
                    kind: CompositeInnerKind::Data,
                    token: Token::CoreBasic(CoreBasic {
                        type_path: "core::ContractAddress".to_string(),
                    }),
                },
            ],
            generic_args: vec![],
            r#type: CompositeType::Struct,
            is_event: true,
            alias: None,
        };
        let tokenized_composite = Token::Composite(composite);

        let mut events_map = HashMap::new();
        events_map
            .insert(starknet_keccak("TestEvent".as_bytes()).to_string(), vec![tokenized_composite]);

        let event = EmittedEvent {
            keys: vec![starknet_keccak("TestEvent".as_bytes())],
            data: vec![
                FieldElement::from_hex_be("0x5465737431").expect("Invalid hex"),
                FieldElement::from(1u8), // bool true
                FieldElement::from(1u8),
                FieldElement::from(2u16),
                FieldElement::from(3u32),
                FieldElement::from(4u64),
                FieldElement::from(5u128),
                FieldElement::from(6usize),
                FieldElement::from_hex_be("0x54657374").expect("Invalid hex"),
                FieldElement::from_hex_be("0x54657374").expect("Invalid hex"),
            ],
            from_address: FieldElement::from_hex_be("0x123").expect("Invalid hex"),
            block_hash: FieldElement::from_hex_be("0x456").expect("Invalid hex"),
            block_number: 1,
            transaction_hash: FieldElement::from_hex_be("0x789").expect("Invalid hex"),
        };

        let expected_output =
            format!("Event name: dojo::world::world::TestEvent\nfelt252: Test1 \"0x5465737431\"\nbool: true\nu8: 1\nu16: 2\nu32: 3\nu64: 4\nu128: 5\nusize: 6\nclass_hash: 0x54657374\ncontract_address: 0x54657374\n");

        let actual_output = parse_event(event.clone(), &events_map)
            .unwrap_or_else(|| "Couldn't parse event".to_string());

        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_array() {
        let composite = Composite {
            type_path: "dojo::world::world::StoreDelRecord".to_string(),
            inners: vec![
                CompositeInner {
                    index: 0,
                    name: "table".to_string(),
                    kind: CompositeInnerKind::Data,
                    token: Token::CoreBasic(CoreBasic { type_path: "core::felt252".to_string() }),
                },
                CompositeInner {
                    index: 1,
                    name: "keys".to_string(),
                    kind: CompositeInnerKind::Data,
                    token: Token::Array(Array {
                        type_path: "core::array::Span::<core::felt252>".to_string(),
                        inner: Box::new(Token::CoreBasic(CoreBasic {
                            type_path: "core::felt252".to_string(),
                        })),
                    }),
                },
            ],
            generic_args: vec![],
            r#type: CompositeType::Struct,
            is_event: true,
            alias: None,
        };
        let tokenized_composite = Token::Composite(composite);

        let mut events_map = HashMap::new();
        events_map.insert(
            starknet_keccak("StoreDelRecord".as_bytes()).to_string(),
            vec![tokenized_composite],
        );

        let event = EmittedEvent {
            keys: vec![starknet_keccak("StoreDelRecord".as_bytes())],
            data: vec![
                FieldElement::from_hex_be("0x54657374").expect("Invalid hex"),
                FieldElement::from(3u128),
                FieldElement::from_hex_be("0x5465737431").expect("Invalid hex"),
                FieldElement::from_hex_be("0x5465737432").expect("Invalid hex"),
                FieldElement::from_hex_be("0x5465737433").expect("Invalid hex"),
            ],
            from_address: FieldElement::from_hex_be("0x123").expect("Invalid hex"),
            block_hash: FieldElement::from_hex_be("0x456").expect("Invalid hex"),
            block_number: 1,
            transaction_hash: FieldElement::from_hex_be("0x789").expect("Invalid hex"),
        };

        let expected_output =
            format!("Event name: dojo::world::world::StoreDelRecord\ntable: Test \"0x54657374\"\nkeys: [\"0x5465737431\", \"0x5465737432\", \"0x5465737433\"]\n");

        let actual_output = parse_event(event.clone(), &events_map)
            .unwrap_or_else(|| "Couldn't parse event".to_string());

        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_custom_event() {
        let composite = Composite {
            type_path: "dojo::world::world::CustomEvent".to_string(),
            inners: vec![
                CompositeInner {
                    index: 0,
                    name: "key_1".to_string(),
                    kind: CompositeInnerKind::Key,
                    token: Token::CoreBasic(CoreBasic {
                        type_path: "core::integer::u32".to_string(),
                    }),
                },
                CompositeInner {
                    index: 1,
                    name: "key_2".to_string(),
                    kind: CompositeInnerKind::Key,
                    token: Token::CoreBasic(CoreBasic { type_path: "core::felt252".to_string() }),
                },
                CompositeInner {
                    index: 2,
                    name: "data_1".to_string(),
                    kind: CompositeInnerKind::Data,
                    token: Token::CoreBasic(CoreBasic {
                        type_path: "core::integer::u8".to_string(),
                    }),
                },
                CompositeInner {
                    index: 3,
                    name: "data_2".to_string(),
                    kind: CompositeInnerKind::Data,
                    token: Token::CoreBasic(CoreBasic {
                        type_path: "core::integer::u8".to_string(),
                    }),
                },
            ],
            generic_args: vec![],
            r#type: CompositeType::Struct,
            is_event: true,
            alias: None,
        };
        let tokenized_composite = Token::Composite(composite);

        let mut events_map = HashMap::new();
        events_map.insert(
            starknet_keccak("CustomEvent".as_bytes()).to_string(),
            vec![tokenized_composite],
        );

        let event = EmittedEvent {
            keys: vec![
                starknet_keccak("CustomEvent".as_bytes()),
                FieldElement::from(3u128),
                FieldElement::from_hex_be("0x5465737431").expect("Invalid hex"),
            ],
            data: vec![FieldElement::from(1u128), FieldElement::from(2u128)],
            from_address: FieldElement::from_hex_be("0x123").expect("Invalid hex"),
            block_hash: FieldElement::from_hex_be("0x456").expect("Invalid hex"),
            block_number: 1,
            transaction_hash: FieldElement::from_hex_be("0x789").expect("Invalid hex"),
        };

        let expected_output =
            format!("Event name: dojo::world::world::CustomEvent\nkey_1: 3\nkey_2: Test1 \"0x5465737431\"\ndata_1: 1\ndata_2: 2\n");

        let actual_output = parse_event(event.clone(), &events_map)
            .unwrap_or_else(|| "Couldn't parse event".to_string());

        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_zero_felt() {
        let composite = Composite {
            type_path: "dojo::world::world::StoreDelRecord".to_string(),
            inners: vec![
                CompositeInner {
                    index: 0,
                    name: "table".to_string(),
                    kind: CompositeInnerKind::Data,
                    token: Token::CoreBasic(CoreBasic { type_path: "core::felt252".to_string() }),
                },
                CompositeInner {
                    index: 1,
                    name: "keys".to_string(),
                    kind: CompositeInnerKind::Data,
                    token: Token::Array(Array {
                        type_path: "core::array::Span::<core::felt252>".to_string(),
                        inner: Box::new(Token::CoreBasic(CoreBasic {
                            type_path: "core::felt252".to_string(),
                        })),
                    }),
                },
            ],
            generic_args: vec![],
            r#type: CompositeType::Struct,
            is_event: true,
            alias: None,
        };
        let tokenized_composite = Token::Composite(composite);

        let mut events_map = HashMap::new();
        events_map.insert(
            starknet_keccak("StoreDelRecord".as_bytes()).to_string(),
            vec![tokenized_composite],
        );

        let event = EmittedEvent {
            keys: vec![starknet_keccak("StoreDelRecord".as_bytes())],
            data: vec![
                FieldElement::from_hex_be("0x0").expect("Invalid hex"),
                FieldElement::from(3u128),
                FieldElement::from_hex_be("0x0").expect("Invalid hex"),
                FieldElement::from_hex_be("0x1").expect("Invalid hex"),
                FieldElement::from_hex_be("0x2").expect("Invalid hex"),
            ],
            from_address: FieldElement::from_hex_be("0x123").expect("Invalid hex"),
            block_hash: FieldElement::from_hex_be("0x456").expect("Invalid hex"),
            block_number: 1,
            transaction_hash: FieldElement::from_hex_be("0x789").expect("Invalid hex"),
        };

        let expected_output =
            format!("Event name: dojo::world::world::StoreDelRecord\ntable: \"0x0\"\nkeys: [\"0x0\", \"0x1\", \"0x2\"]\n");

        let actual_output = parse_event(event.clone(), &events_map)
            .unwrap_or_else(|| "Couldn't parse event".to_string());

        assert_eq!(actual_output, expected_output);
    }
}
