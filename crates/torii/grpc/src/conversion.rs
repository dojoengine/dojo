use std::collections::HashMap;
use std::str::FromStr;

use starknet::core::types::FromStrError;
use starknet_crypto::FieldElement;

use crate::protos;

impl TryFrom<protos::types::ModelMetadata> for dojo_types::schema::ModelMetadata {
    type Error = FromStrError;
    fn try_from(value: protos::types::ModelMetadata) -> Result<Self, Self::Error> {
        Ok(Self {
            name: value.name,
            packed_size: value.packed_size,
            unpacked_size: value.unpacked_size,
            class_hash: FieldElement::from_str(&value.class_hash)?,
        })
    }
}

impl TryFrom<protos::types::WorldMetadata> for dojo_types::WorldMetadata {
    type Error = FromStrError;
    fn try_from(value: protos::types::WorldMetadata) -> Result<Self, Self::Error> {
        let components = value
            .models
            .into_iter()
            .map(|component| Ok((component.name.clone(), component.try_into()?)))
            .collect::<Result<HashMap<_, dojo_types::schema::ModelMetadata>, _>>()?;

        Ok(dojo_types::WorldMetadata {
            models: components,
            world_address: FieldElement::from_str(&value.world_address)?,
            world_class_hash: FieldElement::from_str(&value.world_class_hash)?,
            executor_address: FieldElement::from_str(&value.executor_address)?,
            executor_class_hash: FieldElement::from_str(&value.executor_class_hash)?,
        })
    }
}

impl From<dojo_types::schema::EntityModel> for protos::types::EntityModel {
    fn from(value: dojo_types::schema::EntityModel) -> Self {
        Self {
            model: value.model,
            keys: value.keys.into_iter().map(|key| format!("{key:#}")).collect(),
        }
    }
}
