use std::collections::HashMap;

use crate::CrudModelOptions;
use convert_case::{Case, Casing};
use darling::{Error, FromMeta, ast::NestedMeta};
use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use syn::{Data, DataStruct, DeriveInput, Field, Fields, parse_macro_input};

pub fn expand_db_model(attr: TokenStream, input: TokenStream) -> TokenStream {
    let attr_args = match NestedMeta::parse_meta_list(attr.clone().into()) {
        Ok(v) => v,
        Err(e) => {
            return TokenStream::from(Error::from(e).write_errors());
        }
    };
    let args = CrudModelOptions::from_list(&attr_args).unwrap();

    let input = parse_macro_input!(input as DeriveInput);
    let ident = input.ident.clone();

    let table_name = args
        .table_name
        .unwrap_or_else(|| input.ident.to_string().to_case(Case::Snake))
        .to_token_stream();
    let schema = if let Some(schema) = args.schema {
        quote! {
            /// Return the schema for the table.
            #[inline]
            fn schema() -> &'static str {
                #schema
            }
        }
    } else {
        quote! {}
    };

    let db_type = syn::Path::from_string("sqlx::Sqlite").unwrap();

    let model_struct: DataStruct = match &input.data {
        Data::Struct(model_struct) => model_struct.clone(),
        _ => panic!("CrudModel can only be derived for structs."),
    };

    // ID related fields.
    let id_type = get_id_field_type(model_struct.clone());
    let create_method = generate_create_method(model_struct.clone(), &db_type);
    let update_method = generate_update_method(model_struct.clone(), &db_type);
    let save_method = generate_save_method(model_struct.clone(), &db_type);

    let getters = if args.generate_getters {
        generate_getters(&model_struct)
    } else {
        quote! {}
    };

    let table_name = quote! {
        /// Return the table name for the model.
        fn table_name() -> String {
            #table_name.to_string()
        }
    };

    quote! {
        use sqlx::Executor;

        #[derive(Clone, Debug, sqlx::FromRow, PartialEq)]
        #input

        impl<'a> DatabaseModel<'a, #id_type> for #ident {
            type Database = #db_type;

            #schema

            #table_name

            /// Return a model by its id.
            async fn fetch_one(executor: impl sqlx::Executor<'a, Database = Self::Database>, id: &#id_type) -> sqlx::Result<#ident> {
                let query = format!("SELECT * FROM {} WHERE id = $1", Self::table_name());
                sqlx::query_as::<#db_type, #ident>(&query)
                    .bind(id)
                    .fetch_one(executor)
                    .await
            }

            /// Return one or more rows matching `ids`.
            async fn fetch_many(executor: impl sqlx::Executor<'a, Database = Self::Database>, ids: &[#id_type]) -> sqlx::Result<Vec<#ident>> {
                let mut builder = sqlx::QueryBuilder::<#db_type>::new(format!("SELECT * FROM {} WHERE id IN (", Self::table_name()));
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
                sqlx::query_as::<#db_type, #ident>(&query)
                    .fetch_all(executor)
                    .await
            }

            /// Delete one or more records from the table.
            async fn delete_many(executor: impl sqlx::Executor<'a, Database = Self::Database>, ids: &[#id_type]) -> sqlx::Result<()> {
                let mut builder = sqlx::QueryBuilder::<#db_type>::new(format!("DELETE FROM {} WHERE id IN (", Self::table_name()));
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
                sqlx::query::<#db_type>(&query)
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
            #getters
        }
    }
    .into()
}

fn get_id_field_type(model_struct: DataStruct) -> proc_macro2::TokenStream {
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

/// Return true if a given type is an Option<>, false otherwise.
fn is_option(type_: &syn::Type) -> bool {
    let syn::Type::Path(type_) = type_ else {
        return false;
    };
    if type_.qself.is_some() {
        return false;
    }
    let type_path = &type_.path;
    if type_path.segments.is_empty() || type_path.segments.last().unwrap().ident != "Option" {
        return false;
    }

    if !(type_path.segments.len() == 1
        || (type_path.segments.len() == 3
            && ["core", "std"].contains(&type_path.segments[0].ident.to_string().as_str())
            && type_path.segments[1].ident == "option"))
    {
        return false;
    }

    true
}

/// Walk the path of a given type. If it's an Option<*> field, return the inner
/// type, otherwise return None.
fn get_field_type_from_option(type_: &syn::Type) -> Option<&syn::Type> {
    if !is_option(type_) {
        return None;
    }

    let syn::Type::Path(type_) = type_ else {
        return None;
    };
    let type_path = &type_.path;
    let last_segment = type_path.segments.last().unwrap();
    let syn::PathArguments::AngleBracketed(generics) = &last_segment.arguments else {
        return None;
    };
    if generics.args.len() != 1 {
        return None;
    }
    let syn::GenericArgument::Type(inner_type) = &generics.args[0] else {
        return None;
    };

    Some(inner_type)
}

/// Return all the field names for a struct.
///
/// Don't return `id`, as it's a required field and this list is used to
/// construct queries which may mutate its value if we aren't careful.
fn get_field_names(model_struct: DataStruct) -> Vec<String> {
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

fn generate_create_method(
    model_struct: DataStruct,
    db_type: &syn::Path,
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
            let mut query = sqlx::query_as::<#db_type, Self>(&query);
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
fn generate_update_method(
    model_struct: DataStruct,
    db_type: &syn::Path,
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
            let mut query = sqlx::query_as::<#db_type, Self>(&query);
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

fn generate_save_method(model_struct: DataStruct, db_type: &syn::Path) -> proc_macro2::TokenStream {
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
                let exists = sqlx::query::<#db_type>(&query)
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

/// Generate a geetter function for each field on a model.
fn generate_getters(model_struct: &DataStruct) -> proc_macro2::TokenStream {
    let named_fields = match model_struct.fields {
        Fields::Named(ref fields) => fields.clone(),
        _ => panic!("Can't handle unnamed fields"),
    };

    let getters = named_fields
        .named
        .iter()
        .map(|field| {
            let field_name = field.ident.clone().unwrap();
            let field_type = field.ty.clone();
            let comment =
                format!("Return the current record's `{}`.", field_name).to_token_stream();
            quote! {
                #[doc = #comment]
                pub fn #field_name(&self) -> &#field_type {
                    &self.#field_name
                }
            }
        })
        .collect::<Vec<proc_macro2::TokenStream>>();

    quote! {
        #(#getters)*
    }
}

fn get_id_field_from_model(model_struct: DataStruct) -> Field {
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
