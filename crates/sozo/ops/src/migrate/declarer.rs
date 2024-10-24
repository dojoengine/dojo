//! Declare operations for the migration.
//!
//! Since a resource can be found in different namespaces, we want to optimize
//! the declaration to avoid declaring several times the same contract.
//! Also, checking onchain if the class is declared is less expensive that trying to declare.
//!
//! Declare transactions can't be multicalled. The only way to do so is by having multiple accounts.

use std::collections::HashMap;
use std::sync::Arc;

use dojo_utils::{TransactionExt, TxnConfig};
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{
    BlockId, BlockTag, DeclareTransactionResult, Felt, FlattenedSierraClass, StarknetError,
};
use starknet::providers::{Provider, ProviderError};

use super::MigrationError;

/// A declarer is in charge of declaring contracts.
#[derive(Debug)]
pub struct Declarer {
    /// The classes to declare,  identified by their casm class hash.
    pub classes: HashMap<Felt, FlattenedSierraClass>,
}

impl Declarer {
    /// Creates a new declarer.
    pub fn new() -> Self {
        Self { classes: HashMap::new() }
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
    pub async fn declare_all<A>(
        self,
        account: &A,
        txn_config: TxnConfig,
    ) -> Result<(), MigrationError<A::SignError>>
    where
        A: ConnectedAccount + Send + Sync,
    {
        for (casm_class_hash, class) in self.classes {
            let class_hash = class.class_hash();

            // It's ok if the class is already declared, we just skip the declaration.
            match account.provider().get_class(BlockId::Tag(BlockTag::Pending), class_hash).await {
                Err(ProviderError::StarknetError(StarknetError::ClassHashNotFound)) => {}
                Ok(_) => continue,
                Err(e) => return Err(MigrationError::Provider(e)),
            }

            let DeclareTransactionResult { transaction_hash, class_hash } = account
                .declare_v2(Arc::new(class), casm_class_hash)
                .send_with_cfg(&txn_config)
                .await
                .map_err(MigrationError::Migrator)?;

            tracing::trace!(%transaction_hash, %class_hash, "Declared class.");
        }

        Ok(())
    }
}
