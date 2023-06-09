use starknet::core::types::{BlockId, FieldElement, FunctionCall};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::{Provider, ProviderError};

use crate::manifest::Member;
use crate::world::WorldContractReader;

pub struct ComponentClass<'a, P: Provider + Sync> {
    world: &'a WorldContractReader<'a, P>,
    hash: FieldElement,
}

impl<'a, P: Provider + Sync> ComponentClass<'a, P> {
    pub async fn new(
        world: &'a WorldContractReader<'a, P>,
        name: FieldElement,
        block_id: BlockId,
    ) -> Result<ComponentClass<'a, P>, ProviderError<P::Error>> {
        let res = world
            .provider
            .call(
                FunctionCall {
                    contract_address: world.address,
                    calldata: vec![name],
                    entry_point_selector: get_selector_from_name("component").unwrap(),
                },
                block_id,
            )
            .await?;

        Ok(Self { world, hash: res[0] })
    }

    pub fn hash(&self) -> FieldElement {
        self.hash
    }

    pub async fn schema(&self, block_id: BlockId) -> Result<Vec<Member>, ProviderError<P::Error>> {
        let res = self
            .world
            .call(
                get_selector_from_name("LibraryCall").unwrap(),
                vec![self.hash, get_selector_from_name("schema").unwrap()],
                block_id,
            )
            .await?;

        Ok(res.iter().map(|_x| Member::default()).collect())
    }
}
