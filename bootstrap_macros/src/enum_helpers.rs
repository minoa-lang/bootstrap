use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{parse_macro_input, spanned::Spanned, Data, DeriveInput, Ident};

use crate::utils::*;


pub fn get_casing(ident: &Ident) -> Result<Casing, &'static str> {
    match ident.to_string().as_str() {
        "snake_case" => Ok(Casing::SnakeCase),
        _ => Err("Unknown case"),
    }
}

pub fn enum_utils(attr: proc_macro::TokenStream, ast: DeriveInput) -> TokenStream {
    let Data::Enum(enum_data) = &ast.data else {
        return quote!{ compile_error!("Stringify is only supported on enums") };
    };
    
    let mut as_str = false;
    let mut str_casing = Casing::None;
    let mut display = false;
    let mut from_idx = false;

    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("as_str") {
            as_str = true;
            return meta.parse_nested_meta(|meta| {
                let ident = meta.path.require_ident()?;
                str_casing = match get_casing(ident) {
                    Ok(casing) => casing,
                    Err(err) => return Err(meta.error(err)),
                };
                Ok(())
            });
        } else if meta.path.is_ident("display") {
            display = true;
            return  Ok(());
        } else if meta.path.is_ident("from_idx") {
            from_idx = true;
            return  Ok(());
        }

        Err(meta.error("Unsupported utility"))
    });
    let parse_err_out = (|| { parse_macro_input!(attr with parser); proc_macro::TokenStream::new() })();
    if !parse_err_out.is_empty() {
        return parse_err_out.into();
    }

    let mut variant_names = Vec::new();
    let mut variant_simple_patterns = Vec::new();
    let mut variant_full_patterns = Vec::new();
    let mut variant_fmt_args = Vec::new();
    let mut variant_fmt_discards = Vec::new();
    let mut variant_vals = Vec::new();

    // Different names in case future features need to use collect this
    let collect_simple_patterns = from_idx || as_str || display;
    let collect_full_patterns = display;
    let collect_names = as_str;
    let collect_fmt = display;

    let mut cur_idx = 0;
    for variant in &enum_data.variants {
        let ident = &variant.ident;

        if collect_simple_patterns {
            let pat = match &variant.fields {
                syn::Fields::Named(_) => quote! { Self::#ident{ .. } },
                syn::Fields::Unnamed(_) => quote! { Self::#ident(..) },
                syn::Fields::Unit => quote! { Self::#ident },
            };
            variant_simple_patterns.push(pat);
        }
        
        if collect_full_patterns {
            let (pat, discard) = match &variant.fields {
                syn::Fields::Named(src_fields) => {
                    let mut fields = Vec::new();
                    for field in &src_fields.named {
                        fields.push(field.ident.as_ref().unwrap().clone());
                    }
                    (quote! { Self::#ident{#(#fields),*} }, Some(quote! { #(_ = #fields;)* }))
                },
                syn::Fields::Unnamed(src_fields) => {
                    let mut fields = Vec::new();
                    for i in 0..src_fields.unnamed.len() {
                        let name = format!("_{i}");
                        let ident = Ident::new(&name, Span::call_site());
                        fields.push(ident);
                    }
                    (quote! { Self::#ident(#(#fields),*) }, None)
                },
                syn::Fields::Unit => (quote!{ Self::#ident }, None),
            };
            
            variant_fmt_discards.push(discard);
            variant_full_patterns.push(pat);
        }

        if from_idx {
            match &variant.discriminant {
                Some((_, expr)) => {
                    let idx = match eval_usize_simple(expr) {
                        Ok(idx) => idx,
                        Err(err) => return err,
                    };
                    variant_vals.push(idx); 
                    cur_idx = idx;
                },
                None => {
                    variant_vals.push(cur_idx);
                    cur_idx += 1;
                },
            }
        }

        if collect_names {
            let mut name = None;
            for attr in &variant.attrs {
                if !attr.meta.path().is_ident("string") { continue; }

                let inner = match &attr.meta {
                    syn::Meta::Path(_) => {
                        let span = attr.span();
                        return quote_spanned! { span => compile_error!("Expected value for inner `string`") };
                    },
                    syn::Meta::List(meta) => match meta.parse_args() {
                        Ok(inner) => inner,
                        Err(err) => return err.into_compile_error()
                    },
                    syn::Meta::NameValue(meta) => meta.value.clone(),
                };

                name = match extract_string_literal(&inner) {
                    Ok(name) => Some(name),
                    Err(err) => return err,
                };
                break;
            }

            let name = match name {
                Some(name) => name,
                None => to_case(ident.to_string(), str_casing),
            };
            variant_names.push(name);
        }
        
        if collect_fmt {
            let mut fmt_args = None;
            for attr in &variant.attrs {
                if !attr.meta.path().is_ident("fmt") { continue; }

                let meta = match attr.meta.require_list() {
                    Ok(meta) => meta,
                    Err(err) => return err.into_compile_error(),
                };
                fmt_args = Some(meta.tokens.clone());
            }


            let fmt_args = match fmt_args {
                Some(fmt) => fmt,
                None => {
                     let name = to_case(ident.to_string(), str_casing);
                     quote!{ #name }
                },
            };

            variant_fmt_args.push(fmt_args);
        }


    }
    

    let ident = &ast.ident;

    let from_idx = from_idx.then(|| quote! {
        pub fn from_idx(idx: usize) -> Option<Self> {
            match idx {
                #(#variant_vals => Some(#variant_simple_patterns),)*
                _ => None,
            }
        }
    });

    let as_str = as_str.then(|| quote!{
        pub fn as_str(self) -> &'static str {
            match self {
                #(#variant_simple_patterns => #variant_names),*
            }
        }
    });

    let to_string = display.then(|| if as_str.is_some() {
        quote! {
            impl std::fmt::Display for #ident {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str(self.as_str())
                }
            }
        }
    } else {
        quote! {
            impl std::fmt::Display for #ident {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    match self {
                        #(#variant_full_patterns => {
                            #variant_fmt_discards
                            write!(f, #variant_fmt_args)
                        }),*
                    }
                }
            }
        }
    });

    let tmp = quote! {
        #[derive(bootstrap_macros::EnumHelper)]
        #ast

        impl #ident {
            #from_idx
            #as_str
        }

        #to_string
    };
    //println!("{}", tmp);
    tmp
}

