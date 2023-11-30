use benches::{estimate_gas, log, BenchCall};
use proptest::prelude::*;
use starknet::core::types::FieldElement;

proptest! {
    #[test]
    #[ignore] // needs a running katana
    fn bench_primitive_pass_many(s in "[0-9a-f]{9}") {
        let args = s.chars().map(|c| {
            let c = String::from(c);
            let hex = format!("0x{}", c);
            FieldElement::from_hex_be(&hex).unwrap()
        }).collect::<Vec<_>>();

        let fee = estimate_gas(
            BenchCall("bench_primitive_pass_many", args)
        ).unwrap();

        log("bench_primitive_pass_many", fee, &s);
    }

    #[test]
    #[ignore] // needs a running katana
    fn bench_primitive_iter(s in 990..1010) {
        let s = format!("{}", s);
        let s_hex = FieldElement::from_dec_str(&s).unwrap();

        let fee = estimate_gas(
            BenchCall("bench_primitive_iter", vec![s_hex])
        ).unwrap();

        log("bench_primitive_iter", fee, &s);
    }

    #[test]
    #[ignore] // needs a running katana
    fn bench_primitive_hash(a in 0..u64::MAX, b in 0..u64::MAX, c in 0..u64::MAX) {
        let a = format!("{}", a);
        let b = format!("{}", b);
        let c = format!("{}", c);
        let args = vec![a.clone(), b.clone(), c.clone()].into_iter().map(|d| FieldElement::from_dec_str(&d).unwrap()).collect::<Vec<_>>();

        let fee = estimate_gas(
            BenchCall("bench_primitive_hash", args)
        ).unwrap();

        log("bench_primitive_hash", fee, &format!("{},{},{}", a, b, c));
    }
}
