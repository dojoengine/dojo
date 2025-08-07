//! Declare operations for the migration.
//!
//! Since a resource can be found in different namespaces, we want to optimize
//! the declaration to avoid declaring several times the same contract.
//! Also, checking onchain if the class is declared is less expensive that trying to declare.
//!
//! Declare transactions can't be multicalled. The only way to do so is by having multiple accounts.

use std::collections::HashMap;
use std::sync::Arc;

use starknet::accounts::ConnectedAccount;
use starknet::core::types::{
    BlockId, BlockTag, DeclareTransactionResult, Felt, FlattenedSierraClass, StarknetError,
};
use starknet::providers::{Provider, ProviderError};
use tracing::trace;

use crate::{TransactionError, TransactionExt, TransactionResult, TransactionWaiter, TxnConfig};

#[derive(Debug, Clone)]
pub struct LabeledClass {
    /// The label of the class.
    pub label: String,
    /// The casm class hash of the class.
    pub casm_class_hash: Felt,
    /// The class itself.
    pub class: FlattenedSierraClass,
}

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
    pub classes: HashMap<Felt, LabeledClass>,
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
    pub fn add_class(&mut self, labeled_class: LabeledClass) {
        self.classes.entry(labeled_class.casm_class_hash).or_insert(labeled_class);
    }

    /// Extends the classes to the declarer.
    pub fn extend_classes(&mut self, classes: Vec<LabeledClass>) {
        for labeled_class in classes {
            self.classes.entry(labeled_class.casm_class_hash).or_insert(labeled_class);
        }
    }

    /// Declares all the classes registered in the declarer with a single account.
    ///
    /// Takes ownership of the declarer to avoid cloning the classes.
    ///
    /// The order of the declarations is not guaranteed.
    pub async fn declare_all(
        self,
    ) -> Result<Vec<TransactionResult>, TransactionError<A::SignError>> {
        let mut results = vec![];

        for (_, labeled_class) in self.classes {
            results.push(Self::declare(labeled_class, &self.account, &self.txn_config).await?);
        }

        Ok(results)
    }

    /// Declares a class.
    pub async fn declare(
        labeled_class: LabeledClass,
        account: &A,
        txn_config: &TxnConfig,
    ) -> Result<TransactionResult, TransactionError<A::SignError>> {
        let class_hash = labeled_class.class.class_hash();

        if is_declared(&labeled_class.label, class_hash, account.provider()).await? {
            return Ok(TransactionResult::Noop);
        }

        let casm_class_hash = labeled_class.casm_class_hash;

        trace!(
            label = labeled_class.label,
            class_hash = format!("{:#066x}", class_hash),
            casm_class_hash = format!("{:#066x}", casm_class_hash),
            "Declaring class."
        );

        let DeclareTransactionResult { transaction_hash, class_hash } = account
            .declare_v3(Arc::new(labeled_class.class), casm_class_hash)
            .send_with_cfg(txn_config)
            .await?;

        trace!(
            label = labeled_class.label,
            transaction_hash = format!("{:#066x}", transaction_hash),
            class_hash = format!("{:#066x}", class_hash),
            casm_class_hash = format!("{:#066x}", labeled_class.casm_class_hash),
            "Declared class."
        );

        if txn_config.wait {
            let receipt = TransactionWaiter::new(transaction_hash, &account.provider()).await?;

            if txn_config.receipt {
                return Ok(TransactionResult::HashReceipt(transaction_hash, Box::new(receipt)));
            }
        }

        Ok(TransactionResult::Hash(transaction_hash))
    }
}

/// Check if the provided class is already declared.
pub async fn is_declared<P>(
    class_name: &String,
    class_hash: Felt,
    provider: &P,
) -> Result<bool, ProviderError>
where
    P: Provider,
{
    match provider.get_class(BlockId::Tag(BlockTag::PreConfirmed), class_hash).await {
        Err(ProviderError::StarknetError(StarknetError::ClassHashNotFound)) => Ok(false),
        Ok(_) => {
            trace!(
                label = class_name,
                class_hash = format!("{:#066x}", class_hash),
                "Class already declared."
            );
            Ok(true)
        }
        Err(e) => Err(e),
    }
}
