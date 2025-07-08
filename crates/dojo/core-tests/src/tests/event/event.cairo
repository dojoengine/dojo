#[dojo::event]
struct FooEvent {
    #[key]
    k1: u8,
    #[key]
    k2: felt252,
    v1: u128,
    v2: u32,
}

#[test]
fn test_event_definition() {
    let definition = dojo::event::Event::<FooEvent>::definition();

    assert_eq!(definition.name, dojo::event::Event::<FooEvent>::name());
    assert_eq!(definition.layout, dojo::event::Event::<FooEvent>::layout());
    assert_eq!(definition.schema, dojo::event::Event::<FooEvent>::schema());
}
