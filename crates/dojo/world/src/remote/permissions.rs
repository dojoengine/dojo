//! Manages the permissions loaded from the remote world.

use anyhow::Result;
use starknet::core::types::Felt;

use super::{
    CommonResourceRemoteInfo, ContractRemote, EventRemote, ModelRemote, NamespaceRemote,
    ResourceRemote,
};

pub trait PermissionsUpdateable {
    fn update_writer(&mut self, contract_address: Felt, is_writer: bool) -> Result<()>;
    fn update_owner(&mut self, contract_address: Felt, is_owner: bool) -> Result<()>;
}

impl PermissionsUpdateable for CommonResourceRemoteInfo {
    fn update_writer(&mut self, contract_address: Felt, is_writer: bool) -> Result<()> {
        if is_writer {
            self.writers.insert(contract_address);
        } else {
            self.writers.remove(&contract_address);
        }

        Ok(())
    }

    fn update_owner(&mut self, contract_address: Felt, is_owner: bool) -> Result<()> {
        if is_owner {
            self.owners.insert(contract_address);
        } else {
            self.owners.remove(&contract_address);
        }

        Ok(())
    }
}

impl PermissionsUpdateable for ContractRemote {
    fn update_writer(&mut self, contract_address: Felt, is_writer: bool) -> Result<()> {
        self.common.update_writer(contract_address, is_writer)
    }

    fn update_owner(&mut self, contract_address: Felt, is_owner: bool) -> Result<()> {
        self.common.update_owner(contract_address, is_owner)
    }
}

impl PermissionsUpdateable for ModelRemote {
    fn update_writer(&mut self, contract_address: Felt, is_writer: bool) -> Result<()> {
        self.common.update_writer(contract_address, is_writer)
    }

    fn update_owner(&mut self, contract_address: Felt, is_owner: bool) -> Result<()> {
        self.common.update_owner(contract_address, is_owner)
    }
}

impl PermissionsUpdateable for EventRemote {
    fn update_writer(&mut self, contract_address: Felt, is_writer: bool) -> Result<()> {
        self.common.update_writer(contract_address, is_writer)
    }

    fn update_owner(&mut self, contract_address: Felt, is_owner: bool) -> Result<()> {
        self.common.update_owner(contract_address, is_owner)
    }
}

impl PermissionsUpdateable for NamespaceRemote {
    fn update_writer(&mut self, contract_address: Felt, is_writer: bool) -> Result<()> {
        if is_writer {
            self.writers.insert(contract_address);
        } else {
            self.writers.remove(&contract_address);
        }

        Ok(())
    }

    fn update_owner(&mut self, contract_address: Felt, is_owner: bool) -> Result<()> {
        if is_owner {
            self.owners.insert(contract_address);
        } else {
            self.owners.remove(&contract_address);
        }

        Ok(())
    }
}

impl PermissionsUpdateable for ResourceRemote {
    fn update_writer(&mut self, contract_address: Felt, is_writer: bool) -> Result<()> {
        match self {
            ResourceRemote::Contract(contract) => {
                contract.update_writer(contract_address, is_writer)
            }
            ResourceRemote::Model(model) => model.update_writer(contract_address, is_writer),
            ResourceRemote::Event(event) => event.update_writer(contract_address, is_writer),
            ResourceRemote::Namespace(namespace) => {
                namespace.update_writer(contract_address, is_writer)
            }
        }
    }

    fn update_owner(&mut self, contract_address: Felt, is_owner: bool) -> Result<()> {
        match self {
            ResourceRemote::Contract(contract) => contract.update_owner(contract_address, is_owner),
            ResourceRemote::Model(model) => model.update_owner(contract_address, is_owner),
            ResourceRemote::Event(event) => event.update_owner(contract_address, is_owner),
            ResourceRemote::Namespace(namespace) => {
                namespace.update_owner(contract_address, is_owner)
            }
        }
    }
}
