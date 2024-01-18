use starknet::accounts::Account;
use starknet::core::types::FieldElement;

use benches::{parse_calls, BenchCall, ENOUGH_GAS};

#[katana_runner::katana_test(2, true)]
async fn katana_heavy_single() {
    let n = (1009u64 * 1009u64).to_string();
    let calldata = parse_calls(
        vec![BenchCall("is_prime", vec![FieldElement::from_dec_str(&n).unwrap()])],
        &contract_address,
    );
    let tx = runner
        .account(1)
        .execute(calldata)
        .max_fee(FieldElement::from_hex_be(ENOUGH_GAS).unwrap())
        .nonce(FieldElement::ONE)
        .send()
        .await
        .unwrap();
    dbg!(tx);

    runner.blocks_until_empty().await;
}
