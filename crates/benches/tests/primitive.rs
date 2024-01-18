use benches::{estimate_gas, log, BenchCall};
use katana_runner::runner;
use proptest::prelude::*;
use starknet::core::types::FieldElement;

proptest! {
    #[test]
    fn bench_primitive_pass_many(s in "[0-9a-f]{9}") {
        runner!(bench_primitive_pass_many);

        let args = s.chars().map(|c| {
            let c = String::from(c);
            let hex = format!("0x{}", c);
            FieldElement::from_hex_be(&hex).unwrap()
        }).collect::<Vec<_>>();

        let fee = estimate_gas(&runner.account(0),
            BenchCall("bench_primitive_pass_many", args), contract_address
        ).unwrap();

        log("bench_primitive_pass_many", fee, &s);
    }

    #[test]
    fn bench_primitive_iter(s in 990..1010) {
        runner!(bench_primitive_iter);

        let s = format!("{}", s);
        let s_hex = FieldElement::from_dec_str(&s).unwrap();

        let fee = estimate_gas(&runner.account(0),
            BenchCall("bench_primitive_iter", vec![s_hex]), contract_address
        ).unwrap();

        log("bench_primitive_iter", fee, &s);
    }

    #[test]
    fn bench_primitive_hash(a in 0..u64::MAX, b in 0..u64::MAX, c in 0..u64::MAX) {
        runner!(bench_primitive_hash);

        let a = format!("{}", a);
        let b = format!("{}", b);
        let c = format!("{}", c);
        let args = vec![a.clone(), b.clone(), c.clone()].into_iter().map(|d| FieldElement::from_dec_str(&d).unwrap()).collect::<Vec<_>>();

        let fee = estimate_gas(&runner.account(0),
            BenchCall("bench_primitive_hash", args), contract_address
        ).unwrap();

        log("bench_primitive_hash", fee, &format!("{},{},{}", a, b, c));
    }
}
