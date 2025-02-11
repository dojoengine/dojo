use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use cainome::cairo_serde;
use dojo_utils::{TransactionWaiter, TransactionWaitingError};
use katana_primitives::block::BlockHash;
use katana_primitives::class::{
    CompiledClassHash, ComputeClassHashError, ContractClass, ContractClassCompilationError,
    ContractClassFromStrError,
};
use katana_primitives::{felt, ContractAddress, Felt};
use katana_rpc_types::class::RpcContractClass;
use piltover::{AppchainContract, AppchainContractReader, ProgramInfo};
use spinoff::{spinners, Color, Spinner};
use starknet::accounts::{Account, AccountError, ConnectedAccount, SingleOwnerAccount};
use starknet::contract::ContractFactory;
use starknet::core::crypto::compute_hash_on_elements;
use starknet::core::types::{BlockId, BlockTag, FlattenedSierraClass, StarknetError};
use starknet::macros::short_string;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider, ProviderError};
use starknet::signers::LocalWallet;
use thiserror::Error;
use tracing::trace;

type RpcProvider = Arc<JsonRpcClient<HttpTransport>>;
type InitializerAccount = SingleOwnerAccount<RpcProvider, LocalWallet>;

/// The StarknetOS program (SNOS) is the cairo program that executes the state
/// transition of a new Katana block from the previous block.
/// This program hash is required to be known by the settlement contract in order to
/// only accept a new state from a valid SNOS program.
///
/// This program can be found here: <https://github.com/starkware-libs/cairo-lang/blob/a86e92bfde9c171c0856d7b46580c66e004922f3/src/starkware/starknet/core/os/os.cairo>.
const SNOS_PROGRAM_HASH: Felt =
    felt!("0x054d3603ed14fb897d0925c48f26330ea9950bd4ca95746dad4f7f09febffe0d");

/// To execute the SNOS program, a specific layout named "all_cairo" is required.
/// However, this layout can't be verified by the Cairo verifier that lives on Starknet.
///
/// This is why we're using an other program, the Layout Bridge program, which act as a verifier
/// written in Cairo which uses a layout supported by the Cairo verifier.
///
/// By verifying a SNOS proof using the Layout Bridge program, a new proof is generated which can be
/// verified by the Cairo verifier.
///
/// For the same reason as above, the Layout Bridge program is required to be known by the
/// settlement contract for security reasons.
///
/// This program can be found here: <https://github.com/starkware-libs/cairo-lang/blob/8276ac35830148a397e1143389f23253c8b80e93/src/starkware/cairo/cairo_verifier/layouts/all_cairo/cairo_verifier.cairo>.
const LAYOUT_BRIDGE_PROGRAM_HASH: Felt =
    felt!("0x193641eb151b0f41674641089952e60bc3aded26e3cf42793655c562b8c3aa0");

/// The bootloader program hash is the program hash of the bootloader program.
///
/// This program is used to run the layout bridge program in SHARP. This program hash is also
/// required since the fact is computed based on the bootloader program hash and its output.
///
/// TODO: waiting for SHARP team to confirm if it's a custom bootloader or if we
/// can find it in cairo-lang.
const BOOTLOADER_PROGRAM_HASH: Felt =
    felt!("0x5ab580b04e3532b6b18f81cfa654a05e29dd8e2352d88df1e765a84072db07");

/// The contract address that handles fact verification.
///
/// This address points to Herodotus' Atlantic Fact Registry contract on Starknet Sepolia as we rely
/// on their services to generates and verifies proofs.
///
/// See on [Voyager](https://sepolia.voyager.online/contract/0x04ce7851f00b6c3289674841fd7a1b96b6fd41ed1edc248faccd672c26371b8c).
const ATLANTIC_FACT_REGISTRY_SEPOLIA: Felt =
    felt!("0x4ce7851f00b6c3289674841fd7a1b96b6fd41ed1edc248faccd672c26371b8c");

