use proc_macro2::TokenStream;
use quote::{quote, quote_spanned, ToTokens};
use syn::{spanned::Spanned, Data, DeriveInput};

use crate::utils::{self, create_impls, ImplData};



pub fn flags(_attr: proc_macro::TokenStream, ast: DeriveInput) -> TokenStream {
    let Data::Enum(enum_data) = &ast.data else {
        return quote!{ compile_error!("Stringify is only supported on enums") };
    };

    let ident = &ast.ident;

    let mut zero_name = None;


    let mut variants     = Vec::with_capacity(enum_data.variants.len());

    let mut variant_exprs = Vec::new();

    for variant in &enum_data.variants {
        if !variant.fields.is_empty() {
            let span = variant.span();
            return quote_spanned! {span => compile_error!{ "Only fieldless variants are supported for flags" } }
        }

        if let Some((_, discriminant)) = &variant.discriminant {
            variant_exprs.push((variant.ident.to_string(), Some(discriminant.clone())));
        } else {
            variant_exprs.push((variant.ident.to_string(), None));
        };
        variants.push(variant.ident.clone());
    }

    let variant_vals = match utils::resolve_related_expressions_to_u128(&variant_exprs) {
        Ok(vals) => vals,
        Err(err) => return err.into_token_stream(),
    };

    let mut all_bits = 0u128;
    for (idx, val) in variant_vals.iter().enumerate() {
        all_bits |= val;

        if *val == 0 {
            zero_name = Some(enum_data.variants[idx].ident.to_string());
        }
    }

    let (zero_variant, zero_name) = match zero_name {
        Some(variant) => (quote! {}, variant),
        None => (quote! { #[allow(non_upper_case_globals)] const None: #ident = #ident{ bits: 0 }; }, "None".to_string()),
    };

    let vis = &ast.vis;
    
    let used_bit_count = if all_bits == 0 { 0 } else { all_bits.ilog2() };
    let ty = if used_bit_count > 64 {
        quote! { u128 }
    } else if used_bit_count > 32 {
        quote! { u64 }
    } else if used_bit_count > 16 {
        quote! { u32 }
    } else if all_bits > 8 {
        quote! { u16 }
    } else {
        quote! { u8 }
    };


    let impl_data = &[
        ImplData::BinOp    { trait_name: quote! { BitAnd       }, fn_name: quote! { bitand        }, op: quote! { &  } },
        ImplData::BinOp    { trait_name: quote! { BitXor       }, fn_name: quote! { bitxor        }, op: quote! { |  } },
        ImplData::BinOp    { trait_name: quote! { BitOr        }, fn_name: quote! { bitor         }, op: quote! { ^  } },
        ImplData::AssignOp { trait_name: quote! { BitAndAssign }, fn_name: quote! { bitand_assign }, op: quote! { &= } },
        ImplData::AssignOp { trait_name: quote! { BitXorAssign }, fn_name: quote! { bitxor_assign }, op: quote! { |= } },
        ImplData::AssignOp { trait_name: quote! { BitOrAssign  }, fn_name: quote! { bitor_assign  }, op: quote! { ^= } },
    ];
    let impls = create_impls(ident, impl_data);


    let ident_str = ident.to_string();

    quote! {
        #[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
        #[repr(transparent)]
        #vis struct #ident {
            bits: #ty
        }

        impl #ident {
            #zero_variant

            #(#[allow(non_upper_case_globals)] const #variants: #ident = #ident { bits: #variant_vals as #ty };)*

            pub const fn new(bits: #ty) -> Self {
                Self { bits }
            }

            pub const fn none() -> Self {
                Self { bits: 0 }
            }

            pub const fn all() -> Self {
                Self { bits: #all_bits as #ty }
            }

            pub const fn bits(&self) -> #ty {
                self.bits
            }

            pub const fn contains(&self, flags: #ident) -> bool {
                self.bits & flags.bits == flags.bits
            }

            pub const fn intersects(&self, flags: #ident) -> bool {
                self.bits & flags.bits != 0
            }

            pub const fn is_none(&self) -> bool {
                self.bits == 0
            }

            pub const fn is_any(&self) -> bool {
                self.bits != 0
            }

            pub const fn is_all(&self) -> bool {
                self.bits == Self::all().bits
            }

            pub fn count_ones(&self) -> u32 {
                self.bits.count_ones()
            }

            pub fn count_zeros(&self) -> u32 {
                self.bits.count_zeros()
            }

            pub fn set(&mut self, flags: #ident, enable: bool) {
                if enable {
                    self.bits |= flags.bits;
                } else {
                    self.bits &= !flags.bits;
                }
            }

            pub fn enable(&mut self, flags: #ident) {
                self.bits |= flags.bits;
            }

            pub fn disable(&mut self, flags: #ident) {
                self.bits &= !flags.bits;
            }
        }

        impl ::core::ops::Not for #ident {
            type Output = Self;
            fn not(self) -> Self {
                Self { bits: !self.bits }
            }
        }

        #(#impls)*

        impl From<#ty> for #ident {
            fn from(bits: #ty) -> Self {
                Self { bits }
            }
        }

        impl From<#ident> for #ty {
            fn from(flags: #ident) -> #ty {
                flags.bits
            }
        }

        impl Default for #ident {
            fn default() -> Self {
                Self::none()
            }
        }

        impl ::core::fmt::Display for #ident {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                use ::core::fmt::Write;

                if self.is_none() {
                    if f.alternate() {
                        write!(f, concat!(#ident_str, "::"))?;
                    }
                    write!(f, #zero_name)?;
                }

                let mut flags = *self;
                let mut started = false;
                #(
                    if flags.contains(#ident::#variants) {
                        if started {
                            write!(f, " | ")?;
                        }
                        if f.alternate() {
                            write!(f, concat!(#ident_str, "::"))?;
                        }
                        write!(f, stringify!(#variants))?;

                        flags &= !#ident::#variants; 
                        started = true;
                    }
                )*

                if flags.is_any() {
                    if started {
                        write!(f, " | ")?;
                    }
                    write!(f, "{:x}", flags.bits)?;
                }

                Ok(())
            }
        }
    }
}