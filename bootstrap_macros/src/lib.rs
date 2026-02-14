use proc_macro::TokenStream;
use syn::DeriveInput;

mod utils;

mod enum_helpers;

#[proc_macro_attribute]
pub fn enum_utils(attr: TokenStream, tokens: TokenStream) -> TokenStream {
    let ast = match syn::parse::<DeriveInput>(tokens) {
        Ok(ast) => ast,
        Err(err) => return err.into_compile_error().into()
    };
    enum_helpers::enum_utils(attr, ast).into()
}

utils::derive_helper!(EnumHelper, enum_helper, string, fmt);