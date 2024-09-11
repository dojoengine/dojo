use katana_primitives::chain::ChainId;
use katana_primitives::contract::{ContractAddress, Nonce};
use katana_primitives::env::CfgEnv;
use katana_primitives::genesis::allocation::GenesisAllocation;
use katana_primitives::genesis::constant::DEFAULT_FEE_TOKEN_ADDRESS;
use katana_primitives::genesis::Genesis;
use katana_primitives::transaction::ExecutableTxWithHash;
use katana_primitives::FieldElement;
use starknet::accounts::{Account, ExecutionEncoding, SingleOwnerAccount};
use starknet::core::types::{BlockId, BlockTag, BroadcastedInvokeTransaction, Call};
use starknet::macros::{felt, selector};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Url};
use starknet::signers::{LocalWallet, SigningKey};

use super::{cfg, genesis};

#[allow(unused)]
pub fn invoke_executable_tx(
    address: ContractAddress,
    private_key: FieldElement,
    chain_id: ChainId,
    nonce: Nonce,
    max_fee: FieldElement,
    signed: bool,
) -> ExecutableTxWithHash {
    let url = "http://localhost:5050";
    let provider = JsonRpcClient::new(HttpTransport::new(Url::try_from(url).unwrap()));
    let signer = LocalWallet::from_signing_key(SigningKey::from_secret_scalar(private_key));

    let mut account = SingleOwnerAccount::new(
        provider,
        signer,
        address.into(),
        chain_id.into(),
        ExecutionEncoding::New,
    );

    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let calls = vec![Call {
        to: DEFAULT_FEE_TOKEN_ADDRESS.into(),
        selector: selector!("transfer"),
        calldata: vec![felt!("0x1"), felt!("0x99"), felt!("0x0")],
    }];

    let tx = account.execute_v1(calls).nonce(nonce).max_fee(max_fee).prepared().unwrap();

    let mut broadcasted_tx = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(tx.get_invoke_request(false))
        .unwrap();

    if !signed {
        broadcasted_tx.signature = vec![]
    }

    let tx = katana_rpc_types::transaction::BroadcastedInvokeTx(BroadcastedInvokeTransaction::V1(
        broadcasted_tx,
    ))
    .into_tx_with_chain_id(chain_id);

    ExecutableTxWithHash::new(tx.into())
}

#[rstest::fixture]
fn signed() -> bool {
    true
}

#[rstest::fixture]
pub fn executable_tx(signed: bool, genesis: &Genesis, cfg: CfgEnv) -> ExecutableTxWithHash {
    let (addr, alloc) = genesis.allocations.first_key_value().expect("should have account");

    let GenesisAllocation::Account(account) = alloc else {
        panic!("should be account");
    };

    invoke_executable_tx(
        *addr,
        account.private_key().unwrap(),
        cfg.chain_id,
        FieldElement::ZERO,
        // this is an arbitrary large fee so that it doesn't fail
        felt!("0x999999999999999"),
        signed,
    )
}

#[rstest::fixture]
pub fn executable_tx_without_max_fee(
    signed: bool,
    genesis: &Genesis,
    cfg: CfgEnv,
) -> ExecutableTxWithHash {
    let (addr, alloc) = genesis.allocations.first_key_value().expect("should have account");

    let GenesisAllocation::Account(account) = alloc else {
        panic!("should be account");
    };

    invoke_executable_tx(
        *addr,
        account.private_key().unwrap(),
        cfg.chain_id,
        FieldElement::ZERO,
        FieldElement::ZERO,
        signed,
    )
}
