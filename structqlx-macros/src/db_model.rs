use std::{collections::HashMap, ops::Deref};

use crate::utils::is_option;
use convert_case::{Case, Casing};
use darling::{FromAttributes, FromDeriveInput, FromMeta};
use proc_macro::TokenStream;
use proc_macro2::{Punct, Span};
use quote::{ToTokens, quote};
use syn::{
    Data, DataStruct, DeriveInput, Field, Fields, Ident, ItemImpl, ItemStruct, Token, Type,
    parse_macro_input, parse_quote, punctuated::Punctuated,
};

#[derive(darling::FromDeriveInput, Debug, darling::FromAttributes)]
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

struct DatabaseModelField<'a> {
    field: &'a Field,
    attrs: FieldAttributes,
}

impl<'a> Deref for DatabaseModelField<'a> {
    type Target = &'a Field;

    fn deref(&self) -> &Self::Target {
        &self.field
    }
}

impl<'a> ToTokens for DatabaseModelField<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.field.to_tokens(tokens);
    }
}

impl<'a> TryFrom<&'a Field> for DatabaseModelField<'a> {
    type Error = String;

    fn try_from(field: &'a Field) -> Result<Self, Self::Error> {
        let attrs = FieldAttributes::from_attributes(&field.attrs)
            .map_err(|e| format!("Unable to parse field attributes: {e:?}"))?;
        Ok(Self { field, attrs })
    }
}

pub(crate) struct DatabaseModelStruct<'a> {
    struct_: &'a ItemStruct,
    fields: Punctuated<DatabaseModelField<'a>, Token![,]>,
    options: CrudModelOptions,
}

impl<'a> Deref for DatabaseModelStruct<'a> {
    type Target = ItemStruct;

    fn deref(&self) -> &Self::Target {
        &self.struct_
    }
}

impl<'a> TryFrom<&'a ItemStruct> for DatabaseModelStruct<'a> {
    type Error = String;

    fn try_from(struct_: &'a ItemStruct) -> Result<Self, Self::Error> {
        let fields: Punctuated<DatabaseModelField, Token![,]> = struct_
            .fields
            .iter()
            .map(DatabaseModelField::try_from)
            .collect::<Result<Punctuated<DatabaseModelField, Token![,]>, String>>()?;
        let options = CrudModelOptions::from_attributes(&struct_.attrs)
            .map_err(|e| format!("Unable to parse struct attributes {e:?}"))?;
        Ok(Self {
            struct_,
            fields,
            options,
        })
    }
}

impl<'a> DatabaseModelStruct<'a> {
    /// Return the table name string.
    fn table_name(&self) -> String {
        self.options
            .table_name
            .clone()
            .unwrap_or(self.struct_.ident.to_string())
            .to_case(Case::Snake)
    }

    /// Return a struct representing a new version of this struct, with an optional PK
    /// field.
    fn new_row_struct(&self) -> ItemStruct {
        let fields = self
            .fields
            .iter()
            .map(|field| {
                if let Some(true) = field.attrs.primary_key {
                    let Field { ident, vis, ty, .. } = field.field;
                    return parse_quote!(#vis #ident: Option<#ty>);
                }
                field.field.clone()
            })
            .collect::<Punctuated<Field, Token![,]>>();

        let ItemStruct {
            vis,
            struct_token,
            ident,
            ..
        } = self.struct_;
        let ident = Ident::new(&format!("New{ident}"), Span::call_site());
        parse_quote!(
            #[derive(Debug, Clone)]
            #vis #struct_token #ident {
                #fields
            }
        )
    }

