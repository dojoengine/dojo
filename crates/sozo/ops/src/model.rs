use anyhow::Result;
use dojo_world::contracts::model::ModelReader;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{BlockId, BlockTag, FieldElement};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;

pub async fn model_class_hash(
    name: String,
    world_address: FieldElement,
    provider: JsonRpcClient<HttpTransport>,
) -> Result<()> {
    let mut world_reader = WorldContractReader::new(world_address, &provider);
    world_reader.set_block(BlockId::Tag(BlockTag::Pending));

    let model = world_reader.model_reader(&name).await?;

    println!("{:#x}", model.class_hash());

    Ok(())
}

pub async fn model_contract_address(
    name: String,
    world_address: FieldElement,
    provider: JsonRpcClient<HttpTransport>,
) -> Result<()> {
    let mut world_reader = WorldContractReader::new(world_address, &provider);
    world_reader.set_block(BlockId::Tag(BlockTag::Pending));

    let model = world_reader.model_reader(&name).await?;

    println!("{:#x}", model.contract_address());

    Ok(())
}

pub async fn model_schema(
    name: String,
    world_address: FieldElement,
    provider: JsonRpcClient<HttpTransport>,
    to_json: bool,
) -> Result<()> {
    let mut world_reader = WorldContractReader::new(world_address, &provider);
    world_reader.set_block(BlockId::Tag(BlockTag::Pending));

    let model = world_reader.model_reader(&name).await?;
    let schema = model.schema().await?;

    if to_json {
        println!("{}", serde_json::to_string_pretty(&schema)?)
    } else {
        println!("{schema}");
    }

    Ok(())
}

pub async fn model_get(
    name: String,
    keys: Vec<FieldElement>,
    world_address: FieldElement,
    provider: JsonRpcClient<HttpTransport>,
) -> Result<()> {
    let mut world_reader = WorldContractReader::new(world_address, &provider);
    world_reader.set_block(BlockId::Tag(BlockTag::Pending));

    let model = world_reader.model_reader(&name).await?;
    let entity = model.entity(&keys).await?;

    println!("{entity}");

    Ok(())
}
