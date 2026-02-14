use proc_macro2::TokenStream;
use quote::quote_spanned;
use syn::{spanned::Spanned, Expr, ExprLit, Lit};

macro_rules! derive_helper {
    ($trait_ident:ident, $fn_ident:ident $(, $helper:ident)*) => {
        #[proc_macro_derive($trait_ident, attributes($($helper),*))]
        pub fn $fn_ident(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
            quote::quote!{}.into()
        }
    };
}
pub(crate) use derive_helper;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Casing {
    None,
    SnakeCase,
}

pub fn to_case(orig: String, case: Casing) -> String {
    match case {
        Casing::None => orig,
        Casing::SnakeCase => {
            let mut name = String::with_capacity(orig.len());

            for (idx, ch) in orig.char_indices() {
                if ch.is_uppercase() {
                    let ch = ch.to_lowercase().to_string();
                    if idx == 0 {
                        name.push_str(&ch);
                    } else {
                        name.push('_');
                        name.push_str(&ch);
                    }
                } else {
                    name.push(ch);
                }
            }
            name
        },
    }
}

pub fn extract_string_literal(expr: &Expr) -> Result<String, TokenStream> {
    let err_val = |span| quote_spanned! {span => compile_error!("Expected string literal")};

    let Expr::Lit(lit) = expr else {
        return Err(err_val(expr.span()));
    };

    let Lit::Str(value) = &lit.lit else {
        return Err(err_val(expr.span()));
    };

    Ok(value.value())
}

pub fn eval_usize_simple(expr: &Expr) -> Result<usize, TokenStream> {
    match expr {
        Expr::Lit(lit) => eval_usize_lit(lit),

    
        _ => {
            let span = expr.span();
            Err(quote_spanned! {span => compile_error!("Cannot evaluate expression")})
        },
    }
}

pub fn eval_usize_lit(expr: &ExprLit) -> Result<usize, TokenStream> {
    match &expr.lit {
        syn::Lit::Int(int_lit) => {
            int_lit.base10_parse().map_err(|err| err.into_compile_error())
        },
        _ => {
            let span = expr.span();
            Err(quote_spanned! {span => compile_error("Can only resolve integer literals")})
        }
    }
}