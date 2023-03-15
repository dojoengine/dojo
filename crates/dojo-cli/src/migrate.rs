// use std::env::current_dir;
use std::path::PathBuf;

// use cairo_lang_compiler::db::RootDatabase;
// use cairo_lang_compiler::project::get_main_crate_ids_from_project;
use clap::Args;
use comfy_table::Table;
// use dojo_lang::component::find_components;
// use dojo_lang::db::DojoRootDatabaseBuilderEx;
// use dojo_lang::plugin::get_contract_address;
// use dojo_lang::system::find_systems;
// use smol_str::SmolStr;
// use starknet::core::types::FieldElement;
// use starknet::providers::jsonrpc::models::{BlockId, BlockTag, ErrorCode};
// use starknet::providers::jsonrpc::{HttpTransport, JsonRpcClient, JsonRpcClientError, RpcError};
// use url::Url;

#[derive(Args)]
pub struct MigrateArgs {
    #[clap(help = "Source directory")]
    path: Option<PathBuf>,

    #[clap(short, long, help = "Perform a dry run and outputs the plan to be executed")]
    plan: bool,
}

pub async fn run(_args: MigrateArgs) {
    // let source_dir = match args.path {
    //     Some(path) => path,
    //     None => current_dir().unwrap(),
    // };

    // let config = ProjectConfig::from_directory(&source_dir).unwrap_or_else(|error| {
    //     panic!("Problem creating project config: {:?}", error);
    // });

    // let rpc_client = JsonRpcClient::new(HttpTransport::new(
    //     Url::parse("https://starknet-goerli.cartridge.gg/").unwrap(),
    // ));

    // let db = &mut
    // RootDatabase::builder().with_dojo_config(config.clone()).build().unwrap_or_else(
    //     |error| {
    //         panic!("Migration initialization error: {:?}", error);
    //     },
    // );
    // let main_crate_ids = get_main_crate_ids_from_project(db, &config.clone().into());

    // let components = find_components(db, &main_crate_ids);
    // let systems = find_systems(db, &main_crate_ids);

    let mut table = Table::new();
    table.set_header(vec!["Name", "Type", "Address", "Deployed"]);

    // async fn get_row(
    //     rpc_client: &JsonRpcClient<HttpTransport>,
    //     typ: SmolStr,
    //     name: SmolStr,
    //     config: ProjectConfig,
    // ) -> Vec<SmolStr> {
    //     let contract_address = get_contract_address(
    //         name.as_str(),
    //         config.clone().content.world.initializer_class_hash.unwrap_or_default(),
    //         config.content.world.address.unwrap_or_default(),
    //     );

    //     let existing_class_hash = rpc_client
    //         .get_class_hash_at(&BlockId::Tag(BlockTag::Latest), contract_address)
    //         .await
    //         .unwrap_or_else(|err| match err {
    //             JsonRpcClientError::RpcError(RpcError::Code(ErrorCode::ContractNotFound)) => {
    //                 FieldElement::ZERO
    //             }
    //             _ => panic!("Failed getting contract hash: {err:?}"),
    //         });

    //     vec![
    //         name,
    //         typ,
    //         format!("{:#x}", contract_address).into(),
    //         if existing_class_hash == FieldElement::ZERO { "false".into() } else { "true".into()
    // },     ]
    // }

    // for component in components {
    //     table.add_row(
    //         get_row(&rpc_client, "Component".into(), component.name, config.clone()).await,
    //     );
    // }

    // for system in systems {
    //     table.add_row(get_row(&rpc_client, "System".into(), system.name, config.clone()).await);
    // }

    println!("{table}");
}
