use benches::{estimate_gas, estimate_gas_last, log, BenchCall};
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
            .map(FieldElement::from)
            .collect();

        let fee = estimate_gas(
            BenchCall("bench_complex_set_with_smaller", points)
        ).unwrap();

        log("bench_complex_set_with_smaller", fee, &s);
    }

    #[test]
    #[ignore] // needs a running katana
    fn bench_complex_update_minimal(s in "[0-9]{9}") {
        let calldata = FieldElement::from(s.parse::<u32>().unwrap());
        let fee = estimate_gas_last(vec![
            BenchCall("bench_complex_set_default", vec![]),
            BenchCall("bench_complex_update_minimal", vec![calldata])
        ]).unwrap();

        log("bench_complex_update_minimal", fee, &s);
    }

    #[test]
    #[ignore] // needs a running katana
    fn bench_complex_update_minimal_nested(w in 0..=8) {
        let calldata = FieldElement::from(w as u32);
        let fee = estimate_gas_last(vec![
            BenchCall("bench_complex_set_default", vec![]),
            BenchCall("bench_complex_update_minimal_nested", vec![calldata])
        ]).unwrap();

        log("bench_complex_update_minimal_nested", fee, &(w as u32).to_string());
    }

    #[test]
    #[ignore] // needs a running katana
    fn bench_complex_get(s in "[0-7]{6}") {
        let calldata = s.chars()
            .map(|c| c.to_digit(10).unwrap())
            .map(FieldElement::from)
            .collect();
        let fee = estimate_gas_last(vec![
            BenchCall("bench_complex_set_with_smaller", calldata),
            BenchCall("bench_complex_get", vec![])
        ]).unwrap();

        log("bench_complex_get", fee, &s);
    }

    #[test]
    #[ignore] // needs a running katana
    fn bench_complex_get_minimal(s in "[0-9]{9}") {
        let calldata = FieldElement::from(s.parse::<u32>().unwrap());
        let fee = estimate_gas_last(vec![
            BenchCall("bench_complex_set_default", vec![]),
            BenchCall("bench_complex_update_minimal", vec![calldata]),
            BenchCall("bench_complex_get_minimal", vec![])
        ]).unwrap();

        log("bench_complex_get_minimal", fee, &s);
    }

    #[test]
    #[ignore] // needs a running katana
    fn bench_complex_check(s in "[0-7]{6}", a in 0..6, t in 0..20) {
        let abilities = s.chars()
            .map(|c| c.to_digit(10).unwrap())
            .map(FieldElement::from)
            .collect();

        let ability = FieldElement::from(a as u32);
        let threshold = FieldElement::from(t as u32);

        let fee = estimate_gas_last(vec![
            BenchCall("bench_complex_set_with_smaller", abilities),
            BenchCall("bench_complex_check", vec![ability, threshold])
        ]).unwrap();

        log("bench_complex_check", fee, &format!("{}, {}, {}", s, a, t));

    }
}
