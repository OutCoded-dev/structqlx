use std::collections::HashMap;

use crate::utils::is_option;
use convert_case::{Case, Casing};
use darling::{FromAttributes, FromDeriveInput, FromMeta};
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{ToTokens, quote};
use syn::{
    Data, DataStruct, DeriveInput, Field, Fields, Ident, Token, Type, parse_macro_input,
    parse_quote, punctuated::Punctuated,
};

#[derive(darling::FromDeriveInput, Debug)]
#[darling(attributes(structqlx))]
struct CrudModelOptions {
    table_name: Option<String>,
    #[cfg(feature = "postgres")]
    schema: Option<String>,
    generate_getters: Option<bool>,
    constructor: Option<bool>,
    engine: syn::Path,
}

#[derive(darling::FromAttributes, Debug)]
#[darling(attributes(structqlx), forward_attrs)]
pub(crate) struct FieldAttributes {
    primary_key: Option<bool>,
}

pub fn expand_db_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ident = input.ident.clone();

    let opts = match CrudModelOptions::from_derive_input(&input) {
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

    let mut fields_by_name: HashMap<String, &Field> = HashMap::new();

    let mut pk_fields: Vec<&Field> = vec![];
    for field in model_struct.fields.iter() {
        let Some(ident) = field.ident.clone() else {
            continue;
        };
        let attrs = FieldAttributes::from_attributes(&field.attrs).unwrap();
        if attrs.primary_key == Some(true) {
            pk_fields.push(field);
        }
        fields_by_name.insert(ident.to_string(), field);
    }

    if pk_fields.is_empty() {
        panic!("Struct has no primary key field");
    }

    let pk_field = *pk_fields.first().unwrap();
    let pk_field_name = pk_field.ident.as_ref().unwrap().to_string();
    let field_names = fields_by_name.keys().cloned().collect::<Vec<String>>();

    let pk_type = &pk_field.ty;
    let pk_field_ident = &pk_field.ident.as_ref().unwrap(); // Safe, since we enforce named fields.

    let create_method = generate_create_method(&opts.engine, &field_names, pk_field);
    let update_method = generate_update_method(&opts.engine, pk_field, &field_names);
    let save_method = generate_save_method(model_struct.clone(), &opts.engine, pk_field);
    let query_opts_struct = generate_query_options(&ident, &model_struct.fields);

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

    let field_names = fields_by_name.keys();
    let field_names_func = quote! {
        fn field_names() -> Vec<&'static str> {
            vec![
                #(#field_names),*
            ]
        }
    };

    let fetch_one_query = format!("SELECT * FROM {table_name} WHERE {pk_field_name} = $1");
    let fetch_many_query: String = format!("SELECT * FROM {table_name} WHERE {pk_field_name} IN (");
    let fetch_all_query: String = format!("SELECT * FROM {table_name}");
    let delete_one_query = format!("DELETE FROM {table_name} WHERE {pk_field_name} = $1");
    let delete_many_query: String = format!("DELETE FROM {table_name} WHERE {pk_field_name} IN (");

    quote! {
        impl<'a> ::structqlx::traits::DatabaseModel<'a, #pk_type> for #ident {
            type Database = #db_engine;

            #schema

            #table_name_fn

            #field_names_func

            /// Return a model by its id.
            async fn fetch_one(executor: impl sqlx::Executor<'a, Database = Self::Database>, #pk_field_ident: &#pk_type) -> sqlx::Result<#ident> {
                let query = #fetch_one_query;
                sqlx::query_as::<#db_engine, #ident>(&query)
                    .bind(#pk_field_ident)
                    .fetch_one(executor)
                    .await
            }

            /// Return one or more rows matching `ids`.
            async fn fetch_many(executor: impl sqlx::Executor<'a, Database = Self::Database>, ids: &[#pk_type]) -> sqlx::Result<Vec<#ident>> {
                let mut builder = sqlx::QueryBuilder::<#db_engine>::new(#fetch_many_query);
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
                let query = #fetch_all_query;
                sqlx::query_as::<#db_engine, #ident>(&query)
                    .fetch_all(executor)
                    .await
            }

            /// Delete one or more records from the table.
            async fn delete_many(executor: impl sqlx::Executor<'a, Database = Self::Database>, ids: &[#pk_type]) -> sqlx::Result<()> {
                let mut builder = sqlx::QueryBuilder::<#db_engine>::new(#delete_many_query);
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
                let query = format!(#delete_one_query);
                sqlx::query::<#db_engine>(&query)
                    .bind(&self.#pk_field_ident)
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

        #query_opts_struct
    }
    .into()
}

pub(crate) fn generate_create_method(
    db_engine: &syn::Path,
    field_names: &[String],
    pk_field: &Field,
) -> proc_macro2::TokenStream {
    let field_names = field_names
        .iter()
        .filter_map(|f| {
            if is_option(&pk_field.ty) && *f == pk_field.ident.as_ref().unwrap().to_string() {
                None
            } else {
                Some(f.clone())
            }
        })
        .collect::<Vec<String>>();
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
    db_engine: &syn::Path,
    pk_field: &Field,
    field_names: &[String],
) -> proc_macro2::TokenStream {
    let pk_field_ident = pk_field.ident.as_ref().unwrap();
    let pk_field_name = pk_field_ident.to_string();
    let field_names = field_names
        .iter()
        .filter_map(|fieldname| {
            if *fieldname != pk_field_name {
                Some(fieldname.clone())
            } else {
                None
            }
        })
        .collect::<Vec<String>>();
    let set_clause = field_names
        .iter()
        .enumerate()
        .map(|(i, f)| format!("{} = ${}", f, i + 1))
        .collect::<Vec<String>>()
        .join(", ");
    let id_clause = format!("${}", field_names.len() + 1);
    let query = format!(
        "UPDATE {{table_name}} SET {set_clause} WHERE {pk_field_name} = {id_clause} RETURNING *;",
    );

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
        async fn update(&mut self, executor: impl sqlx::Executor<'a, Database = Self::Database> + Copy) -> sqlx::Result<()> {
            let table_name = Self::table_name();
            let query_sql = format!(#query);
            let mut query = sqlx::query_as::<#db_engine, Self>(&query_sql);
            #(#bind_exprs);*;
            query = query.bind(self.#pk_field_ident.clone());
            let result = query.fetch_one(executor).await?;
            let _ = std::mem::replace(self, result);
            Ok(())
        }
    }
}

pub(crate) fn generate_save_method(
    _model_struct: DataStruct,
    db_engine: &syn::Path,
    pk_field: &Field,
) -> proc_macro2::TokenStream {
    let pk_field_ident = pk_field.ident.as_ref().unwrap();
    if is_option(&pk_field.ty) {
        quote! {
            /// Save t
            ///
            /// If the record has no ID, create a new record. Otherwise, update
            /// the existing one.
            async fn save(&mut self, executor: impl sqlx::Executor<'a, Database = Self::Database> + Copy) -> sqlx::Result<()> {
                if self.#pk_field_ident.is_none() {
                    self.create(executor).await.map(|_| ())
                } else {
                    self.update(executor).await
                }
            }
        }
    } else {
        let pk_field_name = pk_field.ident.as_ref().unwrap().to_string();
        quote! {
            /// Save the record.
            ///
            /// If the record exists in the database, update it. Otherwise, create it.
            async fn save(&mut self, executor: impl sqlx::Executor<'a, Database = Self::Database> + Copy) -> sqlx::Result<()> {
                use sqlx::Row;
                let query = format!("SELECT true AS row_exists FROM {} WHERE {} = $1;", Self::table_name(), #pk_field_name);
                let result = sqlx::query::<#db_engine>(&query)
                    .bind(&self.#pk_field_ident)
                    .fetch_one(executor)
                    .await;
                if result.is_ok() {
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
    fields
        .iter()
        .filter_map(|field| {
            let field_ident = field.ident.as_ref()?;
            if *field_ident == "id" {
                return None;
            }

            let ty = &field.ty;
            let func_ident = Ident::new(&format!("find_by_{}", field_ident), Span::call_site());
            let query = format!(
                "SELECT * FROM {table_name} WHERE {field_name} = $1",
                field_name = field_ident
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
        .collect()
}

/// Generate a query options struct.
pub(crate) fn generate_query_options(ident: &Ident, fields: &Fields) -> proc_macro2::TokenStream {
    let query_opts_struct_ident = Ident::from_string(&format!("{ident}QueryOptions")).unwrap();
    let query_opts_fields = fields
        .iter()
        .map(|field| {
            let ty = &field.ty;
            let ty: Type = if is_option(ty) {
                ty.clone()
            } else {
                parse_quote!(Option<#ty>)
            };

            let mut qo_field = field.clone();
            qo_field.attrs.clear();
            qo_field.ty = ty;

            qo_field
        })
        .collect::<Vec<Field>>();

    quote! {
        #[derive(Clone, Debug, Default, PartialEq)]
        pub struct #query_opts_struct_ident {
            #(#query_opts_fields),*
        }
    }
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
