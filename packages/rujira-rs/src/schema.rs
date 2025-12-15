use cosmwasm_schema::serde::de::DeserializeOwned;
use cosmwasm_std::{from_json, Binary, StdResult};

// fallible_macro! {
//     #[proc_macro_attribute]
//     pub fn rj_serde(
//         attr: proc_macro::TokenStream,
//         input: proc_macro::TokenStream,
//     ) -> syn::Result<proc_macro::TokenStream> {
//         // Parse options and input
//         let mut options: cw_serde::Options = syn::parse(attr)?;
//         let input = syn::parse(input)?;

//         // Override the crate path to "::rujira_schema"
//         options.crate_path = syn::parse_str("::rujira::schema").unwrap();

//         // Generate the new implementation with the modified crate path
//         let expanded = cw_serde::cw_serde_impl(options, input)?;

//         // Return the expanded token stream
//         Ok(expanded.into_token_stream().into())
//     }
// }

pub fn decode_execute_msg<T: DeserializeOwned>(msg: Binary) -> StdResult<T> {
    if let Ok(msg) = postcard::from_bytes::<T>(&msg) {
        return Ok(msg);
    }

    from_json::<T>(msg)
}
