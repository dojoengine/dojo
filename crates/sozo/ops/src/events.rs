use std::collections::{HashMap, VecDeque};

use anyhow::{anyhow, Result};
use cainome::cairo_serde::{ByteArray, CairoSerde};
use cainome::parser::tokens::{CompositeInner, CompositeInnerKind, CoreBasic, Token};
use cainome::parser::AbiParser;
use dojo_world::diff::WorldDiff;
use starknet::core::types::contract::AbiEntry;
use starknet::core::types::{BlockId, EventFilter, Felt};
use starknet::core::utils::{parse_cairo_short_string, starknet_keccak};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};

/// Returns an event filter for the world with the given parameters.
pub fn get_event_filter(
    from_block: Option<u64>,
    to_block: Option<u64>,
    events: Option<Vec<String>>,
    world_address: Option<Felt>,
) -> EventFilter {
    let from_block = from_block.map(BlockId::Number);
    let to_block = to_block.map(BlockId::Number);
    // Currently dojo doesn't use custom keys for events. In future if custom keys are used this
    // needs to be updated for granular queries.
    let keys =
        events.map(|e| vec![e.iter().map(|event| starknet_keccak(event.as_bytes())).collect()]);

    EventFilter { from_block, to_block, address: world_address, keys }
}

/// Parses and prints events of the world.
pub async fn parse(
    world_diff: &WorldDiff,
    provider: &JsonRpcClient<HttpTransport>,
    chunk_size: u64,
    continuation_token: Option<String>,
    event_filter: EventFilter,
) -> Result<()> {
    let events_map = extract_events(world_diff)?;
    let res = provider.get_events(event_filter, continuation_token, chunk_size).await?;

    parse_and_print_events(res, events_map)?;

    Ok(())
}

fn is_event(token: &Token) -> bool {
    match token {
        Token::Composite(composite) => composite.is_event,
        _ => false,
    }
}

fn extract_events(world_diff: &WorldDiff) -> Result<HashMap<String, Vec<Token>>> {
    fn process_abi(events: &mut HashMap<String, Vec<Token>>, abi: &Vec<AbiEntry>) -> Result<()> {
        match AbiParser::collect_tokens(abi, &HashMap::new()) {
            Ok(tokens) => {
                for token in tokens.structs {
                    if is_event(&token) {
                        let event_name = starknet_keccak(token.type_name().as_bytes());
                        let vec = events.entry(event_name.to_string()).or_default();
                        vec.push(token.clone());
                    }
                }
            }
            Err(e) => return Err(anyhow!("Error parsing events from ABI: {}", e)),
        }

        Ok(())
    }

    let mut events_map = HashMap::new();

    process_abi(&mut events_map, &world_diff.world_info.class.abi)?;

    for r in world_diff.resources.values() {
        process_abi(&mut events_map, &r.abi())?;
    }

    Ok(events_map)
}

fn parse_and_print_events(
    res: starknet::core::types::EventsPage,
    events_map: HashMap<String, Vec<Token>>,
) -> Result<()> {
    println!("Continuation token: {:?}", res.continuation_token);
    println!("----------------------------------------------");

    for event in res.events {
        let parsed_event = parse_event(event.clone(), &events_map);

        match parsed_event {
            Ok(parsed_event) => {
                println!("{parsed_event}");
            }
            Err(e) => {
                println!("{}", e);
                println!("Event: {}\n", serde_json::to_string_pretty(&event).unwrap());
            }
        }
    }
    Ok(())
}

fn parse_core_basic(cb: &CoreBasic, value: &Felt, include_felt_string: bool) -> Result<String> {
    match cb.type_name().as_str() {
        "felt252" => {
            let hex = format!("{:#x}", value);
            match parse_cairo_short_string(value) {
                Ok(parsed) if !parsed.is_empty() && (include_felt_string && parsed.is_ascii()) => {
                    Ok(format!("{} \"{}\"", hex, parsed))
                }
                _ => Ok(hex.to_string()),
            }
        }
        "bool" => {
            if *value == Felt::ZERO {
                Ok("false".to_string())
            } else {
                Ok("true".to_string())
            }
        }
        "ClassHash" | "ContractAddress" => Ok(format!("{:#x}", value)),
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64" | "i128" => {
            Ok(value.to_string())
        }
        _ => Err(anyhow!("Unsupported CoreBasic type: {}", cb.type_name())),
    }
}

