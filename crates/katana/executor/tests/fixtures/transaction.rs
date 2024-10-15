use katana_primitives::chain::ChainId;
use katana_primitives::chain_spec::ChainSpec;
use katana_primitives::contract::{ContractAddress, Nonce};
use katana_primitives::env::CfgEnv;
use katana_primitives::genesis::allocation::GenesisAllocation;
use katana_primitives::genesis::constant::DEFAULT_ETH_FEE_TOKEN_ADDRESS;
use katana_primitives::transaction::ExecutableTxWithHash;
use katana_primitives::utils::transaction::compute_invoke_v1_tx_hash;
use katana_primitives::Felt;
use num_traits::ToPrimitive;
use starknet::accounts::{Account, ExecutionEncoder, ExecutionEncoding, SingleOwnerAccount};
use starknet::core::types::{
    BlockId, BlockTag, BroadcastedInvokeTransaction, BroadcastedInvokeTransactionV1, Call,
};
use starknet::macros::{felt, selector};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Url};
use starknet::signers::{LocalWallet, Signer, SigningKey};

use super::{cfg, chain};

#[allow(unused)]
pub fn invoke_executable_tx(
    address: ContractAddress,
    private_key: Felt,
    chain_id: ChainId,
    nonce: Nonce,
    max_fee: Felt,
    signed: bool,
) -> ExecutableTxWithHash {
    let url = "http://localhost:5050";
    let provider = JsonRpcClient::new(HttpTransport::new(Url::try_from(url).unwrap()));
    let signer = LocalWallet::from_signing_key(SigningKey::from_secret_scalar(private_key));

    let mut account = SingleOwnerAccount::new(
        &provider,
        &signer,
        address.into(),
        chain_id.into(),
        ExecutionEncoding::New,
    );

    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let calls = vec![Call {
        to: DEFAULT_ETH_FEE_TOKEN_ADDRESS.into(),
        selector: selector!("transfer"),
        calldata: vec![felt!("0x1"), felt!("0x99"), felt!("0x0")],
    }];

    let calldata = account.encode_calls(&calls);
    let hash = compute_invoke_v1_tx_hash(
        account.address(),
        &calldata,
        max_fee.to_u128().unwrap(),
        chain_id.into(),
        nonce,
        false,
    );

    let signature = if signed {
        let signature = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(signer.sign_hash(&hash))
            .unwrap();

        vec![signature.r, signature.s]
    } else {
        vec![]
    };

    let mut starknet_rs_broadcasted_tx = BroadcastedInvokeTransactionV1 {
        nonce,
        max_fee,
        calldata,
        signature,
        is_query: false,
        sender_address: account.address(),
    };

    if !signed {
        starknet_rs_broadcasted_tx.signature = vec![]
    }

    let tx = katana_rpc_types::transaction::BroadcastedInvokeTx(BroadcastedInvokeTransaction::V1(
        starknet_rs_broadcasted_tx,
    ))
    .into_tx_with_chain_id(chain_id);

    ExecutableTxWithHash::new(tx.into())
}

#[rstest::fixture]
fn signed() -> bool {
    true
}

#[rstest::fixture]
pub fn executable_tx(signed: bool, chain: &ChainSpec, cfg: CfgEnv) -> ExecutableTxWithHash {
    let (addr, alloc) = chain.genesis.allocations.first_key_value().expect("should have account");

    let GenesisAllocation::Account(account) = alloc else {
        panic!("should be account");
    };

    invoke_executable_tx(
        *addr,
        account.private_key().unwrap(),
        cfg.chain_id,
        Felt::ZERO,
        // this is an arbitrary large fee so that it doesn't fail
        felt!("0x999999999999999"),
        signed,
    )
}

#[rstest::fixture]
pub fn executable_tx_without_max_fee(
    signed: bool,
    chain: &ChainSpec,
    cfg: CfgEnv,
) -> ExecutableTxWithHash {
    let (addr, alloc) = chain.genesis.allocations.first_key_value().expect("should have account");

    let GenesisAllocation::Account(account) = alloc else {
        panic!("should be account");
    };

    invoke_executable_tx(
        *addr,
        account.private_key().unwrap(),
        cfg.chain_id,
        Felt::ZERO,
        Felt::ZERO,
        signed,
    )
}
