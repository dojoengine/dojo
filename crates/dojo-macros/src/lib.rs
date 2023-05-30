extern crate proc_macro;

use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, ItemStruct};

/// ### Example
/// ```rust
/// /// Send `starknet_getBlockWithTxHashes` request to a remote node.
/// fn get_block_with_tx_hashes(
///     mut events: bevy::ecs::event::EventReader<StarknetGetBlockWithTxHashes>,
///     query: bevy::ecs::system::Query<&LightClient>,
/// ) -> Result<()> {
///     events.iter().try_for_each(|params| {
///         use crate::light_client::starknet::StarknetRequest;
///         use crate::light_client::LightClientRequest;
///         let client = query.get_single()?;
///         client.send(LightClientRequest::starknet_get_block_with_tx_hashes(params.clone()))?;
///
///         Ok(())
///     })
/// }
/// ``````
#[proc_macro_derive(LightClientSystem)]
pub fn derive_light_client_system(tokens: TokenStream) -> TokenStream {
    let input = parse_macro_input!(tokens as ItemStruct);
    let ident = &input.ident;

    let chain_name_lower_case_str =
        if ident.to_string().starts_with("Starknet") { "starknet" } else { "ethereum" };
    let chain_name_upper_case_str = chain_name_lower_case_str.to_case(Case::UpperCamel);

    // Example: `starknet`
    let chain_name_lower: TokenStream2 = chain_name_lower_case_str.parse().unwrap();
    // Example: `Starknet`
    let chain_name_upper: TokenStream2 = chain_name_upper_case_str.parse().unwrap();

    let event_name_short_upper_camel_str =
        ident.to_string().replace("Starknet", "").replace("Ethereum", "");
    let event_name_short_lower_camel_str = event_name_short_upper_camel_str.to_case(Case::Camel);
    let event_name_short_snake_str = if event_name_short_upper_camel_str.starts_with("L1ToL2") {
        let val = event_name_short_upper_camel_str.replace("L1ToL2", "").to_case(Case::Snake);
        format!("l1_to_l2_{val}")
    } else if event_name_short_upper_camel_str.starts_with("L2ToL1") {
        let val = event_name_short_upper_camel_str.replace("L2ToL1", "").to_case(Case::Snake);
        format!("l2_to_l1_{val}")
    } else {
        event_name_short_upper_camel_str.to_case(Case::Snake)
    };

    // Example: `GetBlockWithTxHashes`
    let event_name_short_upper_camel: TokenStream2 =
        event_name_short_upper_camel_str.parse().unwrap();
    // Example: `get_block_with_tx_hashes`
    let method_name_snake: TokenStream2 = event_name_short_snake_str.parse().unwrap();

    // Example: `StarknetRequest`
    let request_name: TokenStream2 = format!("{}Request", chain_name_upper).parse().unwrap();

    // Example: (`_`, ``) | (`params`, `(params.clone())`)
    let (params_arg, params) = if input.fields.len() == 0 {
        (quote! {_}, quote! {})
    } else {
        (quote! {params}, quote! {(params.clone())})
    };

    let json_rpc_method_name_str = format!("{chain_name_lower}_{event_name_short_lower_camel_str}");

    // Example: `/// Send `starknet_getBlockWithTxHashes` request to a remote node.`
    let doc_string: TokenStream2 =
        format!("/// Send `{}` request to a remote node.", json_rpc_method_name_str)
            .parse()
            .unwrap();

    let gen = quote! {
        #doc_string
        fn #method_name_snake(
            mut events: bevy::ecs::event::EventReader<#ident>,
            query: bevy::ecs::system::Query<&crate::light_client::LightClient>,
        ) -> Result<()> {
            events.iter().try_for_each(|#params_arg| {
                use crate::light_client::{
                    LightClientRequest,
                    #chain_name_lower::#request_name,
                };

                let client = query.get_single()?;
                let req =
                    LightClientRequest::#chain_name_upper(#request_name::#event_name_short_upper_camel #params);

                client.send(req)?;

                Ok(())
            })
        }
    };

    gen.into()
}
