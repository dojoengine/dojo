use benches::execute;
use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use starknet::core::types::FieldElement;

#[derive(Serialize, Deserialize)]
struct Abilities {
    strength: u8,
    dexterity: u8,
    constitution: u8,
    intelligence: u8,
    wisdom: u8,
    charisma: u8,
}

#[test]
#[ignore] // needs a running katana
fn bench_set_complex_default() {
    let fee = execute(vec![("bench_set_complex_default", vec![])]).unwrap();

    assert!(fee > 1);
    println!("fee: {}", fee);
}

proptest! {
    #[test]
    #[ignore] // needs a running katana
    fn bench_set_complex_with_smaller(s in "[0-7]{6}") {
        let points = s.chars()
            .map(|c| c.to_digit(10).unwrap())
            .map(|p| FieldElement::from(p))
            .collect();

        let fee = execute(vec![("bench_set_complex_with_smaller", points)]).unwrap();

        assert!(fee > 1);
        println!("fee: {}\tcalldata: {}", fee, s);
    }
}
