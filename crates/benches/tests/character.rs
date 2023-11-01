use benches::execute;
use proptest::prelude::*;
use starknet::core::types::FieldElement;

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

proptest! {
    #[test]
    #[ignore] // needs a running katana
    fn bench_update_complex_minimal(s in "[0-9]{9}") {
        let calldata = FieldElement::from(u32::from_str_radix(&s, 10).unwrap());
        let fee = execute(vec![("bench_update_complex_minimal", vec![calldata])]).unwrap();

        assert!(fee > 1);
        println!("fee: {}\tcalldata: {}", fee, s);
    }
}
