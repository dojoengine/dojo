use crate::local::{ExternalContractClassLocal, ExternalContractLocal};

/// The difference between a local and a remote external contract class.

#[derive(Debug)]
pub enum ExternalContractClassDiff {
    Created(ExternalContractClassLocal),
    Synced(ExternalContractClassLocal),
}

/// The difference between a local and a remote external contract.
#[derive(Debug)]
pub enum ExternalContractDiff {
    Created(ExternalContractLocal),
    Synced(ExternalContractLocal),
}

impl ExternalContractClassDiff {
    pub fn class_data(&self) -> ExternalContractClassLocal {
        match self {
            ExternalContractClassDiff::Created(c) => c.clone(),
            ExternalContractClassDiff::Synced(c) => c.clone(),
        }
    }
}

impl ExternalContractDiff {
    pub fn contract_data(&self) -> ExternalContractLocal {
        match self {
            ExternalContractDiff::Created(c) => c.clone(),
            ExternalContractDiff::Synced(c) => c.clone(),
        }
    }
}
