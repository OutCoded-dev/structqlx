use darling::FromMeta;
use proc_macro::TokenStream;

mod db_model;

#[derive(FromMeta)]
struct CrudModelOptions {
    #[darling(default)]
    table_name: Option<String>,
    #[darling(default)]
    schema: Option<String>,
    #[darling(default)]
    generate_getters: bool,
}

#[proc_macro_attribute]
pub fn db_model(attr: TokenStream, input: TokenStream) -> TokenStream {
    db_model::expand_db_model(attr, input)
}
