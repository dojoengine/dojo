use dojo::model::Model;
use dojo::utils::{bytearray_hash, selector_from_names};

use crate::tests::helpers::DOJO_NSH;

#[derive(Drop, Copy, Serde)]
#[dojo::model]
struct MyModel {
    #[key]
    x: u8,
    y: u8
}

#[test]
fn test_selector_computation() {
    let namespace = "dojo";
    let name = Model::<MyModel>::name();
    let selector = selector_from_names(@namespace, @name);
    assert(selector == Model::<MyModel>::selector(DOJO_NSH), 'invalid computed selector');
}
