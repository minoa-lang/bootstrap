use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::{spanned::Spanned, BinOp, Expr, ExprLit, Ident, Lit};

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

pub fn eval_usize_simple(expr: &Expr) -> Result<u128, TokenStream> {
    match expr {
        Expr::Lit(lit) => eval_usize_lit(lit),

    
        _ => {
            let span = expr.span();
            Err(quote_spanned! {span => compile_error!("Cannot evaluate expression")})
        },
    }
}

pub fn eval_usize_lit(expr: &ExprLit) -> Result<u128, TokenStream> {
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



pub fn resolve_related_expressions_to_u128(named_exprs: &[(String, Option<Expr>)]) -> Result<Vec<u128>, TokenStream> {
    let mut value_mapping = HashMap::new();
    let mut res = vec![0; named_exprs.len()];

    // If we need more passes than we have expressions, we have a circular dependency
    'outer: for _ in 0..named_exprs.len() {
        for (idx, (name, expr)) in named_exprs.iter().enumerate() {
            if !value_mapping.contains_key(name) {
                match expr {
                    Some(expr) => match resolve_expression_to_u128(expr, &value_mapping) {
                        Ok(Some(val)) => {
                            res[idx] = val;
                            value_mapping.insert(name.clone(), val);
                        },
                        Ok(None) => (),
                        Err(err) => return Err(err)
                    },
                    None => if idx == 0 {
                        value_mapping.insert(name.clone(), 1);
                        res[idx] = 1;
                    } else {
                        let prev_name = &named_exprs[idx - 1].0;
                        if let Some(prev_val) = value_mapping.get(prev_name) {
                            let val = (*prev_val + 1).next_power_of_two();
                            value_mapping.insert(name.clone(), val);
                            res[idx] = val;
                        }
                    }
                }
            }

            if value_mapping.len() == named_exprs.len() {
                break 'outer;
            }
        }
    }
    Ok(res)
}

pub fn resolve_expression_to_u128(expr: &Expr, value_mapping: &HashMap<String, u128>) -> Result<Option<u128>, TokenStream> {
    match expr {
        Expr::Lit(lit) => eval_usize_lit(lit).map(|val| Some(val)),
        Expr::Path(path) => {
            if let Some(qself) = &path.qself {
                let span = qself.span();
                return Err(quote_spanned! {span => compile_error!("Qualified paths are not supprted")})
            }
            let ident = path.path.require_ident().map_err(|err| err.into_compile_error())?;
            if let Some(val) = value_mapping.get(&ident.to_string()) {
                Ok(Some(*val))
            } else {
                Ok(None)
            }

        },
        Expr::Binary(bin) => {
            let Some(left) = resolve_expression_to_u128(&bin.left, value_mapping)? else { return Ok(None) };
            let Some(right) = resolve_expression_to_u128(&bin.right, value_mapping)? else { return Ok(None) };

            match bin.op {
                BinOp::Add(_)    => Ok(Some(left + right)),
                BinOp::Sub(_)    => Ok(Some(left - right)),
                BinOp::Mul(_)    => Ok(Some(left * right)),
                BinOp::Div(_)    => Ok(Some(left / right)),
                BinOp::BitOr(_)  => Ok(Some(left | right)),
                BinOp::BitAnd(_) => Ok(Some(left & right)),
                BinOp::BitXor(_) => Ok(Some(left ^ right)),
                BinOp::Shl(_)    => Ok(Some(left << right)),
                BinOp::Shr(_)    => Ok(Some(left >> right)),
                _ => {
                    let span = bin.op.span();
                    Err(quote_spanned! { span => compile_error!("Unsupported operator") })
                }
            }
        }

        Expr::Paren(paren) => resolve_expression_to_u128(&paren.expr, value_mapping),
        _ => {
            let span = expr.span();
            return Err(quote_spanned! { span => compile_error!("Unsupported expression") });
        }
    }
}




pub enum ImplData {
    BinOp{ trait_name: TokenStream, fn_name: TokenStream, op: TokenStream },
    AssignOp{ trait_name: TokenStream, fn_name: TokenStream, op: TokenStream },
}

pub fn create_impls(ty: &Ident, impl_data: &[ImplData]) -> Vec<TokenStream> {
    let mut impls = Vec::new();
    for data in impl_data {
        impls.push(match data {
            ImplData::BinOp { trait_name, fn_name, op } => quote! {
                impl ::core::ops::#trait_name for #ty {
                    type Output = Self;
                    fn #fn_name(self, rhs: Self) -> Self {
                        Self { bits: self.bits #op rhs.bits }
                    }
                }
            },
            ImplData::AssignOp { trait_name, fn_name, op } => quote! {
                impl ::core::ops::#trait_name for #ty {
                    fn #fn_name(&mut self, rhs: Self) {
                        self.bits #op rhs.bits;
                    }
                }
            },
        });
    }
    impls
}