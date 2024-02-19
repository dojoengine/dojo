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

    //print_events_map(events_map.clone());

    if let Some(events_map) = events_map {
        parse_and_print_events(res, events_map)?;
    } else {
        println!("[serde_json] {}", serde_json::to_string_pretty(&res)?);
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
        //println!("----------------------------------------------");
        //println!("[parse_and_print_events]");
        //println!("{}", serde_json::to_string_pretty(&event)?);
        if let Some(e) = parse_event(event.clone(), &events_map) {
            println!("{e}");
        } else {
            println!("Couldn't parse event - {}", serde_json::to_string_pretty(&event)?);
            println!("-----> {}", event.keys[0].to_string());
        }
    }
    Ok(())
}

fn print_events_map(events_map: Option<HashMap<String, Vec<Token>>>) {
    match events_map {
        Some(map) => {
            for (key, events) in map {
                println!("[print_events_map] Key: {}", key);
                for event in events {
                    // Using {:?} to print the Debug representation of Event
                    // Ensure that Event implements Debug trait
                    println!("[print_events_map] {:?}", event);
                    println!("");
                }
                println!("----------------------------------------------");
            }
        }
        None => println!("No events map."),
    }
}

fn parse_core_basic(
    cb: &CoreBasic,
    value: &FieldElement,
    is_nested: bool,
) -> Result<String, String> {
    match cb.type_name().as_str() {
        "felt252" => {
            if is_nested {
                Ok(format!("\"{:#x}\"", value))
            } else {
                match parse_cairo_short_string(value) {
                    Ok(parsed) => Ok(parsed),
                    Err(_) => Ok(format!("\"{:#x}\"", value)),
                }
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
    //println!("Event: {:?}", event.clone());

    let mut data = VecDeque::from(event.data.clone());
    let mut keys = VecDeque::from(event.keys.clone());
    let event_hash = keys.pop_front()?;
    //println!("Event hash: {}", event_hash.clone());

    let events = events_map.get(&event_hash.to_string())?;
    //println!("Events: {:?}", events.clone());

    'outer: for e in events {
        let mut ret = format!("Event name: {}\n", e.type_path());

        //println!("data: {:?}", data.clone());

        if let Token::Composite(composite) = e {
            //println!("Composite: {:?}", composite);

            for inner in &composite.inners {
                //println!("Inner: {:?}", inner);

                let result: Result<_, &'static str> = match inner.kind {
                    CompositeInnerKind::Data => data.pop_front().ok_or("Missing data value"),
                    CompositeInnerKind::Key => keys.pop_front().ok_or("Missing key value"),
                    _ => Err("Unsupported inner kind encountered"),
                };

                let value = match result {
                    Ok(val) => val,
                    Err(e) => {
                        println!("{}", e);
                        continue 'outer;
                    }
                };
                //println!("Value: {}", value.to_string());

                let formatted_value = match &inner.token {
                    Token::CoreBasic(ref cb) => match parse_core_basic(cb, &value, false) {
                        Ok(parsed_value) => parsed_value,
                        Err(e) => {
                            println!("Error parsing CoreBasic: {}", e);
                            continue 'outer;
                        }
                    },
                    Token::Array(ref array) => {
                        let length = match value.to_string().parse::<usize>() {
                            Ok(len) => len,
                            Err(e) => {
                                println!("Error parsing length to usize: {}", e);
                                continue 'outer;
                            }
                        };
                        //println!("Length: {}", length);

                        let cb = if let Token::CoreBasic(ref cb) = *array.inner {
                            cb
                        } else {
                            println!("Inner token of array is not CoreBasic");
                            continue 'outer;
                        };

                        let mut elements = Vec::new();
                        for _ in 0..length {
                            if let Some(element_value) = data.pop_front() {
                                //println!("Element value: {}", element_value.to_string());
                                match parse_core_basic(cb, &element_value, true) {
                                    Ok(element_str) => elements.push(element_str),
                                    Err(e) => {
                                        println!(
                                            "Error parsing CoreBasic for array element: {}",
                                            e
                                        );
                                        continue 'outer;
                                    }
                                };
                            } else {
                                println!("Missing array element value");
                                continue 'outer;
                            }
                        }

                        format!("[{}]", elements.join(", "))
                    }
                    _ => {
                        // Default case for unsupported Token types.
                        println!("Unsupported token type encountered");
                        "Unsupported token type".to_string()
                    }
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

        // Construct the expected output string
        let expected_output =
            format!("Event name: StoreDelRecord\ntable: Test\nkeys: [Test1, Test2, Test3]\n");

        // Parse the event and check the result
        let actual_output = parse_event(event.clone(), &events_map)
            .unwrap_or_else(|| "Couldn't parse event".to_string());

        // Assert that the actual output matches the expected output
        assert_eq!(actual_output, expected_output);
    }
}
