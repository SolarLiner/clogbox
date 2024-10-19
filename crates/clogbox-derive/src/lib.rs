use darling::FromDeriveInput;
use proc_macro::TokenStream;
use quote::ToTokens;

mod r#enum;

#[proc_macro_derive(Enum, attributes(display))]
pub fn derive_enum(item: TokenStream) -> TokenStream {
    match r#enum::DeriveEnum::from_derive_input(&syn::parse_macro_input!(item as syn::DeriveInput))
    {
        Ok(d) => d.into_token_stream().into(),
        Err(err) => err.write_errors().into(),
    }
}
