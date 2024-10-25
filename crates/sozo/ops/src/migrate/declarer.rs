//! Declare operations for the migration.
//!
//! Since a resource can be found in different namespaces, we want to optimize
//! the declaration to avoid declaring several times the same contract.
//! Also, checking onchain if the class is declared is less expensive that trying to declare.
//!
//! Declare transactions can't be multicalled. The only way to do so is by having multiple accounts.

use std::collections::HashMap;
use std::sync::Arc;

use dojo_utils::{TransactionExt, TransactionWaiter, TxnConfig};
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{
    BlockId, BlockTag, DeclareTransactionResult, Felt, FlattenedSierraClass, StarknetError,
};
use starknet::providers::{Provider, ProviderError};

use super::MigrationError;

/// A declarer is in charge of declaring contracts.
#[derive(Debug)]
pub struct Declarer<A>
where
    A: ConnectedAccount + Send + Sync,
{
    /// The account to use to deploy the contracts.
    pub account: A,
    /// The transaction configuration.
    pub txn_config: TxnConfig,
    /// The classes to declare,  identified by their casm class hash.
    pub classes: HashMap<Felt, FlattenedSierraClass>,
}

/// The output of a declaration.
#[derive(Debug)]
pub struct DeclareOutput {
    /// The transaction hash of the declaration.
    pub transaction_hash: Felt,
}

impl DeclareOutput {
    /// Returns true if the class was already declared.
    pub fn already_declared(&self) -> bool {
        self.transaction_hash == Felt::ZERO
    }
}

impl<A> Declarer<A>
where
    A: ConnectedAccount + Send + Sync,
{
    /// Creates a new declarer.
    pub fn new(account: A, txn_config: TxnConfig) -> Self {
        Self { account, txn_config, classes: HashMap::new() }
    }

    /// Adds a class to the declarer, do nothing if the class is already known.
    pub fn add_class(&mut self, casm_class_hash: Felt, class: FlattenedSierraClass) {
        self.classes.entry(casm_class_hash).or_insert(class);
    }

    /// Declares all the classes registered in the declarer with a single account.
    ///
    /// Takes ownership of the declarer to avoid cloning the classes.
    ///
    /// The order of the declarations is not guaranteed.
    pub async fn declare_all(self) -> Result<(), MigrationError<A::SignError>> {
        for (casm_class_hash, class) in self.classes {
            Self::declare(casm_class_hash, class, &self.account, &self.txn_config).await?;
        }

        Ok(())
    }

    /// Declares a class.
    pub async fn declare(
        casm_class_hash: Felt,
        class: FlattenedSierraClass,
        account: &A,
        txn_config: &TxnConfig,
    ) -> Result<DeclareOutput, MigrationError<A::SignError>> {
        let class_hash = class.class_hash();

        match account.provider().get_class(BlockId::Tag(BlockTag::Pending), class_hash).await {
            Err(ProviderError::StarknetError(StarknetError::ClassHashNotFound)) => {}
            Ok(_) => {
                tracing::trace!(
                    class_hash = format!("{:#066x}", class_hash),
                    "Class already declared."
                );
                return Ok(DeclareOutput { transaction_hash: Felt::ZERO });
            }
            Err(e) => return Err(MigrationError::Provider(e)),
        }

        let DeclareTransactionResult { transaction_hash, class_hash } = account
            .declare_v2(Arc::new(class), casm_class_hash)
            .send_with_cfg(&txn_config)
            .await
            .map_err(MigrationError::Migrator)?;

        tracing::trace!(
            transaction_hash = format!("{:#066x}", transaction_hash),
            class_hash = format!("{:#066x}", class_hash),
            casm_class_hash = format!("{:#066x}", casm_class_hash),
            "Declared class."
        );

        // Since TxnConfig::wait doesn't work for now, we wait for the transaction manually.
        if txn_config.wait {
            TransactionWaiter::new(transaction_hash, &account.provider()).await?;
        }

        Ok(DeclareOutput { transaction_hash })
    }
}
