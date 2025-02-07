use std::collections::BTreeMap;

use arbitrary::{Arbitrary, Unstructured};
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use katana_core::backend::UncommittedBlock;
use katana_primitives::block::{GasPrices, PartialHeader};
use katana_primitives::da::L1DataAvailabilityMode;
use katana_primitives::receipt::{Receipt, ReceiptWithTxHash};
use katana_primitives::state::StateUpdates;
use katana_primitives::transaction::{Tx, TxWithHash};
use katana_primitives::version::CURRENT_STARKNET_VERSION;
use katana_primitives::{ContractAddress, Felt};
use katana_provider::providers::db::DbProvider;
use pprof::criterion::{Output, PProfProfiler};

struct BlockConfig {
    nb_of_txs: usize,
    nb_of_receipts: usize,
    nb_of_nonces: usize,
    nb_of_storage_keys: usize,
    nb_of_storage_values: usize,
    nb_of_classes: usize,
    nb_of_contracts: usize,
}

const SMALL_BLOCK_CONFIG: BlockConfig = BlockConfig {
    nb_of_txs: 1,
    nb_of_receipts: 1,
    nb_of_nonces: 1,
    nb_of_storage_keys: 1,
    nb_of_storage_values: 1,
    nb_of_classes: 1,
    nb_of_contracts: 1,
};

const BIG_BLOCK_CONFIG: BlockConfig = BlockConfig {
    nb_of_txs: 20,
    nb_of_receipts: 20,
    nb_of_nonces: 100,
    nb_of_storage_keys: 100,
    nb_of_storage_values: 100,
    nb_of_classes: 100,
    nb_of_contracts: 100,
};

fn commit(block: UncommittedBlock<'_, DbProvider>) {
    let _ = block.commit();
}

fn commit_parallel(block: UncommittedBlock<'_, DbProvider>) {
    let _ = block.commit_parallel();
}

#[inline(always)]
fn random_array(size: usize) -> Vec<u8> {
    (0..size).map(|_| rand::random::<u8>()).collect()
}

#[inline(always)]
fn random_felt() -> Felt {
    Felt::arbitrary(&mut Unstructured::new(&random_array(Felt::size_hint(0).0))).unwrap()
}

#[inline(always)]
fn random_tx() -> Tx {
    Tx::arbitrary(&mut Unstructured::new(&random_array(Tx::size_hint(0).0))).unwrap()
}

#[inline(always)]
fn random_tx_with_hash() -> TxWithHash {
    TxWithHash { hash: random_felt(), transaction: random_tx() }
}

#[inline(always)]
fn random_receipt() -> Receipt {
    Receipt::arbitrary(&mut Unstructured::new(&random_array(Receipt::size_hint(0).0))).unwrap()
}

#[inline(always)]
fn random_receipt_with_hash() -> ReceiptWithTxHash {
    ReceiptWithTxHash { tx_hash: random_felt(), receipt: random_receipt() }
}

#[inline(always)]
fn random_felt_to_felt_map(size: usize) -> BTreeMap<Felt, Felt> {
    (0..size).map(|_| (random_felt(), random_felt())).collect()
}

#[inline(always)]
fn random_address_to_felt_map(size: usize) -> BTreeMap<ContractAddress, Felt> {
    (0..size).map(|_| (ContractAddress::new(random_felt()), random_felt())).collect()
}

fn build_block(config: BlockConfig) -> (Vec<TxWithHash>, Vec<ReceiptWithTxHash>, StateUpdates) {
    let transactions: Vec<TxWithHash> =
        (0..config.nb_of_txs).map(|_| random_tx_with_hash()).collect();

    let receipts: Vec<ReceiptWithTxHash> =
        (0..config.nb_of_receipts).map(|_| random_receipt_with_hash()).collect();

    let nonce_updates: BTreeMap<ContractAddress, Felt> = (0..config.nb_of_nonces)
        .map(|_| (ContractAddress::new(random_felt()), random_felt()))
        .collect();

    let storage_updates: BTreeMap<ContractAddress, BTreeMap<Felt, Felt>> = (0..config
        .nb_of_storage_keys)
        .map(|_| {
            (
                ContractAddress::new(random_felt()),
                random_felt_to_felt_map(config.nb_of_storage_values),
            )
        })
        .collect();

    let declared_classes: BTreeMap<Felt, Felt> = random_felt_to_felt_map(config.nb_of_classes);
    let deployed_contracts: BTreeMap<ContractAddress, Felt> =
        random_address_to_felt_map(config.nb_of_contracts);

    let state_updates = StateUpdates {
        nonce_updates,
        storage_updates,
        declared_classes,
        deployed_contracts,
        ..Default::default()
    };

    (transactions, receipts, state_updates)
}

fn commit_benchmark(c: &mut Criterion) {
    let gas_prices = GasPrices { eth: 100 * u128::pow(10, 9), strk: 100 * u128::pow(10, 9) };
    let sequencer_address = ContractAddress(1u64.into());

    let header = PartialHeader {
        protocol_version: CURRENT_STARKNET_VERSION,
        number: 1,
        timestamp: 100,
        sequencer_address,
        parent_hash: 123u64.into(),
        l1_gas_prices: gas_prices.clone(),
        l1_data_gas_prices: gas_prices.clone(),
        l1_da_mode: L1DataAvailabilityMode::Calldata,
    };

    let (small_transactions, small_receipts, small_state_updates) = build_block(SMALL_BLOCK_CONFIG);

    let small_block = UncommittedBlock::new(
        header.clone(),
        small_transactions,
        small_receipts.as_slice(),
        &small_state_updates,
        DbProvider::new_ephemeral(),
    );

    let (big_transactions, big_receipts, big_state_updates) = build_block(BIG_BLOCK_CONFIG);
    let big_block = UncommittedBlock::new(
        header,
        big_transactions,
        big_receipts.as_slice(),
        &big_state_updates,
        DbProvider::new_ephemeral(),
    );

    c.bench_function("Commit.Small.Serial", |b| {
        b.iter_batched(
            || small_block.clone(),
            |input| commit(black_box(input)),
            BatchSize::SmallInput,
        );
    });

    c.bench_function("Commit.Small.Parallel", |b| {
        b.iter_batched(
            || small_block.clone(),
            |input| commit_parallel(black_box(input)),
            BatchSize::SmallInput,
        );
    });

    c.bench_function("Commit.Big.Serial", |b| {
        b.iter_batched(
            || big_block.clone(),
            |input| commit(black_box(input)),
            BatchSize::SmallInput,
        );
    });

    c.bench_function("Commit.Big.Parallel", |b| {
        b.iter_batched(
            || big_block.clone(),
            |input| commit_parallel(black_box(input)),
            BatchSize::SmallInput,
        );
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets = commit_benchmark
}
criterion_main!(benches);
