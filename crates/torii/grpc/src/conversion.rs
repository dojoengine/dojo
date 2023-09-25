use std::collections::HashMap;
use std::str::FromStr;

use starknet::core::types::FromStrError;
use starknet_crypto::FieldElement;

use crate::protos;

impl TryFrom<protos::types::ComponentMetadata> for dojo_types::component::ComponentMetadata {
    type Error = FromStrError;
    fn try_from(value: protos::types::ComponentMetadata) -> Result<Self, Self::Error> {
        Ok(Self {
            name: value.name,
            size: value.size,
            class_hash: FieldElement::from_str(&value.class_hash)?,
        })
    }
}

impl TryFrom<protos::types::SystemMetadata> for dojo_types::system::SystemMetadata {
    type Error = FromStrError;
    fn try_from(value: protos::types::SystemMetadata) -> Result<Self, Self::Error> {
        Ok(Self { name: value.name, class_hash: FieldElement::from_str(&value.class_hash)? })
    }
}

impl TryFrom<protos::types::WorldMetadata> for dojo_types::WorldMetadata {
    type Error = FromStrError;
    fn try_from(value: protos::types::WorldMetadata) -> Result<Self, Self::Error> {
        let components = value
            .components
            .into_iter()
            .map(|component| Ok((component.name.clone(), component.try_into()?)))
            .collect::<Result<HashMap<_, dojo_types::component::ComponentMetadata>, _>>()?;

        let systems = value
            .systems
            .into_iter()
            .map(|system| Ok((system.name.clone(), system.try_into()?)))
            .collect::<Result<HashMap<_, dojo_types::system::SystemMetadata>, _>>()?;

        Ok(dojo_types::WorldMetadata {
            systems,
            components,
            world_address: FieldElement::from_str(&value.world_address)?,
            world_class_hash: FieldElement::from_str(&value.world_class_hash)?,
            executor_address: FieldElement::from_str(&value.executor_address)?,
            executor_class_hash: FieldElement::from_str(&value.executor_class_hash)?,
        })
    }
}

impl From<dojo_types::component::EntityComponent> for protos::types::EntityComponent {
    fn from(value: dojo_types::component::EntityComponent) -> Self {
        Self {
            component: value.component,
            keys: value.keys.into_iter().map(|key| format!("{key:#}")).collect(),
        }
    }
}
