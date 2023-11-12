use benches::{estimate_gas, estimate_gas_last, log, BenchCall};
use hex::ToHex;
use proptest::prelude::*;
use starknet::core::types::FieldElement;

proptest! {
    #[test]
    #[ignore] // needs a running katana
    fn bench_basic_emit(s in "[A-Za-z0-9]{1,31}") {
        let s_hex = FieldElement::from_hex_be(&format!("0x{}", s.as_bytes().encode_hex::<String>())).unwrap();

        let fee = estimate_gas(
            BenchCall("bench_basic_emit", vec![s_hex])
        ).unwrap();

        log("bench_basic_emit", fee, &s);
    }

    #[test]
    #[ignore] // needs a running katana
    fn bench_basic_set(s in "[A-Za-z0-9]{1,31}") {
        let s_hex = FieldElement::from_hex_be(&format!("0x{}", s.as_bytes().encode_hex::<String>())).unwrap();

        let fee = estimate_gas(
            BenchCall("bench_basic_set", vec![s_hex])
        ).unwrap();

        log("bench_basic_set", fee, &s);
    }

    #[test]
    #[ignore] // needs a running katana
    fn bench_basic_double_set(s in "[A-Za-z0-9]{1,31}") {
        let s_hex = FieldElement::from_hex_be(&format!("0x{}", s.as_bytes().encode_hex::<String>())).unwrap();

        let fee = estimate_gas(
            BenchCall("bench_basic_double_set", vec![s_hex])
        ).unwrap();

        log("bench_basic_double_set", fee, &s);
    }

    #[test]
    #[ignore] // needs a running katana
    fn bench_basic_get(s in "[A-Za-z0-9]{1,31}") {
        let s_hex = FieldElement::from_hex_be(&format!("0x{}", s.as_bytes().encode_hex::<String>())).unwrap();
        let fee = estimate_gas_last(vec![
            BenchCall("bench_basic_set", vec![s_hex]),
            BenchCall("bench_basic_get", vec![])
        ]).unwrap();

        log("bench_basic_get", fee, &s);
    }
}