/// Deploys the settlement contract in the settlement layer and initializes it with the right
/// necessary states.
pub async fn deploy_settlement_contract(
    mut account: InitializerAccount,
    chain_id: Felt,
) -> Result<ContractAddress, ContractInitError> {
    // This is important! Otherwise all the estimate fees after a transaction will be executed
    // against invalid state.
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let mut sp = Spinner::new(spinners::Dots, "", Color::Blue);

    let result = async {
        // -----------------------------------------------------------------------
        // CONTRACT DEPLOYMENT
        // -----------------------------------------------------------------------

        let class = include_str!(
            "../../../../../crates/katana/contracts/build/appchain_core_contract.json"
        );

        let class = ContractClass::from_str(class)?;
        let class_hash = class.class_hash()?;

        // Check if the class has already been declared,
        match account.provider().get_class(BlockId::Tag(BlockTag::Pending), class_hash).await {
            Ok(..) => {
                // Class has already been declared, no need to do anything...
            }

            Err(ProviderError::StarknetError(StarknetError::ClassHashNotFound)) => {
                sp.update_text("Declaring contract...");
                let (rpc_class, casm_hash) = prepare_contract_declaration_params(class)?;

                let res = account
                    .declare_v2(rpc_class.into(), casm_hash)
                    .send()
                    .await
                    .inspect(|res| {
                        let tx = format!("{:#x}", res.transaction_hash);
                        trace!(target: "init", %tx, "Transaction sent");
                    })
                    .map_err(ContractInitError::DeclarationError)?;

                TransactionWaiter::new(res.transaction_hash, account.provider()).await?;
            }

            Err(err) => return Err(ContractInitError::Provider(err)),
        }

        sp.update_text("Deploying contract...");

        let salt = Felt::from(rand::random::<u64>());
        let factory = ContractFactory::new(class_hash, &account);

        const INITIAL_STATE_ROOT: Felt = Felt::ZERO;
        /// When updating the piltover contract with the genesis block (ie block number 0), in the
        /// attached StarknetOsOutput, the [previous block number] is expected to be
        /// `0x800000000000011000000000000000000000000000000000000000000000000`. Which is the
        /// maximum value of a [`Felt`].
        ///
        /// It is a value generated by StarknetOs.
        ///
        /// [previous block number]: https://github.com/keep-starknet-strange/piltover/blob/a7d6b17f855f2295a843bfd0ab0dcd696c6229a8/src/snos_output.cairo#L34
        const INITIAL_BLOCK_NUMBER: Felt = Felt::MAX;
        const INITIAL_BLOCK_HASH: BlockHash = Felt::ZERO;

        // appchain::constructor() https://github.com/keep-starknet-strange/piltover/blob/a7d6b17f855f2295a843bfd0ab0dcd696c6229a8/src/appchain.cairo#L122-L128
        let request = factory.deploy_v1(
            vec![
                // owner.
                account.address(),
                // state root.
                INITIAL_STATE_ROOT,
                // block_number must be magic value for genesis block.
                INITIAL_BLOCK_NUMBER,
                // block_hash.
                INITIAL_BLOCK_HASH,
            ],
            salt,
            false,
        );

        let res = request
            .send()
            .await
            .inspect(|res| {
                let tx = format!("{:#x}", res.transaction_hash);
                trace!(target: "init", %tx, "Transaction sent");
            })
            .map_err(ContractInitError::DeploymentError)?;

        TransactionWaiter::new(res.transaction_hash, account.provider()).await?;

        // -----------------------------------------------------------------------
        // CONTRACT INITIALIZATIONS
        // -----------------------------------------------------------------------

        let deployed_appchain_contract = request.deployed_address();
        let appchain = AppchainContract::new(deployed_appchain_contract, &account);

        // Compute the chain's config hash
        let snos_config_hash = compute_config_hash(
            chain_id,
            // NOTE:
            //
            // This is the default fee token contract address of chains generated using `katana
            // init`. We shouldn't hardcode this and need to handle this more
            // elegantly.
            felt!("0x2e7442625bab778683501c0eadbc1ea17b3535da040a12ac7d281066e915eea"),
        );

        // 1. Program Info

        sp.update_text("Setting program info...");

        let program_info = ProgramInfo {
            snos_config_hash,
            snos_program_hash: SNOS_PROGRAM_HASH,
            bootloader_program_hash: BOOTLOADER_PROGRAM_HASH,
            layout_bridge_program_hash: LAYOUT_BRIDGE_PROGRAM_HASH,
        };

        let res = appchain
            .set_program_info(&program_info)
            .send()
            .await
            .inspect(|res| {
                let tx = format!("{:#x}", res.transaction_hash);
                trace!(target: "init", %tx, "Transaction sent");
            })
            .map_err(ContractInitError::Initialization)?;

        TransactionWaiter::new(res.transaction_hash, account.provider()).await?;

        // 2. Fact Registry

        sp.update_text("Setting fact registry...");

        let res = appchain
            .set_facts_registry(&ATLANTIC_FACT_REGISTRY_SEPOLIA.into())
            .send()
            .await
            .inspect(|res| {
                let tx = format!("{:#x}", res.transaction_hash);
                trace!(target: "init", %tx, "Transaction sent");
            })
            .map_err(ContractInitError::Initialization)?;

        TransactionWaiter::new(res.transaction_hash, account.provider()).await?;

        // -----------------------------------------------------------------------
        // FINAL CHECKS
        // -----------------------------------------------------------------------

        check_program_info(chain_id, deployed_appchain_contract, account.provider()).await?;

        Ok(deployed_appchain_contract.into())
    }
    .await;

    match result {
        Ok(addr) => sp.success(&format!("Deployment successful ({addr})")),
        Err(..) => sp.fail("Deployment failed"),
    }
    result
}

