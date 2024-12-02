use darling::{ast, FromDeriveInput, FromVariant};
use proc_macro2::{Ident, TokenStream};
use quote::{quote, ToTokens};
use syn::{TypeGenerics, WhereClause};

#[derive(Debug, FromVariant)]
#[darling(attributes(param), supports(unit, newtype))]
struct EnumVariant {
    ident: syn::Ident,
    fields: ast::Fields<()>,
    range: Option<syn::Expr>,
    flags: Option<syn::Expr>,
    default: Option<syn::Expr>,
    value_to_string: Option<syn::Expr>,
    string_to_value: Option<syn::Expr>,
}

#[derive(Debug, FromDeriveInput)]
#[darling(supports(enum_unit, enum_newtype))]
pub(crate) struct DeriveParams {
    ident: syn::Ident,
    generics: syn::Generics,
    data: ast::Data<EnumVariant, ()>,
}

impl DeriveParams {
    fn impl_params(&self, ident: &syn::Ident, fields: &[EnumVariant]) -> TokenStream {
        let metadata_variants = fields.iter().map(
            |EnumVariant {
                 ident: variant_ident,
                 fields,
                 range,
                 flags,
                 default,
                 ..
             }| {
                if fields.is_empty() {
                    let range = range.as_ref().map(|range| quote! { range: #range, });
                    let flags = flags.as_ref().map(|flags| quote! { flags: #flags, });
                    let default = default
                        .as_ref()
                        .map(|default| quote! { default: #default, });
                    quote! {
                        #[allow(clippy::needless_update)]
                        #ident::#variant_ident => ::clogbox_core::param::ParamMetadata {
                            #range
                            #flags
                            #default
                            ..Default::default()
                        }
                    }
                } else if fields.len() == 1 {
                    quote! {
                        #ident::#variant_ident(val) => val.metadata()
                    }
                } else {
                    return syn::Error::new_spanned(
                        variant_ident,
                        "Enum variants with more than one field are not supported",
                    )
                    .to_compile_error();
                }
            },
        );
        let v2s_variants = fields.iter().map(
            |EnumVariant {
                 ident: variant_ident,
                fields,
                 value_to_string,
                 ..
             }| {
                if fields.is_empty() {
                    value_to_string
                        .as_ref()
                        .map(|v2s| quote! { #ident::#variant_ident => #v2s(value) })
                        .unwrap_or_else(
                            || quote! { #ident::#variant_ident => ::clogbox_core::param::value_to_string_default(value) },
                        )
                } else if fields.len() == 1 {
                    quote! {
                        #ident::#variant_ident(v) => v.value_to_string(value)
                    }
                } else {
                    return syn::Error::new_spanned(variant_ident, "Enum variants with more than one field are not supported")
                        .to_compile_error();
                }
            },
        );
        let s2v_variants = fields.iter().map(
            |EnumVariant {
                ident: variant_ident,
                fields,
                 string_to_value, ..
             }| {
                if fields.is_empty() {
                    string_to_value
                        .as_ref()
                        .map(|s2v| quote! { #ident::#variant_ident => #s2v(string) })
                        .unwrap_or_else(|| {
                            quote! { #ident::#variant_ident => ::clogbox_core::param::string_to_value_default(string) }
                        })
                } else if fields.len() == 1 {
                    quote! {
                        #ident::#variant_ident(v) => v.string_to_value(string)
                    }
                } else {
                    return syn::Error::new_spanned(variant_ident, "Enum variants with more than one field are not supported")
                        .to_compile_error();
                }
            },
        );

        let (ty_generics, where_clause, generics) = self.generics_for_impl(quote! {
        ::clogbox_core::param::Params });
        let where_clause = if let Some(where_clause) = where_clause {
            quote! {
                #where_clause
                Self: ::clogbox_core::r#enum::Enum,
            }
        } else {
            quote! { where Self: ::clogbox_core::r#enum::Enum, }
        };
        quote! {
            #[automatically_derived]
            impl #generics ::clogbox_core::param::Params for #ident #ty_generics #where_clause {
                fn metadata(&self) -> ::clogbox_core::param::ParamMetadata {
                    match self {
                        #(#metadata_variants),*
                    }
                }

                fn value_to_string(&self, value: ::clogbox_core::param::Normalized) -> String {
                    match self {
                        #(#v2s_variants),*
                    }
                }

                fn string_to_value(&self, string: &str) -> Result<::clogbox_core::param::Normalized, String> {
                    match self {
                        #(#s2v_variants),*
                    }
                }
            }
        }
    }

    fn generics_for_impl(
        &self,
        bound: TokenStream,
    ) -> (TypeGenerics, Option<&WhereClause>, TokenStream) {
        let (_, ty_generics, where_clause) = self.generics.split_for_impl();
        let generics = self
            .generics
            .type_params()
            .map(|syn::TypeParam { ident, .. }| {
                quote! {
                    #ident:
                    #bound
                }
            });
        let generics = self
            .generics
            .lifetimes()
            .map(ToTokens::to_token_stream)
            .chain(generics)
            .chain(self.generics.const_params().map(ToTokens::to_token_stream));
        let generics = if !self.generics.params.is_empty() {
            quote! {
                <#(#generics),*>
            }
        } else {
            quote! {}
        };
        (ty_generics, where_clause, generics)
    }
}

impl quote::ToTokens for DeriveParams {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self { ident, data, .. } = self;
        let ast::Data::Enum(fields) = data else {
            tokens.extend(syn::Error::new_spanned(ident, "Expected an enum").to_compile_error());
            return;
        };

        tokens.extend(self.impl_params(ident, fields));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use quote::ToTokens;
    use syn::parse_quote;

    fn format_output(input: DeriveParams) -> String {
        let raw_output = input.to_token_stream().to_string();
        match syn::parse_file(&raw_output) {
            Ok(file) => prettyplease::unparse(&file).to_string(),
            Err(err) => {
                eprintln!("Error parsing Rust: {err}");
                raw_output
            }
        }
    }

    #[test]
    fn test_derive_params_macro_parsing() {
        let input: syn::DeriveInput = parse_quote! {
            #[derive(Params)]
            enum TestEnum {
                #[param(range = "1..=10", flags = "some_flag", default = "5")]
                VariantOne,
                #[param(range = "20..=30", value_to_string = "v2s_function")]
                VariantTwo,
            }
        };

        let derive_params = DeriveParams::from_derive_input(&input).expect("Failed to parse input");
        insta::assert_snapshot!(format_output(derive_params));
    }

    #[test]
    fn test_derive_params_macro_nested() {
        let input: syn::DeriveInput = parse_quote! {
            #[derive(Params)]
            enum NestedEnum {
                Standalone,
                Nested(Inner),
            }
        };

        let derive_params = DeriveParams::from_derive_input(&input).expect("Failed to parse input");
        insta::assert_snapshot!(format_output(derive_params));
    }

    #[test]
    fn test_derive_params_macro_with_where_clause() {
        let input: syn::DeriveInput = parse_quote! {
        #[derive(Params)]
        enum WhereClauseEnum<T> 
        where 
            T: std::fmt::Debug + Default,
        {
            Variant(T),
            OtherVariant,
        }
    };

        let derive_params = DeriveParams::from_derive_input(&input).expect("Failed to parse input");
        insta::assert_snapshot!(format_output(derive_params));
    }

    #[test]
    fn test_derive_params_macro_with_generic() {
        let input: syn::DeriveInput = parse_quote! {
        #[derive(Params)]
        enum GenericEnum<T> {
            Variant(T),
            OtherVariant,
        }
    };

        let derive_params = DeriveParams::from_derive_input(&input).expect("Failed to parse input");
        insta::assert_snapshot!(format_output(derive_params));
    }
}
