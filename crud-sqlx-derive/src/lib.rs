use darling::FromMeta;
use proc_macro::TokenStream;
use syn::token::Token;

mod db_model;
mod utils;

#[derive(FromMeta)]
struct CrudModelOptions {
    #[darling(default)]
    table_name: Option<String>,
    #[darling(default)]
    schema: Option<String>,
    #[darling(default)]
    generate_getters: bool,
}

#[proc_macro_derive(CrudModel, attributes(db_model))]
pub fn derive_database_model(input: TokenStream) -> TokenStream {
    db_model::expand_db_model(input)
}

#[proc_macro_attribute]
pub fn db_model(_attr: TokenStream, _input: TokenStream) -> TokenStream {
    TokenStream::new()
}
