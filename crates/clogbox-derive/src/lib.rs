use darling::FromDeriveInput;
use proc_macro::TokenStream;
use quote::ToTokens;

mod r#enum;
mod params;

#[proc_macro_derive(Enum, attributes(display, prefix))]
pub fn derive_enum(item: TokenStream) -> TokenStream {
    match r#enum::DeriveEnum::from_derive_input(&syn::parse_macro_input!(item as syn::DeriveInput))
    {
        Ok(d) => d.into_token_stream().into(),
        Err(err) => err.write_errors().into(),
    }
}

#[proc_macro_derive(Params, attributes(param))]
pub fn derive_params(item: TokenStream) -> TokenStream {
    match params::DeriveParams::from_derive_input(&syn::parse_macro_input!(item as 
        syn::DeriveInput)) {
        Ok(d) => d.into_token_stream().into(),
        Err(err) => err.write_errors().into(),
    }
}