/// Checks that the settlement contract is correctly configured.
///
/// The values checked are:-
/// * Program info (config hash, and StarknetOS program hash)
/// * Fact registry contract address
/// * Layout bridge program hash
pub async fn check_program_info(
    chain_id: Felt,
    appchain_address: Felt,
    provider: &RpcProvider,
) -> Result<(), ContractInitError> {
    let appchain = AppchainContractReader::new(appchain_address, provider);

    // Compute the chain's config hash
    let config_hash = compute_config_hash(
        chain_id,
        // NOTE:
        //
        // This is the default fee token contract address of chains generated using `katana init`.
        // We shouldn't hardcode this and need to handle this more elegantly.
        felt!("0x2e7442625bab778683501c0eadbc1ea17b3535da040a12ac7d281066e915eea"),
    );

    // Assert that the values are correctly set
    let (program_info_res, facts_registry_res) =
        tokio::join!(appchain.get_program_info().call(), appchain.get_facts_registry().call());

    let actual_program_info = program_info_res?;
    let facts_registry = facts_registry_res?;

    if actual_program_info.layout_bridge_program_hash != LAYOUT_BRIDGE_PROGRAM_HASH {
        return Err(ContractInitError::InvalidLayoutBridgeProgramHash {
            actual: actual_program_info.layout_bridge_program_hash,
            expected: LAYOUT_BRIDGE_PROGRAM_HASH,
        });
    }

    if actual_program_info.bootloader_program_hash != BOOTLOADER_PROGRAM_HASH {
        return Err(ContractInitError::InvalidBootloaderProgramHash {
            actual: actual_program_info.bootloader_program_hash,
            expected: BOOTLOADER_PROGRAM_HASH,
        });
    }

    if actual_program_info.snos_program_hash != SNOS_PROGRAM_HASH {
        return Err(ContractInitError::InvalidSnosProgramHash {
            actual: actual_program_info.snos_program_hash,
            expected: SNOS_PROGRAM_HASH,
        });
    }

    if actual_program_info.snos_config_hash != config_hash {
        return Err(ContractInitError::InvalidConfigHash {
            actual: actual_program_info.snos_config_hash,
            expected: config_hash,
        });
    }

    if facts_registry != ATLANTIC_FACT_REGISTRY_SEPOLIA.into() {
        return Err(ContractInitError::InvalidFactRegistry {
            actual: facts_registry.into(),
            expected: ATLANTIC_FACT_REGISTRY_SEPOLIA,
        });
    }

    Ok(())
}

/// Error that can happen during the initialization of the core contract.
#[derive(Error, Debug)]
pub enum ContractInitError {
    #[error("failed to declare contract: {0:#?}")]
    DeclarationError(AccountError<<InitializerAccount as Account>::SignError>),

    #[error("failed to deploy contract: {0:#?}")]
    DeploymentError(AccountError<<InitializerAccount as Account>::SignError>),

    #[error("failed to initialize contract: {0:#?}")]
    Initialization(AccountError<<InitializerAccount as Account>::SignError>),