fn parse_event(
    event: starknet::core::types::EmittedEvent,
    events_map: &HashMap<String, Vec<Token>>,
) -> Result<String> {
    let mut data = VecDeque::from(event.data.clone());
    let mut keys = VecDeque::from(event.keys.clone());
    let event_hash = keys.pop_front().ok_or(anyhow!("Event hash missing")).unwrap();

    let events = events_map
        .get(&event_hash.to_string())
        .ok_or(anyhow!("Events for hash not found: {:#x}", event_hash))?;

    for e in events {
        if let Token::Composite(composite) = e {
            let processed_inners = process_inners(&composite.inners, &mut data, &mut keys)?;
            let ret = format!("Event name: {}\n{}", e.type_path(), processed_inners);
            return Ok(ret);
        }
    }

    Err(anyhow!("No matching event found in tokens {:?}", event))
}

fn process_inners(
    inners: &[CompositeInner],
    data: &mut VecDeque<Felt>,
    keys: &mut VecDeque<Felt>,
) -> Result<String> {
    let mut ret = String::new();

    for inner in inners {
        let value = match inner.kind {
            CompositeInnerKind::Data => data.pop_front().ok_or(anyhow!("Missing data value")),
            CompositeInnerKind::Key => keys.pop_front().ok_or(anyhow!("Missing key value")),
            _ => Err(anyhow!("Unsupported inner kind encountered")),
        }?;

        let formatted_value = match &inner.token {
            Token::CoreBasic(ref cb) => parse_core_basic(cb, &value, true)?,
            Token::Composite(c) => {
                if c.type_path.eq("core::byte_array::ByteArray") {
                    data.push_front(value);
                    data.make_contiguous();
                    let bytearray = ByteArray::cairo_deserialize(data.as_mut_slices().0, 0)?;
                    data.drain(0..ByteArray::cairo_serialized_size(&bytearray));
                    ByteArray::to_string(&bytearray)?
                } else {
                    return Err(anyhow!("Unhandled Composite token"));
                }
            }
            Token::Array(ref array) => {
                let length = value
                    .to_string()
                    .parse::<usize>()
                    .map_err(|_| anyhow!("Error parsing length to usize"))?;

                let cb = if let Token::CoreBasic(ref cb) = *array.inner {
                    cb
                } else {
                    return Err(anyhow!("Inner token of array is not CoreBasic"));
                };

                let mut elements = Vec::new();
                for _ in 0..length {
                    if let Some(element_value) = data.pop_front() {
                        let element_str = parse_core_basic(cb, &element_value, false)?;
                        elements.push(element_str);
                    } else {
                        return Err(anyhow!("Missing array element value"));
                    }
                }

                format!("[{}]", elements.join(", "))
            }
            _ => return Err(anyhow!("Unsupported token type encountered")),
        };
        ret.push_str(&format!("{}: {}\n", inner.name, formatted_value));
    }

    Ok(ret)
}

#[cfg(test)]
mod tests {
    use cainome::parser::tokens::{Array, Composite, CompositeInner, CompositeType};
    use dojo_test_utils::compiler::CompilerTestSetup;
    use scarb::compiler::Profile;
    use sozo_scarbext::WorkspaceExt;
    use starknet::core::types::EmittedEvent;

    use super::*;

    const WORLD_QUALIFIED_PATH: &str = "dojo::world::world_contract::world";

    #[test]
    fn extract_events_work_as_expected() {
        let setup = CompilerTestSetup::from_examples("../../dojo/core", "../../../examples/");
        let config = setup.build_test_config("spawn-and-move", Profile::DEV);

        let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

        let world_local = ws.load_world_local().unwrap();
        let world_diff = WorldDiff::from_local(world_local).unwrap();

        let result = extract_events(&world_diff).unwrap();

        // we are just collecting all events from manifest file so just verifying count should work
        assert_eq!(result.len(), 20);
    }

