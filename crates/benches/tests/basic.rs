use benches::execute;
use hex::ToHex;
use proptest::prelude::*;
use starknet::core::types::FieldElement;
use tracing::info;

proptest! {
    #[test]
    #[ignore] // needs a running katana
    fn bench_emit(s in "[A-Za-z0-9]{1,31}") {
        let s_hex = FieldElement::from_hex_be(&format!("0x{}", s.as_bytes().encode_hex::<String>())).unwrap();

        let fee = execute(vec![("bench_emit", vec![s_hex])]).unwrap();

        info!(target: "bench_emit", "fee: {}\tcalldata: {}", fee, s);
    }

    #[test]
    #[ignore] // needs a running katana
    fn bench_set(s in "[A-Za-z0-9]{1,31}") {
        let s_hex = FieldElement::from_hex_be(&format!("0x{}", s.as_bytes().encode_hex::<String>())).unwrap();

        let fee = execute(vec![("bench_set", vec![s_hex])]).unwrap();

        info!(target: "bench_set", "fee: {}\tcalldata: {}", fee, s);
    }

    #[test]
    #[ignore] // needs a running katana
    fn bench_get(s in "[A-Za-z0-9]{1,31}") {
        let s_hex = FieldElement::from_hex_be(&format!("0x{}", s.as_bytes().encode_hex::<String>())).unwrap();
        let calls = vec![("bench_set", vec![s_hex]), ("bench_get", vec![])];

        let fee = execute(calls).unwrap();

        info!(target: "bench_get", "fee: {}\tcalldata: {}", fee, s);
    }
}
