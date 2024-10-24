use darling::{ast, FromDeriveInput, FromField, FromVariant};
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};

#[derive(Debug, FromField)]
#[darling(attributes(r#enum))]
#[allow(unused)]
struct EnumField {
    ident: Option<syn::Ident>,
    ty: syn::Type,
}

#[derive(Debug, FromVariant)]
#[darling(attributes(r#enum), supports(unit, newtype))]
struct EnumVariant {
    ident: syn::Ident,
    #[darling(rename = "display")]
    name: Option<String>,
    prefix: Option<String>,
    fields: ast::Fields<EnumField>,
}

#[derive(Debug, FromDeriveInput)]
#[darling(supports(enum_unit, enum_newtype))]
pub(crate) struct DeriveEnum {
    ident: syn::Ident,
    data: ast::Data<EnumVariant, ()>,
}

impl DeriveEnum {
    fn impl_cast_from(&self, ident: &syn::Ident, fields: &[EnumVariant]) -> TokenStream {
        let arms = fields
            .iter()
            .map(|EnumVariant { ident, fields, .. }| match fields.len() {
                0 => quote! { if i == 0 { return Self::#ident; } else { i -= 1; } },
                1 => {
                    let EnumField { ty, .. } = &fields.fields[0];
                    quote! {
                        if i < <<#ty as ::clogbox_core::r#enum::Enum>::Count as ::typenum::Unsigned>::USIZE {
                            return Self::#ident(#ty::cast_from(i));
                        } else {
                            i -= <<#ty as ::clogbox_core::r#enum::Enum>::Count as ::typenum::Unsigned>::USIZE;
                        }
                    }
                }
                _ => syn::Error::new(
                    ident.span(),
                    "Cannot derive Enum for enum with variants having more than 1 field",
                )
                .into_compile_error(),
            });
        quote! {
            #[automatically_derived]
            impl ::clogbox_core::r#enum::az::CastFrom<usize> for #ident {
                fn cast_from(mut i: usize) -> Self {
                    #(#arms)*
                    unreachable!();
                }
            }
        }
    }

    fn impl_cast(&self, ident: &syn::Ident, fields: &[EnumVariant]) -> TokenStream {
        let arms = fields
            .iter()
            .map(|EnumVariant { ident, fields, .. }| match fields.len() {
                0 => quote! { if let Self::#ident = self { return i; } else { i += 1;} },
                1 => {
                    let EnumField { ty, .. } = &fields.fields[0];
                    quote! {
                        if let Self::#ident(value) = self {
                            return i + value.cast();
                        } else {
                            i += <#ty as ::clogbox_core::r#enum::Enum>::Count::USIZE;
                        }
                    }
                }
                _ => syn::Error::new(
                    ident.span(),
                    "Cannot derive Enum for enum with variants having more than 1 field",
                )
                .into_compile_error(),
            });
        quote! {
            #[automatically_derived]
            impl ::clogbox_core::r#enum::az::Cast<usize> for #ident {
                fn cast(self) -> usize {
                    let mut i = 0;
                    #(#arms)*
                    unreachable!()
                }
            }
        }
    }

    fn impl_enum(&self, ident: &syn::Ident, fields: &[EnumVariant]) -> TokenStream {
        let (unit, variant) = fields
            .iter()
            .partition::<Vec<_>, _>(|field| field.fields.is_empty());
        let unit_count_ty = syn::parse_str::<syn::Type>(&format!("::typenum::U{}", unit.len()))
            .unwrap()
            .to_token_stream();
        let count_ty = if variant.is_empty() {
            unit_count_ty
        } else {
            let variant_count_ty = variant
                .into_iter()
                .map(|EnumVariant { fields, .. }| {
                    let EnumField { ty, .. } = &fields.fields[0];
                    quote! { <#ty as ::clogbox_core::r#enum::Enum>::Count }
                })
                .reduce(|a, b| quote! { ::typenum::operator_aliases::Sum<#a, #b> })
                .unwrap();
            quote! { ::typenum::operator_aliases::Sum<#unit_count_ty, #variant_count_ty> }
        };
        let arms = fields.iter().map(|EnumVariant { ident, name, fields, prefix }| {
            let name = name
                .clone()
                .unwrap_or_else(|| ident.to_string());
            match fields.len() {
                0 => quote! { Self::#ident => ::std::borrow::Cow::from(#name) },
                1 => {
                    let borrow = if let Some(prefix) = prefix {
                        let format_string = format!("{prefix} {{}}");
                        quote! { ::std::borrow::Cow::Owned(format!(#format_string, inner.name())) }
                    } else {
                        quote! { inner.name() }
                    };
                    quote! {
                        Self::#ident(inner) => {
                            #borrow
                        }
                    }
                }
                _ => syn::Error::new(ident.span(), "Cannot derive Enum for enum with variants having more than 1 field").into_compile_error(),
            }
        });
        quote! {
            #[automatically_derived]
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
        let from_derive_input = DeriveEnum::from_derive_input(&input).expect("Parsing valid code");
        eprintln!("{from_derive_input:#?}");
        let output = from_derive_input.into_token_stream().to_string();
        insta::assert_snapshot!(prettyplease::unparse(&syn::parse_file(&output).unwrap()));
    }

    #[test]
    fn test_derive_nested() {
        let input = syn::parse_str(
            r#"
            enum Outer {
                A,
                #[r#enum(prefix = "B")]
                B(Inner),
                C(Inner),
            }"#,
        )
        .expect("Parsing valid code");
        let from_derive_input = DeriveEnum::from_derive_input(&input).expect("Parsing valid code");
        eprintln!("{from_derive_input:#?}");
        let output = from_derive_input.into_token_stream().to_string();
        insta::assert_snapshot!(prettyplease::unparse(&syn::parse_file(&output).unwrap()));
    }
}