    #[test]
    fn test_core_basic() {
        let composite = Composite {
            type_path: format!("{WORLD_QUALIFIED_PATH}::TestEvent"),
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
                Felt::from_hex("0x5465737431").unwrap(),
                Felt::from(1u8), // bool true
                Felt::from(1u8),
                Felt::from(2u16),
                Felt::from(3u32),
                Felt::from(4u64),
                Felt::from(5u128),
                Felt::from(6usize),
                Felt::from_hex("0x54657374").unwrap(),
                Felt::from_hex("0x54657374").unwrap(),
            ],
            from_address: Felt::from_hex("0x123").unwrap(),
            block_hash: Felt::from_hex("0x456").ok(),
            block_number: Some(1),
            transaction_hash: Felt::from_hex("0x789").unwrap(),
        };

        let expected_output = format!(
            "Event name: {WORLD_QUALIFIED_PATH}::TestEvent\nfelt252: 0x5465737431 \
             \"Test1\"\nbool: true\nu8: 1\nu16: 2\nu32: 3\nu64: 4\nu128: 5\nusize: 6\nclass_hash: \
             0x54657374\ncontract_address: 0x54657374\n"
        );

        let actual_output = parse_event(event, &events_map).expect("Failed to parse event");
        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_array() {
        let composite = Composite {
            type_path: format!("{WORLD_QUALIFIED_PATH}::StoreDelRecord"),
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
                        is_legacy: false,
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
                Felt::from_hex("0x54657374").unwrap(),
                Felt::from(3u128),
                Felt::from_hex("0x5465737431").unwrap(),
                Felt::from_hex("0x5465737432").unwrap(),
                Felt::from_hex("0x5465737433").unwrap(),
            ],
            from_address: Felt::from_hex("0x123").unwrap(),
            block_hash: Felt::from_hex("0x456").ok(),
            block_number: Some(1),
            transaction_hash: Felt::from_hex("0x789").unwrap(),
        };

        let expected_output = format!(
            "Event name: {WORLD_QUALIFIED_PATH}::StoreDelRecord\ntable: 0x54657374 \
             \"Test\"\nkeys: [0x5465737431, 0x5465737432, 0x5465737433]\n"
        );

        let actual_output = parse_event(event, &events_map).expect("Failed to parse event");
        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_custom_event() {
        let composite = Composite {
            type_path: format!("{WORLD_QUALIFIED_PATH}::CustomEvent"),
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
                Felt::from(3u128),
                Felt::from_hex("0x5465737431").unwrap(),
            ],
            data: vec![Felt::from(1u128), Felt::from(2u128)],
            from_address: Felt::from_hex("0x123").unwrap(),
            block_hash: Felt::from_hex("0x456").ok(),
            block_number: Some(1),
            transaction_hash: Felt::from_hex("0x789").unwrap(),
        };

        let expected_output = format!(
            "Event name: {WORLD_QUALIFIED_PATH}::CustomEvent\nkey_1: 3\nkey_2: 0x5465737431 \
             \"Test1\"\ndata_1: 1\ndata_2: 2\n"
        );

        let actual_output = parse_event(event, &events_map).expect("Failed to parse event");
        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_zero_felt() {
        let composite = Composite {
            type_path: format!("{WORLD_QUALIFIED_PATH}::StoreDelRecord"),
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
                        is_legacy: false,
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
                Felt::from_hex("0x0").unwrap(),
                Felt::from(3u128),
                Felt::from_hex("0x0").unwrap(),
                Felt::from_hex("0x1").unwrap(),
                Felt::from_hex("0x2").unwrap(),
            ],
            from_address: Felt::from_hex("0x123").unwrap(),
            block_hash: Felt::from_hex("0x456").ok(),
            block_number: Some(1),
            transaction_hash: Felt::from_hex("0x789").unwrap(),
        };

        let expected_output = format!(
            "Event name: {WORLD_QUALIFIED_PATH}::StoreDelRecord\ntable: 0x0\nkeys: [0x0, 0x1, \
             0x2]\n"
        );

        let actual_output = parse_event(event, &events_map).expect("Failed to parse event");
        assert_eq!(actual_output, expected_output);
    }
}
