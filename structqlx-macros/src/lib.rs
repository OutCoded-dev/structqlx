use proc_macro::TokenStream;

mod db_model;
mod utils;

#[proc_macro_derive(DatabaseModel, attributes(structqlx))]
pub fn derive_database_model(input: TokenStream) -> TokenStream {
    db_model::expand_db_model(input)
}
