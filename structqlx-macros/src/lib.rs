use proc_macro::TokenStream;

mod db_model;
mod utils;

#[derive(darling::FromAttributes, Default, Debug)]
#[darling(attributes(db_model))]
struct CrudModelOptions {
    table_name: Option<String>,
    #[cfg(feature = "postgres")]
    schema: Option<String>,
    generate_getters: Option<bool>,
    constructor: Option<bool>,
}

#[proc_macro_derive(DatabaseModel, attributes(db_model))]
pub fn derive_database_model(input: TokenStream) -> TokenStream {
    db_model::expand_db_model(input)
}
