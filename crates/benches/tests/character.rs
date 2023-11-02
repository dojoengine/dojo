use benches::{estimate_gas, estimate_gas_last, estimate_gas_multiple, log, BenchCall};
use proptest::prelude::*;
use starknet::core::types::FieldElement;

#[test]
#[ignore] // needs a running katana
fn bench_complex_set_default() {
    let fee = estimate_gas(BenchCall("bench_complex_set_default", vec![])).unwrap();

    log("bench_complex_set_default", fee, "");
}

proptest! {
    #[test]
    #[ignore] // needs a running katana
    fn bench_complex_set_with_smaller(s in "[0-7]{6}") {
        let points = s.chars()
            .map(|c| c.to_digit(10).unwrap())
            .map(|p| FieldElement::from(p))
            .collect();

        let fee = estimate_gas(
            BenchCall("bench_complex_set_with_smaller", points)
        ).unwrap();

        log("bench_complex_set_with_smaller", fee, &s);
    }

    #[test]
    #[ignore] // needs a running katana
    fn bench_complex_update_minimal(s in "[0-9]{9}") {
        let calldata = FieldElement::from(u32::from_str_radix(&s, 10).unwrap());
        let fee = estimate_gas_last(vec![
            BenchCall("bench_complex_set_default", vec![]),
            BenchCall("bench_complex_update_minimal", vec![calldata])
        ]).unwrap();

        log("bench_complex_update_minimal", fee, &s);
    }

    #[test]
    #[ignore] // needs a running katana
    fn bench_complex_get(s in "[0-9]{9}") {
        let calldata = FieldElement::from(u32::from_str_radix(&s, 10).unwrap());
        let fee = estimate_gas_last(vec![
            BenchCall("bench_complex_set_with_smaller", vec![calldata]),
            BenchCall("bench_complex_get", vec![calldata])
        ]).unwrap();

        log("bench_complex_get", fee, &s);
    }
}
