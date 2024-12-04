use cairo_lang_macro::TokenStream;

use crate::attributes::constants::{DOJO_CONTRACT_ATTR, DOJO_MODEL_ATTR};
use crate::attributes::dojo_contract::{handle_module_attribute_macro, DOJO_INIT_FN};
use crate::tests::utils::assert_output_stream;

const SIMPLE_CONTRACT: &str = "
mod simple_contract {
}
";

const EXPANDED_SIMPLE_CONTRACT: &str = include_str!("./expanded/simple_contract.cairo");

const COMPLEX_CONTRACT: &str = "
mod complex_contract {
    use starknet::{ContractAddress, get_caller_address};

    #[derive(Copy, Drop, Serde)]
    #[dojo::event]
    struct MyInit {
        #[key]
        caller: ContractAddress,
        value: u8,
    }

    #[storage]
    struct Storage {
        value: u128
    }

    #[derive(Drop, starknet::Event)]
    pub struct MyEvent {
        #[key]
        pub selector: felt252,
        pub value: u64,
    }

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        MyEvent: MyEvent,
    }

    #[constructor]
    fn constructor(ref self: ContractState) {
        self.value.write(12);
    }

    fn dojo_init(self: @ContractState, value: u8) {
        let mut world = self.world(@\"ns\");
        world.emit_event(@MyInit { caller: get_caller_address(), value });
    }

    #[generate_trait]
    impl SelfImpl of SelfTrait {
        fn my_internal_function(self: @ContractState) -> u8 {
            42
        }
    }
}
";

const EXPANDED_COMPLEX_CONTRACT: &str = include_str!("./expanded/complex_contract.cairo");

#[test]
fn test_contract_is_not_a_struct() {
    let input = TokenStream::new("enum MyEnum { X, Y }".to_string());

    let res = handle_module_attribute_macro(input);

    assert!(res.diagnostics.is_empty());
    assert!(res.token_stream.is_empty());
}

#[test]
fn test_contract_has_duplicated_attributes() {
    let input = TokenStream::new(format!(
        "
        #[{DOJO_CONTRACT_ATTR}]
        {SIMPLE_CONTRACT}
        "
    ));

    let res = handle_module_attribute_macro(input);

    assert_eq!(
        res.diagnostics[0].message,
        format!("Only one {DOJO_CONTRACT_ATTR} attribute is allowed per module.")
    );
}

#[test]
fn test_contract_has_attribute_conflict() {
    let input = TokenStream::new(format!(
        "
        #[{DOJO_MODEL_ATTR}]
        {SIMPLE_CONTRACT}
        "
    ));

    let res = handle_module_attribute_macro(input);

    assert_eq!(
        res.diagnostics[0].message,
        format!("A {DOJO_CONTRACT_ATTR} can't be used together with a {DOJO_MODEL_ATTR}.")
    );
}

#[test]
fn test_contract_has_bad_init_function() {
    let input = TokenStream::new(
        "
mod simple_contract {
    fn dojo_init(self: @ContractState) -> u8 {
        0
    }
}
        "
        .to_string(),
    );

    let res = handle_module_attribute_macro(input);

    assert_eq!(
        res.diagnostics[0].message,
        format!("The {DOJO_INIT_FN} function cannot have a return type.")
    );
}

#[test]
fn test_simple_contract() {
    let input = TokenStream::new(SIMPLE_CONTRACT.to_string());

    let res = handle_module_attribute_macro(input);

    assert!(res.diagnostics.is_empty());
    assert_output_stream(&res.token_stream, EXPANDED_SIMPLE_CONTRACT);
}

#[test]
fn test_complex_contract() {
    let input = TokenStream::new(COMPLEX_CONTRACT.to_string());

    let res = handle_module_attribute_macro(input);

    assert!(res.diagnostics.is_empty());
    assert_output_stream(&res.token_stream, EXPANDED_COMPLEX_CONTRACT);
}