    #[error(
        "invalid program info: layout bridge program hash mismatch - expected {expected:#x}, got \
         {actual:#x}"
    )]
    InvalidLayoutBridgeProgramHash { expected: Felt, actual: Felt },

    #[error(
        "invalid program info: bootloader program hash mismatch - expected {expected:#x}, got \
         {actual:#x}"
    )]
    InvalidBootloaderProgramHash { expected: Felt, actual: Felt },

    #[error(
        "invalid program info: snos program hash mismatch - expected {expected:#x}, got \
         {actual:#x}"
    )]
    InvalidSnosProgramHash { expected: Felt, actual: Felt },

    #[error("invalid program info: config hash mismatch - expected {expected:#x}, got {actual:#x}")]
    InvalidConfigHash { expected: Felt, actual: Felt },

    #[error("invalid program state: fact registry mismatch - expected {expected:}, got {actual}")]
    InvalidFactRegistry { expected: Felt, actual: Felt },

    #[error(transparent)]
    TxWaitingError(#[from] TransactionWaitingError),

    #[error("failed parsing contract class: {0}")]
    ContractParsing(#[from] ContractClassFromStrError),

    #[error(transparent)]
    ContractClassCompilation(#[from] ContractClassCompilationError),

    #[error(transparent)]
    ComputeClassHash(#[from] ComputeClassHashError),

    #[error(transparent)]
    Provider(#[from] ProviderError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<cairo_serde::Error> for ContractInitError {
    fn from(value: cairo_serde::Error) -> Self {
        match value {
            cairo_serde::Error::Provider(e) => Self::Provider(e),
            _ => Self::Other(anyhow!(value)),
        }
    }
}

fn prepare_contract_declaration_params(
    class: ContractClass,
) -> Result<(FlattenedSierraClass, CompiledClassHash)> {
    let casm_hash = class.clone().compile()?.class_hash()?;

    let rpc_class = RpcContractClass::try_from(class).expect("should be valid");
    let RpcContractClass::Class(class) = rpc_class else { unreachable!("unexpected legacy class") };
    let flattened: FlattenedSierraClass = class.try_into()?;

    Ok((flattened, casm_hash))
}

// NOTE: The reason why we're using the same address for both fee tokens is because we don't yet
// support having native fee token on the chain.
fn compute_config_hash(chain_id: Felt, fee_token: Felt) -> Felt {
    compute_starknet_os_config_hash(chain_id, fee_token, fee_token)
}

// https://github.com/starkware-libs/cairo-lang/blob/a86e92bfde9c171c0856d7b46580c66e004922f3/src/starkware/starknet/core/os/os_config/os_config.cairo#L1-L39
fn compute_starknet_os_config_hash(
    chain_id: Felt,
    deprecated_fee_token: Felt,
    fee_token: Felt,
) -> Felt {
    // A constant representing the StarkNet OS config version.
    const STARKNET_OS_CONFIG_VERSION: Felt = short_string!("StarknetOsConfig2");

    compute_hash_on_elements(&[
        STARKNET_OS_CONFIG_VERSION,
        chain_id,
        deprecated_fee_token,
        fee_token,
    ])
}

#[cfg(test)]
mod tests {
    use katana_primitives::{felt, Felt};
    use starknet::core::chain_id::{MAINNET, SEPOLIA};

    use super::compute_starknet_os_config_hash;

    const ETH_FEE_TOKEN: Felt =
        felt!("0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7");
    const STRK_FEE_TOKEN: Felt =
        felt!("0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d");

    // Source:
    //
    // - https://github.com/starkware-libs/cairo-lang/blob/8e11b8cc65ae1d0959328b1b4a40b92df8b58595/src/starkware/starknet/core/os/os_config/os_config_hash.json#L4
    // - https://docs.starknet.io/tools/important-addresses/#fee_tokens
    #[rstest::rstest]
    #[case::mainnet(felt!("0x5ba2078240f1585f96424c2d1ee48211da3b3f9177bf2b9880b4fc91d59e9a2"), MAINNET)]
    #[case::testnet(felt!("0x504fa6e5eb930c0d8329d4a77d98391f2730dab8516600aeaf733a6123432"), SEPOLIA)]
    fn calculate_config_hash(#[case] config_hash: Felt, #[case] chain: Felt) {
        let computed = compute_starknet_os_config_hash(chain, ETH_FEE_TOKEN, STRK_FEE_TOKEN);
        assert_eq!(computed, config_hash);
    }
}