    /// Return the new row struct impl, which allows the new row struct to be saved.
    fn new_row_struct_impl(&self) -> ItemImpl {
        let ItemStruct {
            attrs,
            vis,
            struct_token,
            ident,
            generics,
            fields,
            semi_token,
        } = self.struct_;

        let db_engine = &self.options.engine;
        let doc = format!("Save a new {ident} to the database, and return the saved instance");
        let new_type_ident = Ident::new(&format!("New{ident}"), Span::call_site());
        let query = self.insert_query();

        let mut insert_query = quote! {
            let query = sqlx::query_as::<Self::Database, #ident>(#query)
        };
        for bind_call in self.fields.iter().map(|f| {
            let ident = f.ident.as_ref();
            quote! {.bind(&self.#ident)}.to_token_stream()
        }) {
            insert_query.extend(bind_call);
        }

        parse_quote!(
            impl<'a> #new_type_ident<'a> {
                type Database = #db_engine;

                #[doc = #doc]
                async fn save(self, executor: impl sqlx::Executor<'a, Database = Self::Database>) -> sqlx::Result<#ident> {
                    #insert_query;
                    query.execute(&executor).await?.fetch_one().await?
                }
            }
        )
    }

    /// Return the INSERT query for this struct.
    fn insert_query(&self) -> String {
        let mut pk_field_name: Option<String> = None;
        let field_names = self
            .fields
            .iter()
            .filter_map(|field| {
                if let Some(true) = field.attrs.primary_key {
                    pk_field_name = field.ident.as_ref().map(|i| i.to_string());
                }
                field.ident.as_ref().map(|f| f.to_string())
            })
            .collect::<Vec<String>>()
            .join(", ");

        if pk_field_name.is_none() {
            panic!(
                "No primary key found for {} when generating INSERT query",
                self.ident
            );
        }

        format!(
            "INSERT INTO {table_name} ({field_names}) VALUES ({params}) RETURNING *",
            table_name = self.table_name(),
            params = (0..self.fields.len())
                .map(|i| format!("${}", i + 1))
                .collect::<Vec<String>>()
                .join(", ")
        )
    }
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

    let field_names = fields_by_name.keys().cloned().collect::<Vec<String>>();

    let delete_query_base = format!("DELETE FROM {table_name}");
    let select_query_base = format!("SELECT FROM {table_name}");
    let update_query_base = format!("UPDATE {table_name}");
    let insert_query_base = format!(
        "INSERT INTO {table_name} ({field_names}) VALUES",
        field_names = field_names.join(", ")
    );

    let query_builder_method: syn::ImplItemFn = parse_quote!(
        /// Return a [sqlx::QueryBuilder] initialised with the correct SQL for the
        /// passed QueryVerb.
        fn query_builder<D: sqlx::Database>(
            query_verb: structqlx::QueryVerb,
        ) -> sqlx::QueryBuilder<D> {
            let query = match query_verb {
                structqlx::QueryVerb::Delete => #delete_query_base,
                structqlx::QueryVerb::Select => #select_query_base,
                structqlx::QueryVerb::Update => #update_query_base,
                structqlx::QueryVerb::Insert => #insert_query_base,
            };
            sqlx::QueryBuilder::new(query)
        }
    );

    let count_query = format!("SELECT COUNT(*) FROM {table_name}");
    let count_method: syn::ImplItemFn = parse_quote!(
        /// Return the count of rows in the table.
        async fn count(
            executor: impl sqlx::Executor<'a, Database = Self::Database>,
        ) -> sqlx::Result<i64> {
            use sqlx::Row;
            let row = sqlx::query(#count_query).fetch_one(executor).await?;
            row.try_get(0)
        }
    );

    let create_query = format!(
        "{insert_query_base} ({params}) RETURNING {pk_field_ident}",
        params = (0..field_names.len())
            .map(|i| format!("${}", i + 1))
            .collect::<Vec<String>>()
            .join(", ")
    );
    let mut create_query_stream = quote! {
        let mut query = sqlx::query_as::<#db_engine, Self>(#create_query)
    }
    .to_token_stream();
    for bind_stream in field_names.iter().map(|f| {
        let ident = Ident::new(f, Span::call_site());
        quote! {.bind(&self.#ident)}.to_token_stream()
    }) {
        create_query_stream.extend(bind_stream);
    }

    let insert_method: syn::ImplItem = parse_quote!(
        /// Insert a new record from the current data.
        async fn insert(&mut self, executor: impl sqlx::Executor<'a, Database = Self::Database>) -> sqlx::Result<Self> {
            #create_query_stream;
            let result = query.fetch_one(executor).await;
            if let Ok(ref result) = result {
                let _ = std::mem::replace(self, result.clone());
            }
            result
        }
    );

    let update_method: syn::ImplItem = {
        let query = format!(
            "{update_query_base} SET {set} WHERE {pk_field_ident} = ${pk_param}",
            set = field_names
                .iter()
                .filter(|f| **f != pk_field_name)
                .enumerate()
                .map(|(i, f)| { format!("{f} = ${i}", i = i + 1) })
                .fold(String::new(), |prev, new| format!("{prev}, {new}")),
            pk_param = field_names.len() + 1
        );
        let query: syn::Expr = parse_quote!(
            sqlx::query_as::<#db_engine, #ident>(#query)
        );
        let mut query = query.to_token_stream();
        for bind_stream in field_names.iter().filter_map(|f| {
            let ident = Ident::new(f, Span::call_site());
            // Don't include the pk field in the list of fields to set.
            if ident == **pk_field_ident {
                return None;
            }
            Some(quote! {.bind(&self.#ident)}.to_token_stream())
        }) {
            bind_stream.to_tokens(&mut query)
        }
        quote!(.bind(&self.#pk_field_ident)).to_tokens(&mut query);

        parse_quote!(
            /// Update an existing record with the current data.
            async fn update(
                &mut self,
                executor: impl sqlx::Executor<'a, Database = Self::Database> + Copy,
            ) -> sqlx::Result<()> {
                #query;
            }
        )
    };

    let field_names = field_names.join(", ");
    let fetch_one_query =
        format!("SELECT {field_names} FROM {table_name} WHERE {pk_field_name} = $1");
    let fetch_all_query: String = format!("SELECT {field_names} FROM {table_name}");
    let delete_one_query = format!("DELETE FROM {table_name} WHERE {pk_field_name} = $1");

    let impl_trait: ItemImpl = parse_quote!(
        impl<'a> ::structqlx::traits::DatabaseModel<'a, #pk_type> for #ident {
            type Database = #db_engine;

            /// Return a model by its id.
            async fn fetch_one(executor: impl sqlx::Executor<'a, Database = Self::Database>, #pk_field_ident: &#pk_type) -> sqlx::Result<#ident> {
                sqlx::query_as::<#db_engine, #ident>(#fetch_one_query)
                    .bind(#pk_field_ident)
                    .fetch_one(executor)
                    .await
            }

            /// Return one or more rows matching `ids`.
            async fn fetch_many(executor: impl sqlx::Executor<'a, Database = Self::Database>, ids: &[#pk_type]) -> sqlx::Result<Vec<#ident>> {
                let mut builder = Self::query_builder(structqlx::QueryVerb::Select);
                let mut separated = builder.separated(", ");
                for id in ids {
                    separated.push_bind(id);
                }
                separated.push_unseparated(") ");

                let query = builder.build_query_as::<Self>();
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
                let mut builder = Self::query_builder(structqlx::QueryVerb::Delete).push("(");
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

            #count_method

            #query_builder_method

            #update_method

            #save_method

            #insert_method

        }
    );

    let impl_type: syn::ItemImpl = parse_quote!(
        impl #ident {
            #constructor

            #getters

            #find_by_funcs
        }
    );

    let mut token_stream = impl_trait.to_token_stream();
    impl_type.to_tokens(&mut token_stream);
    query_opts_struct.to_tokens(&mut token_stream);
    token_stream.into()
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
            /// Save the current record.
            ///
            /// If the record has no ID, create a new record. Otherwise, update
            /// the existing one.
            async fn save(&mut self, executor: impl sqlx::Executor<'a, Database = Self::Database> + Copy) -> sqlx::Result<()> {
                if self.#pk_field_ident.is_none() {
                    self.insert(executor).await.map(|_| ())
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
                    self.insert(executor).await.map(|_| ())
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
            let find_func = quote! {
                async fn #func_ident<'a>(executor: impl sqlx::Executor<'a, Database = #db_engine>, #field_ident: #ty) -> Result<Vec<Self>, sqlx::Error> {
                    let query = #query;
                    sqlx::query_as::<#db_engine, #ident>(&query)
                        .bind(#field_ident)
                        .fetch_all(executor)
                        .await
                }
            };

            let func_ident = Ident::new(&format!("find_one_by_{}", field_ident), Span::call_site());
            let find_one_func = quote! {
                async fn #func_ident<'a>(executor: impl sqlx::Executor<'a, Database = #db_engine>, #field_ident: #ty) -> Result<Self, sqlx::Error> {
                    let query = #query;
                    sqlx::query_as::<#db_engine, #ident>(&query)
                        .bind(#field_ident)
                        .fetch_one(executor)
                        .await
                }
            };

            let func_ident = Ident::new(&format!("find_optional_one_by_{}", field_ident), Span::call_site());
            let find_one_optional_func = quote! {
                async fn #func_ident<'a>(executor: impl sqlx::Executor<'a, Database = #db_engine>, #field_ident: #ty) -> Result<Option<Self>, sqlx::Error> {
                    let query = #query;
                    sqlx::query_as::<#db_engine, #ident>(&query)
                        .bind(#field_ident)
                        .fetch_optional(executor)
                        .await
                }
            };
            Some(quote!{
                #find_func

                #find_one_func

                #find_one_optional_func
            })
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

#[cfg(test)]
mod test {
    use prettyplease::unparse;
    use quote::ToTokens;
    use syn::{File, ItemImpl};
    use syn::{ItemStruct, parse_quote};

    use crate::db_model::expand_db_model;

    use crate::db_model::DatabaseModelStruct;

    // #[test]
    fn expand_db_model_expands() {
        let struct_: ItemStruct = parse_quote!(
            #[derive(Clone, Debug, DatabaseModel)]
            struct MyStuct {
                id: u32,
                msg: String,
            }
        );

        let _expanded = expand_db_model(struct_.to_token_stream().into());
    }

    #[test]
    fn database_model_struct_new_row_struct() {
        let struct_: ItemStruct = parse_quote!(
            #[structqlx(engine = ::sqlx::Postgres)]
            struct MyDatabaseModel {
                #[structqlx(primary_key = true)]
                id: u32,
                name: String,
            }
        );

        let model_struct_ = DatabaseModelStruct::try_from(&struct_).unwrap();
        let new_row_struct = model_struct_.new_row_struct();
        let actual: File = parse_quote!(
            #new_row_struct
        );
        let expected: File = parse_quote!(
            #[derive(Debug, Clone)]
            struct NewMyDatabaseModel {
                id: Option<u32>,
                name: String,
            }
        );

        assert_eq!(unparse(&expected), unparse(&actual))
    }

    #[test]
    fn new_row_struct_impl() {
        let struct_: ItemStruct = parse_quote!(
            #[structqlx(engine = ::sqlx::Postgres)]
            struct MyDatabaseModel {
                #[structqlx(primary_key = true)]
                id: u32,
                name: String,
            }
        );

        let model_struct_ = DatabaseModelStruct::try_from(&struct_).unwrap();
        let expected: ItemImpl = parse_quote!(
            impl<'a> NewMyDatabaseModel<'a> {
                type Database = ::sqlx::Postgres;

                ///Save a new MyDatabaseModel to the database, and return the saved instance
                async fn save(
                    self,
                    executor: impl sqlx::Executor<'a, Database = Self::Database>,
                ) -> sqlx::Result<MyDatabaseModel> {
                    let query = sqlx::query_as::<Self::Database, MyDatabaseModel>(
                        "INSERT INTO my_database_model (id, name) VALUES ($1, $2) RETURNING *",
                    )
                    .bind(&self.id)
                    .bind(&self.name);
                    query.execute(&executor).await?.fetch_one().await?
                }
            }
        );
        let actual = model_struct_.new_row_struct_impl();
        let expected: File = parse_quote!(#expected);
        let actual: File = parse_quote!(#actual);
        assert_eq!(unparse(&expected), unparse(&actual));
    }
}
