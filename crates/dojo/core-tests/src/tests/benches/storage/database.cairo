use dojo::storage::database::fill_with_zeroes;

#[test]
fn bench_fill_with_zeroes() {
    let mut values = array![];
    fill_with_zeroes(ref values, 1000);
}
