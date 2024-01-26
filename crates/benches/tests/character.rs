#[cfg(not(feature = "skip-gas-benchmarks"))]
pub use benches::{
    deploy, deploy_sync, estimate_gas, estimate_gas_last, log, runner, BenchCall, FieldElement,
    CONTRACT,
};
#[cfg(not(feature = "skip-gas-benchmarks"))]
pub use proptest::prelude::*;

#[cfg(not(feature = "skip-gas-benchmarks"))]
#[katana_runner::katana_test]
async fn bench_complex_set_default() {
    let contract_address = deploy(&runner).await.unwrap();

    let fee = estimate_gas(
        &runner.account(1),
        BenchCall("bench_complex_set_default", vec![]),
        contract_address,
    )
    .unwrap();

    log("bench_complex_set_default", fee, "");
}

#[cfg(not(feature = "skip-gas-benchmarks"))]
proptest! {
    #[test]
    fn bench_complex_set_with_smaller(s in "[0-7]{6}") {
        runner!(bench_complex_set_with_smaller);
        let contract_address = deploy_sync(runner).unwrap();

        let points = s.chars()
            .map(|c| c.to_digit(10).unwrap())
            .map(FieldElement::from)
            .collect();

        let fee = estimate_gas(&runner.account(1),
            BenchCall("bench_complex_set_with_smaller", points), contract_address
        ).unwrap();

        log("bench_complex_set_with_smaller", fee, &s);
    }

    #[test]
    fn bench_complex_update_minimal(s in "[0-9]{9}") {
        runner!(bench_complex_update_minimal);
        let contract_address = deploy_sync(runner).unwrap();

        let calldata = FieldElement::from(s.parse::<u32>().unwrap());
        let fee = estimate_gas_last(&runner.account(1), vec![
            BenchCall("bench_complex_set_default", vec![]),
            BenchCall("bench_complex_update_minimal", vec![calldata])
        ], contract_address).unwrap();

        log("bench_complex_update_minimal", fee, &s);
    }

    #[test]
    fn bench_complex_update_minimal_nested(w in 0..=8) {
        runner!(bench_complex_update_minimal_nested);
        let contract_address = deploy_sync(runner).unwrap();

        let calldata = FieldElement::from(w as u32);
        let fee = estimate_gas_last(&runner.account(1), vec![
            BenchCall("bench_complex_set_default", vec![]),
            BenchCall("bench_complex_update_minimal_nested", vec![calldata])
        ], contract_address).unwrap();

        log("bench_complex_update_minimal_nested", fee, &(w as u32).to_string());
    }

    #[test]
    fn bench_complex_get(s in "[0-7]{6}") {
        runner!(bench_complex_get);
        let contract_address = deploy_sync(runner).unwrap();

        let calldata = s.chars()
            .map(|c| c.to_digit(10).unwrap())
            .map(FieldElement::from)
            .collect();
        let fee = estimate_gas_last(&runner.account(1), vec![
            BenchCall("bench_complex_set_with_smaller", calldata),
            BenchCall("bench_complex_get", vec![])
        ], contract_address).unwrap();

        log("bench_complex_get", fee, &s);
    }

    #[test]
    fn bench_complex_get_minimal(s in "[0-9]{9}") {
        runner!(bench_complex_get_minimal);
        let contract_address = deploy_sync(runner).unwrap();

        let calldata = FieldElement::from(s.parse::<u32>().unwrap());
        let fee = estimate_gas_last(&runner.account(1), vec![
            BenchCall("bench_complex_set_default", vec![]),
            BenchCall("bench_complex_update_minimal", vec![calldata]),
            BenchCall("bench_complex_get_minimal", vec![])
        ], contract_address).unwrap();

        log("bench_complex_get_minimal", fee, &s);
    }

    #[test]
    fn bench_complex_check(s in "[0-7]{6}", a in 0..6, t in 0..20) {
        runner!(bench_complex_check);
        let contract_address = deploy_sync(runner).unwrap();

        let abilities = s.chars()
            .map(|c| c.to_digit(10).unwrap())
            .map(FieldElement::from)
            .collect();

        let ability = FieldElement::from(a as u32);
        let threshold = FieldElement::from(t as u32);

        let fee = estimate_gas_last(&runner.account(1), vec![
            BenchCall("bench_complex_set_with_smaller", abilities),
            BenchCall("bench_complex_check", vec![ability, threshold])
        ], contract_address).unwrap();

        log("bench_complex_check", fee, &format!("{}, {}, {}", s, a, t));

    }
}
