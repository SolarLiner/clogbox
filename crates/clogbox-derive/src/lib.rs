use darling::FromDeriveInput;
use proc_macro::TokenStream;
use quote::ToTokens;

#[cfg(feature = "enum")]
mod r#enum;

#[cfg(feature = "enum")]
#[proc_macro_derive(Enum, attributes(r#enum, display))]
pub fn derive_enum(item: TokenStream) -> TokenStream {
    match r#enum::DeriveEnum::from_derive_input(&syn::parse_macro_input!(item as syn::DeriveInput))
    {
        Ok(d) => d.into_token_stream().into(),
        Err(err) => err.write_errors().into(),
    }
}
