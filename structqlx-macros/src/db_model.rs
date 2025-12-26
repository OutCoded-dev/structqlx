use std::collections::HashMap;

use crate::utils::{get_field_type_from_option, is_option};
use convert_case::{Case, Casing};
use darling::FromAttributes;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{ToTokens, quote};
use syn::{
    Data, DataStruct, DeriveInput, Field, Fields, Ident, Token, Type, parse_macro_input,
    punctuated::Punctuated,
};

#[derive(darling::FromAttributes, Debug)]
#[darling(attributes(structqlx))]
struct CrudModelOptions {
    table_name: Option<String>,
    #[cfg(feature = "postgres")]
    schema: Option<String>,
    generate_getters: Option<bool>,
    constructor: Option<bool>,
    engine: syn::Path,
}

pub fn expand_db_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ident = input.ident.clone();

    let opts = match CrudModelOptions::from_attributes(&input.attrs) {
        Ok(opts) => opts,
        Err(err) => return err.write_errors().into(),
    };

    let table_name = opts
        .table_name
        .unwrap_or(ident.to_string().to_case(Case::Snake));

    let schema = quote! {};
    #[cfg(feature = "postgres")]
    let schema = {
        let schema = opts.schema.unwrap_or("public".to_string());
        quote! {
            #[doc("Return the schema in which the #table_name table resides")]
            #[inline]
            fn schema() -> &'static str {
                #schema
            }
        }
    };

    let model_struct: DataStruct = match &input.data {
        Data::Struct(model_struct) => model_struct.clone(),
        _ => panic!("CrudModel can only be derived for structs."),
    };

    // ID related fields.
    let id_type = get_id_field_type(model_struct.clone());
    let create_method = generate_create_method(model_struct.clone(), &opts.engine);
    let update_method = generate_update_method(model_struct.clone(), &opts.engine);
    let save_method = generate_save_method(model_struct.clone(), &opts.engine);

    let constructor = if let Some(true) = opts.constructor {
        generate_constructor(&ident, &model_struct)
    } else {
        quote! {}
    };

    let getters = if let Some(true) = opts.generate_getters {
        generate_getters(&model_struct)
    } else {
        quote! {}
    };

    let db_engine = &opts.engine;

    let find_by_funcs =
        generate_find_by_funcs(&model_struct.fields, &table_name, db_engine, &ident);

    let table_name_fn = quote! {
        /// Return the table name for the model.
        fn table_name() -> String {
            #table_name.to_string()
        }
    };

    let field_names = get_field_names(model_struct.clone());
    let field_names_func = quote! {
        fn field_names() -> Vec<&'static str> {
            vec![
                #(#field_names),*
            ]
        }
    };

    quote! {
        impl<'a> ::structqlx::traits::DatabaseModel<'a, #id_type> for #ident {
            type Database = #db_engine;

            #schema

            #table_name_fn

            #field_names_func

            /// Return a model by its id.
            async fn fetch_one(executor: impl sqlx::Executor<'a, Database = Self::Database>, id: &#id_type) -> sqlx::Result<#ident> {
                let query = format!("SELECT * FROM {} WHERE id = $1", Self::table_name());
                sqlx::query_as::<#db_engine, #ident>(&query)
                    .bind(id)
                    .fetch_one(executor)
                    .await
            }

            /// Return one or more rows matching `ids`.
            async fn fetch_many(executor: impl sqlx::Executor<'a, Database = Self::Database>, ids: &[#id_type]) -> sqlx::Result<Vec<#ident>> {
                let mut builder = sqlx::QueryBuilder::<#db_engine>::new(format!("SELECT * FROM {} WHERE id IN (", Self::table_name()));
                let mut separated = builder.separated(", ");
                for id in ids {
                    separated.push_bind(id);
                }
                separated.push_unseparated(") ");

                let query = builder.build_query_as::<#ident>();
                query.fetch_all(executor).await
            }

            /// Return all records in the table.
            async fn fetch_all(executor: impl sqlx::Executor<'a, Database = Self::Database>) -> sqlx::Result<Vec<#ident>> {
                let query = format!("SELECT * FROM {}", Self::table_name());
                sqlx::query_as::<#db_engine, #ident>(&query)
                    .fetch_all(executor)
                    .await
            }

            /// Delete one or more records from the table.
            async fn delete_many(executor: impl sqlx::Executor<'a, Database = Self::Database>, ids: &[#id_type]) -> sqlx::Result<()> {
                let mut builder = sqlx::QueryBuilder::<#db_engine>::new(format!("DELETE FROM {} WHERE id IN (", Self::table_name()));
                let mut separated = builder.separated(", ");
                for id in ids {
                    separated.push_bind(id);
                }
                separated.push_unseparated(") ");

                let query = builder.build();
                query.execute(executor).await.map(|_| ())
            }

            /// Delete exactly one record -- the current one.
            async fn delete(&self, executor: impl sqlx::Executor<'a, Database = Self::Database>) -> sqlx::Result<()> {
                let query = format!("DELETE FROM {} WHERE id = $1", Self::table_name());
                sqlx::query::<#db_engine>(&query)
                    .bind(&self.id)
                    .execute(executor)
                    .await?;
                Ok(())
            }

            /// Return the count of rows in the table.
            async fn count(executor: impl sqlx::Executor<'a, Database = Self::Database>) -> sqlx::Result<i64> {
                use sqlx::Row;
                let query = format!("SELECT COUNT(*) FROM {}", Self::table_name());
                let row = sqlx::query(&query)
                    .fetch_one(executor)
                    .await?;
                row.try_get(0)
            }

            #update_method

            #create_method

            #save_method
        }

        impl #ident {
            #constructor
            #getters

            #find_by_funcs
        }
    }
    .into()
}

