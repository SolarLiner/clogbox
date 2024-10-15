use darling::{ast, FromDeriveInput, FromField, FromVariant};
use proc_macro2::TokenStream;
use quote::quote;

#[derive(Debug, FromField)]
#[darling(attributes(r#enum))]
#[allow(unused)]
struct EnumField {
    ident: Option<syn::Ident>,
    ty: syn::Type,
}

#[derive(Debug, FromVariant)]
#[darling(supports(unit), attributes(r#enum))]
struct EnumVariant {
    ident: syn::Ident,
    #[darling(rename = "display")]
    name: Option<String>,
    #[allow(unused)]
    fields: ast::Fields<EnumField>,
}

#[derive(Debug, FromDeriveInput)]
#[darling(supports(enum_unit))]
pub(crate) struct DeriveEnum {
    ident: syn::Ident,
    data: ast::Data<EnumVariant, ()>,
}

impl DeriveEnum {
    fn impl_cast_from(&self, ident: &syn::Ident, fields: &[EnumVariant]) -> TokenStream {
        let arms = fields
            .iter()
            .enumerate()
            .map(|(i, EnumVariant { ident, .. })| {
                let id = syn::Index::from(i);
                quote! { #id => Self::#ident }
            });
        quote! {
            impl ::az::CastFrom<usize> for #ident {
                fn cast_from(idx: usize) -> Self {
                    match idx {
                        #(#arms,)*
                        _ => unreachable!(),
                    }
                }
            }
        }
    }

    fn impl_cast(&self, ident: &syn::Ident, fields: &[EnumVariant]) -> TokenStream {
        let arms = fields
            .iter()
            .enumerate()
            .map(|(i, EnumVariant { ident, .. })| {
                let id = syn::Index::from(i);
                quote! { Self::#ident => #id }
            });
        quote! {
            impl ::az::Cast<usize> for #ident {
                fn cast(self) -> usize {
                    match self {
                        #(#arms),*
                    }
                }
            }
        }
    }

    fn impl_enum(&self, ident: &syn::Ident, fields: &[EnumVariant]) -> TokenStream {
        let count = fields.len();
        let count_ty = syn::parse_str::<syn::Type>(&format!("::typenum::U{count}")).unwrap();
        let arms = fields.iter().map(|field| {
            let ident = &field.ident;
            let name = field
                .name
                .clone()
                .unwrap_or_else(|| field.ident.to_string());
            eprintln!("Name: {name:?}");
            quote! {
                Self::#ident => ::std::borrow::Cow::Borrowed(#name)
            }
        });
        quote! {
            impl ::clogbox_core::r#enum::Enum for #ident {
                type Count = #count_ty;

                fn name(&self) -> ::std::borrow::Cow<str> {
                    match self {
                        #(#arms),*
                    }
                }
            }
        }
    }
}

impl quote::ToTokens for DeriveEnum {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self { ident, data } = self;
        let ast::Data::Enum(fields) = data else {
            tokens.extend(
                syn::Error::new(ident.span(), "Cannot derive Enum for non enum types")
                    .to_compile_error(),
            );
            return;
        };

        tokens.extend(self.impl_cast_from(ident, fields));
        tokens.extend(self.impl_cast(ident, fields));
        tokens.extend(self.impl_enum(ident, fields));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens;

    #[test]
    fn test_simple_derive() {
        let input = syn::parse_str(
            /* rust */
            r#"enum Params {
                Cutoff,
                Resonance,
                Drive,
                #[r#enum(display = "Input FM")]
                InputFM,
            }"#,
        )
            .expect("Parsing valid code");
        let from_derive_input =
            DeriveEnum::from_derive_input(&input).expect("Parsing valid code");
        eprintln!("{from_derive_input:#?}");
        let output = from_derive_input.into_token_stream().to_string();
        insta::assert_snapshot!(prettyplease::unparse(&syn::parse_file(&output).unwrap()));
    }
}