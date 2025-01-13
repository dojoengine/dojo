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

const NB_OF_TXS: usize = 20;
const NB_OF_RECEIPTS: usize = 20;
const NB_OF_NONCES: usize = 100;
const NB_OF_STORAGE_KEYS: usize = 100;
const NB_OF_STORAGE_VALUES: usize = 100;
const NB_OF_CLASSES: usize = 100;
const NB_OF_CONTRACTS: usize = 100;

pub fn commit(block: UncommittedBlock<'_, DbProvider>) {
    let _ = block.commit();
}

pub fn commit_parallel(block: UncommittedBlock<'_, DbProvider>) {
    let _ = block.commit_parallel();
}

#[inline(always)]
pub fn random_array(size: usize) -> Vec<u8> {
    (0..size).map(|_| rand::random::<u8>()).collect()
}

#[inline(always)]
pub fn random_felt() -> Felt {
    Felt::arbitrary(&mut Unstructured::new(&random_array(Felt::size_hint(0).0))).unwrap()
}

#[inline(always)]
pub fn random_tx() -> Tx {
    Tx::arbitrary(&mut Unstructured::new(&random_array(Tx::size_hint(0).0))).unwrap()
}

#[inline(always)]
pub fn random_tx_with_hash() -> TxWithHash {
    TxWithHash { hash: random_felt(), transaction: random_tx() }
}

#[inline(always)]
pub fn random_receipt() -> Receipt {
    Receipt::arbitrary(&mut Unstructured::new(&random_array(Receipt::size_hint(0).0))).unwrap()
}

#[inline(always)]
pub fn random_receipt_with_hash() -> ReceiptWithTxHash {
    ReceiptWithTxHash { tx_hash: random_felt(), receipt: random_receipt() }
}

#[inline(always)]
pub fn random_felt_to_felt_map(size: usize) -> BTreeMap<Felt, Felt> {
    (0..size).map(|_| (random_felt(), random_felt())).collect()
}

#[inline(always)]
pub fn random_address_to_felt_map(size: usize) -> BTreeMap<ContractAddress, Felt> {
    (0..size).map(|_| (ContractAddress::new(random_felt()), random_felt())).collect()
}

pub fn commit_benchmark(c: &mut Criterion) {
    let provider = DbProvider::new_ephemeral();

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

    let transactions: Vec<TxWithHash> = (0..NB_OF_TXS).map(|_| random_tx_with_hash()).collect();
    let receipts: Vec<ReceiptWithTxHash> =
        (0..NB_OF_RECEIPTS).map(|_| random_receipt_with_hash()).collect();

    let nonce_updates: BTreeMap<ContractAddress, Felt> =
        (0..NB_OF_NONCES).map(|_| (ContractAddress::new(random_felt()), random_felt())).collect();

    let storage_updates: BTreeMap<ContractAddress, BTreeMap<Felt, Felt>> = (0..NB_OF_STORAGE_KEYS)
        .map(|_| {
            (ContractAddress::new(random_felt()), random_felt_to_felt_map(NB_OF_STORAGE_VALUES))
        })
        .collect();

    let declared_classes: BTreeMap<Felt, Felt> = random_felt_to_felt_map(NB_OF_CLASSES);
    let deployed_contracts: BTreeMap<ContractAddress, Felt> =
        random_address_to_felt_map(NB_OF_CONTRACTS);

    let state_updates = StateUpdates {
        nonce_updates,
        storage_updates,
        declared_classes,
        deployed_contracts,
        ..Default::default()
    };

    let block =
        UncommittedBlock::new(header, transactions, receipts.as_slice(), &state_updates, provider);

    c.bench_function("commit", |b| {
        b.iter_batched(|| block.clone(), |input| commit(black_box(input)), BatchSize::SmallInput);
    });

    c.bench_function("commit_parallel", |b| {
        b.iter_batched(
            || block.clone(),
            |input| commit_parallel(black_box(input)),
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, commit_benchmark);
criterion_main!(benches);