pub(crate) fn get_id_field_type(model_struct: DataStruct) -> proc_macro2::TokenStream {
    let id_field = get_id_field_from_model(model_struct);
    let id_field_type = id_field.ty.clone();
    if !is_option(&id_field_type) {
        return quote!(#id_field_type);
    }
    match get_field_type_from_option(&id_field_type) {
        Some(ty_) => quote! {#ty_},
        None => quote!(#id_field_type),
    }
}

pub(crate) fn generate_create_method(
    model_struct: DataStruct,
    db_engine: &syn::Path,
) -> proc_macro2::TokenStream {
    let field_names = get_field_names(model_struct.clone());
    let field_names_clause = field_names.join(", ");
    let values_clause = field_names
        .iter()
        .enumerate()
        .map(|r| format!("${}", r.0 + 1))
        .collect::<Vec<String>>()
        .join(", ");

    let query = format!(
        "INSERT INTO {{table_name}} ({}) VALUES ({}) RETURNING *;",
        field_names_clause, values_clause
    )
    .to_token_stream();

    let bind_exprs = field_names
        .iter()
        .map(|f| {
            let assignment = format!("query = query.bind(self.{}.clone())", f);
            syn::parse_str::<syn::Expr>(&assignment)
                .unwrap_or_else(|_| panic!("Unable to parse assignment {}", assignment))
        })
        .collect::<Vec<syn::Expr>>();

    quote! {
        /// Create a new record from the current data.
        async fn create(&mut self, executor: impl sqlx::Executor<'a, Database = Self::Database>) -> sqlx::Result<Self> {
            let table_name = Self::table_name();
            let query = format!(#query);
            let mut query = sqlx::query_as::<#db_engine, Self>(&query);
            #(#bind_exprs);*
            ;
            let result = query.fetch_one(executor).await;
            if let Ok(ref result) = result {
                let _ = std::mem::replace(self, result.clone());
            }
            result
        }
    }
}

/// Build and return an update() method for a writeable struct.
pub(crate) fn generate_update_method(
    model_struct: DataStruct,
    db_engine: &syn::Path,
) -> proc_macro2::TokenStream {
    let field_names = get_field_names(model_struct.clone());
    let set_clause = field_names
        .iter()
        .enumerate()
        .map(|(i, f)| format!("{} = ${}", f, i + 1))
        .collect::<Vec<String>>()
        .join(", ");
    let id_clause = format!("${}", field_names.len() + 1);
    let query = format!("UPDATE {{table_name}} SET {set_clause} WHERE id = {id_clause};");

    let bind_exprs = field_names
        .iter()
        .map(|f| {
            let assignment = format!("query = query.bind(self.{}.clone())", f);
            syn::parse_str::<syn::Expr>(&assignment)
                .unwrap_or_else(|_| panic!("Unable to parse assignment {}", assignment))
        })
        .collect::<Vec<syn::Expr>>();
    quote! {
        /// Update an existing record with the current data.
        async fn update(&mut self, executor: impl sqlx::Executor<'a, Database = Self::Database>) -> sqlx::Result<()> {
            let table_name = Self::table_name();
            let query = format!(#query);
            let mut query = sqlx::query_as::<#db_engine, Self>(&query);
            #(#bind_exprs);*
            ;
            query = query.bind(self.id.clone());
            let result = query.fetch_one(executor).await;
            match result {
                Ok(result) => {
                    let _ = std::mem::replace(self, result);
                    Ok(())
                },
                Err(err) => Err(err),
            }
        }
    }
}

pub(crate) fn generate_save_method(
    model_struct: DataStruct,
    db_engine: &syn::Path,
) -> proc_macro2::TokenStream {
    if is_option(&get_id_field_from_model(model_struct).ty) {
        quote! {
            /// Save t
            ///
            /// If the record has no ID, create a new record. Otherwise, update
            /// the existing one.
            async fn save(&mut self, executor: impl sqlx::Executor<'a, Database = Self::Database>) -> sqlx::Result<()> {
                if self.id.is_none() {
                    self.create(executor).await.map(|_| ())
                } else {
                    self.update(executor).await
                }
            }
        }
    } else {
        quote! {
            /// Save the record.
            ///
            /// If the record exists in the database, update it. Otherwise, create it.
            async fn save(&mut self, executor: impl sqlx::Executor<'a, Database = Self::Database> + Copy) -> sqlx::Result<()> {
                use sqlx::Row;
                let query = format!("SELECT 1 AS row_exists FROM {} WHERE id = $1;", Self::table_name());
                let exists = sqlx::query::<#db_engine>(&query)
                    .bind(&self.id)
                    .fetch_one(executor)
                    .await
                    .is_ok();
                if exists {
                    self.update(executor).await.map(|_| ())
                } else {
                    self.create(executor).await.map(|_| ())
                }
            }
        }
    }
}

fn field_type_is_string(field_type: &Type) -> bool {
    if let Type::Path(path) = field_type {
        let Some(type_ident) =
            path.path
                .get_ident()
                .or(path.path.segments.last().map(|seg| &seg.ident))
        else {
            return false;
        };

        return *type_ident == "String";
    }
    false
}

/// Generate find_by_<X> methods.
pub(crate) fn generate_find_by_funcs(
    fields: &Fields,
    table_name: &str,
    db_engine: &syn::Path,
    ident: &syn::Ident,
) -> proc_macro2::TokenStream {
    let find_by_funcs = fields
        .iter()
        .filter_map(|field| {
            let field_ident = field.ident.as_ref()?;
            if field_ident.to_string() == "id" {
                return None;
            }

            let ty = &field.ty;
            let func_ident = Ident::new(&format!("find_by_{}", field_ident), Span::call_site());
            let query = format!(
                "SELECT * FROM {table_name} WHERE {field_name} = $1",
                field_name = field_ident.to_string()
            );
            let func = quote! {
                async fn #func_ident<'a>(executor: impl sqlx::Executor<'a, Database = #db_engine>, #field_ident: #ty) -> Result<Vec<Self>, sqlx::Error> {
                    let query = #query;
                    sqlx::query_as::<#db_engine, #ident>(&query)
                        .bind(#field_ident)
                        .fetch_all(executor)
                        .await
                }
            };
            Some(func)
        })
        .collect();
    find_by_funcs
}

/// Generate a geetter function for each field on a model.
pub(crate) fn generate_getters(model_struct: &DataStruct) -> proc_macro2::TokenStream {
    let named_fields = match model_struct.fields {
        Fields::Named(ref fields) => fields.clone(),
        _ => panic!("Can't handle unnamed fields"),
    };

    let getters = named_fields
        .named
        .iter()
        .map(|field| {
            let field_name = field.ident.clone().unwrap();
            let return_type = if field_type_is_string(&field.ty) {
                quote! {&str}
            } else {
                let ty = field.ty.clone();
                quote! {&#ty}
            };
            let comment =
                format!("Return the current record's `{}`.", field_name).to_token_stream();
            quote! {
                #[doc = #comment]
                pub fn #field_name(&self) -> #return_type {
                    &self.#field_name
                }
            }
        })
        .collect::<Vec<proc_macro2::TokenStream>>();

    quote! {
        #(#getters)*
    }
}

pub(crate) fn get_id_field_from_model(model_struct: DataStruct) -> Field {
    let fields = model_struct.fields;
    match fields {
        Fields::Named(fields) => {
            let mut fields_by_name: HashMap<String, &Field> = HashMap::new();

            for field in fields.named.iter() {
                if let Some(ident) = field.ident.clone() {
                    fields_by_name.insert(ident.to_string(), field);
                }
            }

            let id_field = match fields_by_name.get("id") {
                Some(field) => *field,
                None => panic!("Struct does not have an id field."),
            };
            id_field.clone()
        }
        _ => panic!("Can't handle unnamed fields"),
    }
}

/// Return all the field names for a struct.
///
/// Don't return `id`, as it's a required field and this list is used to
/// construct queries which may mutate its value if we aren't careful.
pub(crate) fn get_field_names(model_struct: DataStruct) -> Vec<String> {
    let named_fields = match model_struct.fields {
        Fields::Named(ref fields) => fields,
        _ => panic!("Can't handle unnamed fields"),
    };

    let id_field_type = get_id_field_from_model(model_struct.clone()).ty.clone();
    let mut field_names: Vec<String> = vec![];
    for field in named_fields.named.iter() {
        if let Some(ident) = field.ident.clone() {
            let field_name = ident.to_string();
            if ["created_at", "updated_at", "deleted_at"].contains(&field_name.as_str()) {
                continue;
            }
            // Don't try to write ID fields if they're options.
            if is_option(&id_field_type) && field_name == "id" {
                continue;
            }
            field_names.push(field_name);
        }
    }

    field_names
}

fn generate_constructor(ident: &syn::Ident, model_struct: &DataStruct) -> proc_macro2::TokenStream {
    let comment = format!("Create and return a new {} instance.", ident);
    let Fields::Named(fields) = model_struct.fields.clone() else {
        panic!("Cannot build a constructor from unnamed fields",);
    };

    let mut params: Punctuated<proc_macro2::TokenStream, Token![,]> = Punctuated::new();
    let mut assignments: Punctuated<syn::Ident, Token![,]> = Punctuated::new();

    for field in fields.named.iter() {
        let ident = field.ident.as_ref().unwrap();
        let ty = field.ty.clone();
        params.push(quote! {#ident: #ty});
        assignments.push(ident.clone());
    }

    quote! {
        #[doc = #comment]
        pub fn new(#params) -> Self {
            Self {
                #assignments
            }
        }
    }
}
