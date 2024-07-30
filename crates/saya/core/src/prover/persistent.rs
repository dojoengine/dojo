use starknet_crypto::FieldElement;

pub struct BatcherOutput {
    prev_state_root: FieldElement,
    new_state_root: FieldElement,
    block_number: FieldElement,
    block_hash: FieldElement,
    config_hash: FieldElement,
    message_to_starknet_segment: Vec<FieldElement>,
    message_to_appchain_segment: Vec<FieldElement>,
}

#[test]
fn test_batcher_args() {
    // vec![
    //     101,
    //     3313789320606820252395445984521954593484836861909228871431543166644962841670,
    //     103,
    //     1032,
    //     104,
    //     922512285569582564187044518642857801925844887360711375174535902473923967426,
    // ]
}
