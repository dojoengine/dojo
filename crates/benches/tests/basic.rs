#[cfg(not(feature = "skip-gas-benchmarks"))]
mod imports {
    pub use benches::{deploy_sync, estimate_gas, estimate_gas_last, log, BenchCall};
    pub use hex::ToHex;
    pub use katana_runner::runner;
    pub use proptest::prelude::*;
    pub use starknet::core::types::FieldElement;
}

#[cfg(not(feature = "skip-gas-benchmarks"))]
use imports::*;

#[cfg(not(feature = "skip-gas-benchmarks"))]
proptest! {
    #[test]
    fn bench_basic_emit(s in "[A-Za-z0-9]{1,31}") {
        runner!(bench_basic_emit);
        let contract_address = deploy_sync(runner).unwrap();

        let s_hex = FieldElement::from_hex_be(&format!("0x{}", s.as_bytes().encode_hex::<String>())).unwrap();

        let fee = estimate_gas(
           &runner.account(1),
            BenchCall("bench_basic_emit", vec![s_hex]), contract_address
        ).unwrap();

        log("bench_basic_emit", fee, &s);
    }

    #[test]
    fn bench_basic_set(s in "[A-Za-z0-9]{1,31}") {
        runner!(bench_basic_set);
        let contract_address = deploy_sync(runner).unwrap();

        let s_hex = FieldElement::from_hex_be(&format!("0x{}", s.as_bytes().encode_hex::<String>())).unwrap();

        let fee = estimate_gas(&runner.account(1),
            BenchCall("bench_basic_set", vec![s_hex]), contract_address
        ).unwrap();

        log("bench_basic_set", fee, &s);
    }

    #[test]
    fn bench_basic_double_set(s in "[A-Za-z0-9]{1,31}") {
        runner!(bench_basic_double_set);
        let contract_address = deploy_sync(runner).unwrap();

        let s_hex = FieldElement::from_hex_be(&format!("0x{}", s.as_bytes().encode_hex::<String>())).unwrap();

        let fee = estimate_gas(&runner.account(1),
            BenchCall("bench_basic_double_set", vec![s_hex]), contract_address
        ).unwrap();

        log("bench_basic_double_set", fee, &s);
    }

    #[test]
    fn bench_basic_get(s in "[A-Za-z0-9]{1,31}") {
        runner!(bench_basic_get);
        let contract_address = deploy_sync(runner).unwrap();

        let s_hex = FieldElement::from_hex_be(&format!("0x{}", s.as_bytes().encode_hex::<String>())).unwrap();
        let fee = estimate_gas_last(&runner.account(1), vec![
            BenchCall("bench_basic_set", vec![s_hex]),
            BenchCall("bench_basic_get", vec![])
        ], contract_address).unwrap();

        log("bench_basic_get", fee, &s);
    }
}
