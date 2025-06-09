use dojo::storage::database::fill_with_zeroes;
use crate::utils::GasCounterTrait;

#[test]
fn bench_fill_with_zeroes() {
    let mut values = array![];

    let mut gas = GasCounterTrait::start();
    fill_with_zeroes(ref values, 1000);
    gas.end_csv("fill_with_zeroes");
}
