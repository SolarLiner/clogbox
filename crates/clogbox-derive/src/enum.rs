use darling::{ast, FromDeriveInput, FromField, FromVariant};
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::TypeGenerics;

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
    generics: syn::Generics,
    data: ast::Data<EnumVariant, ()>,
}

impl DeriveEnum {
    fn impl_from_usize(&self, fields: &[EnumVariant]) -> TokenStream {
        let arms = fields
            .iter()
            .map(|EnumVariant { ident, fields, .. }| match fields.len() {
                0 => quote! { if i == 0 { return Self::#ident; } else { i -= 1; } },
                1 => {
                    let EnumField { ty, .. } = &fields.fields[0];
                    quote! {
                        if i < <<#ty as ::clogbox_enum::Enum>::Count as ::clogbox_enum::typenum::Unsigned>::USIZE {
                            return Self::#ident(<#ty as ::clogbox_enum::Enum>::from_usize(i));
                        } else {
                            i -= <<#ty as ::clogbox_enum::Enum>::Count as ::clogbox_enum::typenum::Unsigned>::USIZE;
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
            fn from_usize(mut i: usize) -> Self {
                #(#arms)*
                unreachable!();
            }
        }
    }

    fn impl_to_usize(&self, fields: &[EnumVariant]) -> TokenStream {
        let arms = fields
            .iter()
            .map(|EnumVariant { ident, fields, .. }| match fields.len() {
                0 => quote! { if let Self::#ident = self { return i; } else { i += 1;} },
                1 => {
                    let EnumField { ty, .. } = &fields.fields[0];
                    quote! {
                        if let Self::#ident(value) = self {
                            return i + value.to_usize();
                        } else {
                            i += <<#ty as ::clogbox_enum::Enum>::Count as ::clogbox_enum::typenum::Unsigned>::USIZE;
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
            fn to_usize(self) -> usize {
                let mut i = 0;
                #(#arms)*
                unreachable!()
            }
        }
    }

    fn impl_enum(&self, ident: &syn::Ident, fields: &[EnumVariant]) -> TokenStream {
        let (ty_generics, impl_generics) = self.generics_for_impl(quote! { ::clogbox_enum::Enum });
        let (unit, variant) = fields.iter().partition::<Vec<_>, _>(|field| field.fields.is_empty());
        let unit_count_ty = syn::parse_str::<syn::Type>(&format!("::clogbox_enum::typenum::U{}", unit.len()))
            .unwrap()
            .to_token_stream();
        let count_ty = Self::count_ty(&variant, unit_count_ty.clone());
        let where_clause = self.where_clause(&variant, unit_count_ty);

        let name = Self::impl_name(fields);
        let from_usize = self.impl_from_usize(fields);
        let to_usize = self.impl_to_usize(fields);

        quote! {
            #[automatically_derived]
            impl #impl_generics ::clogbox_enum::Enum for #ident #ty_generics #where_clause {
                type Count = #count_ty;

                #from_usize

                #to_usize

                #name
            }
        }
    }

    fn count_ty_iter<'a>(variant: &'a [&'a EnumVariant]) -> impl 'a + Iterator<Item = TokenStream> {
        variant
            .iter()
            .map(|EnumVariant { fields, .. }| {
                let EnumField { ty, .. } = &fields.fields[0];
                quote! { <#ty as ::clogbox_enum::Enum>::Count }
            })
            .inspect(|tok| eprintln!("Count Type Item: {tok}"))
    }

    fn count_ty(variant: &[&EnumVariant], unit_count_ty: TokenStream) -> TokenStream {
        std::iter::once(unit_count_ty)
            .chain(Self::count_ty_iter(variant))
            .reduce(|a, b| {
                quote! { <#a as std::ops::Add<#b>>::Output }
            })
            .unwrap()
    }

    fn where_clause(&self, variant: &[&EnumVariant], unit_count_ty: TokenStream) -> TokenStream {
        let where_clause = {
            let mut last_variant_input = None;
            let additional = Self::count_ty_iter(variant)
                .scan(quote! { #unit_count_ty }, |acc, ty| {
                    let ret = quote! {
                        #acc: ::std::ops::Add<#ty>
                    };
                    *acc = quote! { <#acc as ::std::ops::Add<#ty>>::Output };
                    last_variant_input.replace(acc.clone());
                    Some(ret)
                })
                .collect::<Vec<_>>();
            let where_clause = self.generics.where_clause.as_ref();
            if let Some(where_clause) = where_clause {
                let punct = (!where_clause.predicates.trailing_punct()).then(|| quote! { , });
                let last_variant_input = last_variant_input.map(|inp| {
                    quote! { #inp: ::clogbox_enum::typenum::Unsigned + ::clogbox_enum::generic_array::ArrayLength }
                });
                quote! {
                    #where_clause #punct
                    #(#additional,)*
                    #last_variant_input
                }
            } else {
                let last_variant_input = last_variant_input.map(|inp| {
                    quote! { #inp: ::clogbox_enum::typenum::Unsigned + ::clogbox_enum::generic_array::ArrayLength }
                });
                quote! { where
                    #(#additional,)*
                    #last_variant_input
                }
            }
        };
        where_clause
    }

    fn impl_name(fields: &[EnumVariant]) -> TokenStream {
        let arms = fields.iter().map(
            |EnumVariant {
                 ident,
                 name,
                 fields,
                 prefix,
             }| {
                let name = name.clone().unwrap_or_else(|| ident.to_string());
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
                    _ => syn::Error::new(
                        ident.span(),
                        "Cannot derive Enum for enum with variants having more than 1 field",
                    )
                    .into_compile_error(),
                }
            },
        );

        let name = quote! {
            fn name(&self) -> ::std::borrow::Cow<str> {
                match self {
                    #(#arms),*
                }
            }
        };
        name
    }

    fn generics_for_impl(&self, bound: TokenStream) -> (TypeGenerics, TokenStream) {
        let (_, ty_generics, _) = self.generics.split_for_impl();
        let generics = self.generics.type_params().map(|syn::TypeParam { ident, .. }| {
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
        (ty_generics, generics)
    }
}

impl quote::ToTokens for DeriveEnum {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self { ident, data, .. } = self;
        let ast::Data::Enum(fields) = data else {
            tokens.extend(syn::Error::new(ident.span(), "Cannot derive Enum for non enum types").to_compile_error());
            return;
        };

        tokens.extend(self.impl_enum(ident, fields));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens;

    fn format_output(from_derive_input: DeriveEnum) -> String {
        let output = from_derive_input.to_token_stream().to_string();

        match syn::parse_file(&output) {
            Ok(contents) => prettyplease::unparse(&contents),
            Err(err) => {
                eprintln!("Failed to parse output: {}", err);
                output
            }
        }
    }

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
        insta::assert_snapshot!(format_output(from_derive_input));
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

    #[test]
    fn test_derive_enum_with_where_clause() {
        let input = syn::parse_str(
            r#"
        enum Constrained<T> 
        where
            T: std::fmt::Debug {
            VariantA(T),
            VariantB,
        }"#,
        )
        .expect("Parsing valid code");
        let from_derive_input = DeriveEnum::from_derive_input(&input).expect("Parsing valid code");
        eprintln!("{from_derive_input:#?}");
        insta::assert_snapshot!(format_output(from_derive_input));
    }

    #[test]
    fn test_derive_generic_enum() {
        let input = syn::parse_str(
            r#"
        enum Generic<T> {
            VariantA(T),
            VariantB,
            #[r#enum(prefix = "Special")]
            SpecialVariant(T),
        }"#,
        )
        .expect("Parsing valid code");
        let from_derive_input = DeriveEnum::from_derive_input(&input).expect("Parsing valid code");
        eprintln!("{from_derive_input:#?}");
        insta::assert_snapshot!(format_output(from_derive_input));
    }

    #[test]
    fn test_derive_generic_enum_with_nested() {
        let input = syn::parse_str(
            r#"        
            pub enum MixedEnum<C> {
                First,
                Second(Inner),
                Third(C),
            }
            "#,
        )
        .expect("Parsing valid code");
        let from_derive_input = DeriveEnum::from_derive_input(&input).expect("Parsing valid code");
        insta::assert_snapshot!(format_output(from_derive_input));
    }
}
