use std::ops::Deref;

use convert_case::{Case, Casing};
use darling::FromAttributes;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{ToTokens, quote};
use syn::{
    Field, Ident, ImplItemFn, ItemImpl, ItemStruct, Token, parse_macro_input, parse_quote,
    punctuated::Punctuated,
};

#[derive(darling::FromDeriveInput, Debug, darling::FromAttributes)]
#[darling(attributes(structqlx), forward_attrs)]
struct DatabaseModelOptions {
    table_name: Option<String>,
    #[cfg(feature = "postgres")]
    schema: Option<String>,
    #[darling(default)]
    with_getters: bool,
    #[darling(default)]
    with_constructor: bool,
    engine: syn::Path,
}

#[derive(Clone, darling::FromAttributes, Debug)]
#[darling(attributes(structqlx), forward_attrs)]
pub(crate) struct FieldAttributes {
    #[darling(default)]
    primary_key: bool,
}

#[derive(Clone)]
struct DatabaseModelField<'a> {
    ident: &'a Ident,
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
        let Some(ident) = field.ident.as_ref() else {
            return Err(format!(
                "Field {field:?} has no ident; cannot operate on anonymous fields"
            ));
        };
        let attrs = FieldAttributes::from_attributes(&field.attrs)
            .map_err(|e| format!("Unable to parse field attributes: {e:?}"))?;
        Ok(Self {
            ident,
            field,
            attrs,
        })
    }
}

pub(crate) struct DatabaseModelStruct<'a> {
    struct_: &'a ItemStruct,
    fields: Punctuated<DatabaseModelField<'a>, Token![,]>,
    field_names: Vec<String>,
    pk_fields: Vec<DatabaseModelField<'a>>,
    pk_field_names: Vec<String>,
    options: DatabaseModelOptions,
}

impl<'a> Deref for DatabaseModelStruct<'a> {
    type Target = ItemStruct;

    fn deref(&self) -> &Self::Target {
        self.struct_
    }
}

impl<'a> TryFrom<&'a ItemStruct> for DatabaseModelStruct<'a> {
    type Error = String;

    fn try_from(struct_: &'a ItemStruct) -> Result<Self, Self::Error> {
        let mut fields: Punctuated<DatabaseModelField, Token![,]> = Punctuated::new();
        let mut field_names: Vec<String> = vec![];
        let mut pk_field_names: Vec<String> = vec![];
        let mut pk_fields: Vec<DatabaseModelField> = vec![];

        for field in struct_.fields.iter() {
            let model_field = DatabaseModelField::try_from(field)?;
            field_names.push(model_field.ident.to_string());
            if model_field.attrs.primary_key {
                pk_field_names.push(model_field.ident.to_string());
                pk_fields.push(model_field.clone());
            }
            fields.push(model_field);
        }

        let options = DatabaseModelOptions::from_attributes(&struct_.attrs)
            .map_err(|e| format!("Unable to parse struct attributes {e:?}"))?;
        Ok(Self {
            struct_,
            fields,
            field_names,
            pk_fields,
            pk_field_names,
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

    /// Return the PK field (singluar for now) for this DatabaseModel.
    fn primary_key_field(&self) -> &'a DatabaseModelField<'_> {
        if self.pk_fields.is_empty() {
            panic!("No pk fields found for database model struct.");
        };

        // Only the one for now.
        (self.pk_fields.first().unwrap()) as _
    }

    /// Return a struct representing a new version of this struct, with an optional PK
    /// field.
    fn new_row_struct(&self) -> ItemStruct {
        let fields = self
            .fields
            .iter()
            .map(|field| {
                if field.attrs.primary_key {
                    let Field { ident, vis, ty, .. } = field.field;
                    return parse_quote!(#vis #ident: Option<#ty>);
                }
                // Strip any attrs from the field to avoid confusing errors.
                let mut field = field.field.clone();
                field.attrs.clear();
                field
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
            attrs: _,
            vis: _,
            struct_token: _,
            ident,
            generics: _,
            fields: _,
            semi_token: _,
        } = self.struct_;

        let _db_engine = &self.options.engine;
        let _doc = format!("Save a new {ident} to the database, and return the saved instance");
        let new_type_ident = Ident::new(&format!("New{ident}"), Span::call_site());
        let query = self.insert_query();
        let engine = &self.options.engine;

        let mut insert_query = quote! {
            sqlx::query_as::<#engine, #ident>(
                #query
            )
        };
        for bind_call in self.fields.iter().map(|f| {
            let ident = f.ident;
            quote! {.bind(&self.#ident)}.to_token_stream()
        }) {
            insert_query.extend(bind_call);
        }

        let engine = &self.options.engine;
        parse_quote!(
            impl #new_type_ident {
                pub async fn save<'a>(
                    &self,
                    executor: impl sqlx::Executor<'a, Database = #engine>
                ) -> ::sqlx::Result<#ident> {
                    #insert_query.fetch_one(executor).await
                }
            }
        )
    }

    /// Return the impl for the struct.
    fn struct_impl(&self) -> ItemImpl {
        let ident = &self.ident;
        let fields = self
            .fields
            .iter()
            .map(|f| {
                let DatabaseModelField {
                    ident,
                    field: Field { ty, .. },
                    ..
                } = f;
                let field: Field = parse_quote!(#ident: #ty);
                field
            })
            .collect::<Punctuated<Field, Token![,]>>();
        let field_idents = self
            .fields
            .iter()
            .map(|f| f.ident)
            .collect::<Punctuated<&Ident, Token![,]>>();

        parse_quote!(
            impl #ident {
                pub fn new(#fields) -> Self {
                    Self {
                        #field_idents
                    }
                }
            }
        )
    }

    /// Return the INSERT query for this struct.
    fn insert_query(&self) -> String {
        let field_names = self.field_names.join(", ");
        format!(
            "INSERT INTO {table_name} ({field_names}) VALUES ({params}) RETURNING *",
            table_name = self.table_name(),
            params = (0..self.fields.len())
                .map(|i| format!("${}", i + 1))
                .collect::<Vec<String>>()
                .join(", ")
        )
    }

    fn impl_databasemodel_fetch_one_fn(&self) -> ImplItemFn {
        let where_clause = self
            .pk_field_names
            .iter()
            .enumerate()
            .map(|(idx, name)| format!("{name} = ${idx}", idx = idx + 1))
            .collect::<Vec<String>>()
            .join(" AND ");
        let query = format!(
            "SELECT {field_names} FROM {table_name} WHERE {where_clause}",
            field_names = self.field_names.join(", "),
            table_name = self.table_name(),
        );
        let pk_field_type = &self.primary_key_field().ty;
        parse_quote!(
            async fn fetch_one<'e: 'a>(
                executor: impl sqlx::Executor<'e, Database = Self::Database>,
                id: &#pk_field_type,
            ) -> sqlx::Result<Self> {
                sqlx::query_as::<Self::Database, Self>(#query).bind(id).fetch_one(executor).await
            }
        )
    }

    fn impl_databasemodel_fetch_many_fn(&self) -> ImplItemFn {
        let pk_field_name = self.primary_key_field().ident.to_string();
        let pk_field_type = &self.primary_key_field().ty;
        let query = format!(
            "SELECT {field_names} FROM {table_name} WHERE {pk_field_name} IN UNNEST($1::text)",
            field_names = self.field_names.join(", "),
            table_name = self.table_name(),
        );
        parse_quote!(
            fn fetch<'e, 'c, E>(
                executor: E,
                ids: &'c[#pk_field_type],
            ) -> futures_core::stream::BoxStream<'e, sqlx::Result<Self>>
            where
                'c: 'e,
                E: 'e + sqlx::Executor<'c, Database = Self::Database>,
                Self::Database: 'e,
                Self: 'e,
            {
                sqlx::query_as::<Self::Database, Self>(#query).bind(ids).fetch(executor)
            }
        )
    }

    fn impl_databasemodel_fetch_all_fn(&self) -> ImplItemFn {
        let query = format!("SELECT * FROM {table_name}", table_name = self.table_name());
        parse_quote!(
            fn fetch_all<'e, 'c, E>(
                executor: E,
            ) -> futures_core::stream::BoxStream<'e, sqlx::Result<Self>>
            where
                'c: 'e,
                E: 'e + sqlx::Executor<'c, Database = Self::Database>,
                Self::Database: 'e,
                Self: 'e,
            {
                sqlx::query_as::<Self::Database, Self>(#query).fetch(executor)
            }
        )
    }

    fn impl_databasemodel_delete(&self) -> ImplItemFn {
        let pk_field_name = self.primary_key_field().ident.to_string();
        let query = format!(
            "DELETE FROM {table_name} WHERE {pk_field_name} = $1",
            table_name = self.table_name()
        );
        parse_quote!(
            async fn delete(
                &self,
                executor: impl sqlx::Executor<'a, Database = Self::Database>,
            ) -> sqlx::Result<()> {
                sqlx::query(#query).execute(executor).await?;
                Ok(())
            }
        )
    }

    fn impl_databasemodel_delete_many(&self) -> ImplItemFn {
        let pk_field_name = self.primary_key_field().ident.to_string();
        let pk_field_type = &self.primary_key_field().ty;
        let query = format!(
            "DELETE FROM {table_name} WHERE {pk_field_name} IN UNNEST($1::text)",
            table_name = self.table_name()
        );
        parse_quote!(
            async fn delete_many(
                executor: impl sqlx::Executor<'a, Database = Self::Database>,
                ids: &[#pk_field_type],
            ) -> sqlx::Result<()> {
                sqlx::query(#query)
                    .bind(ids)
                    .execute(executor)
                    .await;
                Ok(())
            }
        )
    }

    fn impl_databasemodel_update(&self) -> ImplItemFn {
        let mut bind_index = 0;
        let field_assignments = self
            .field_names
            .iter()
            .map(|name| {
                bind_index += 1;
                format!("{name} = ${bind_index}")
            })
            .collect::<Vec<String>>()
            .join(", ");
        let where_clause = self
            .pk_field_names
            .iter()
            .enumerate()
            .map(|(idx, name)| format!("{name} = ${idx}", idx = idx + bind_index))
            .collect::<Vec<String>>()
            .join(" AND ");
        let bind_clauses: Vec<proc_macro2::TokenStream> = self
            .fields
            .iter()
            .chain(self.pk_fields.iter())
            .map(|field| {
                let ident = &field.ident;
                quote! { .bind(&self.#ident) }
            })
            .collect();
        let query = format!(
            "UPDATE {table_name} SET ({field_assignments}) WHERE {where_clause} RETURNING *",
            table_name = self.table_name()
        );
        parse_quote!(
            async fn update(
                &mut self,
                executor: impl sqlx::Executor<'a, Database = Self::Database>,
            ) -> sqlx::Result<()> {
                let updated: Self = sqlx::query_as(#query)
                #(#bind_clauses)*
                .fetch_one(executor)
                .await?;
                let _ = std::mem::replace(self, updated);
                Ok(())
            }
        )
    }

    fn impl_databasemodel_insert(&self) -> ImplItemFn {
        let mut field_names: Vec<String> = Vec::new();
        let mut field_binds: Vec<proc_macro2::TokenStream> = Vec::new();
        let mut field_params: Vec<String> = Vec::new();
        for (i, field) in self.fields.iter().enumerate() {
            let ident = field.ident;
            field_names.push(ident.to_string());
            field_binds.push(quote! { .bind(&self.#ident) });
            field_params.push(format!("${}", i + 1));
        }

        let field_names = field_names.join(", ");
        let field_params = field_params.join(", ");
        let query = format!(
            "INSERT INTO {table_name} ({field_names} VALUES ({field_params}) RETURNING *",
            table_name = self.table_name()
        );

        parse_quote!(
            async fn insert(
                &mut self,
                executor: impl sqlx::Executor<'a, Database = Self::Database>,
            ) -> sqlx::Result<()> {
                let inserted: Self = sqlx::query_as::<Self::Database, Self>(
                    #query
                )
                #(#field_binds)*
                .fetch_one(executor)
                .await?;
                let _ = std::mem::replace(self, inserted);
                Ok(())
            }
        )
    }

    fn databasemodel_impl_count(&self) -> ImplItemFn {
        let query = format!(
            "SELECT COUNT(1) AS row_count FROM {table_name}",
            table_name = self.table_name()
        );
        parse_quote!(
            async fn count(
                executor: impl sqlx::Executor<'a, Database = Self::Database>,
            ) -> sqlx::Result<i64> {
                let (row_count,) = sqlx::query_as::<Self::Database, (i64,)>(#query).fetch_one(executor).await?;
                Ok(row_count)
            }
        )
    }

    fn impl_databasemodel_query_builder(&self) -> ImplItemFn {
        let table_name = self.table_name();
        let delete_query_base = format!("DELETE FROM {table_name}");
        let select_query_base = format!("SELECT FROM {table_name}");
        let update_query_base = format!("UPDATE {table_name}");
        let insert_query_base = format!("INSERT INTO {table_name}");

        parse_quote!(
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
        )
    }

    fn databasemodel_impl(&self) -> ItemImpl {
        let pk_field = self.primary_key_field();
        let pk_type = &pk_field.ty;
        let ident = &self.ident;
        let fetch_one_impl = self.impl_databasemodel_fetch_one_fn();
        let fetch_impl = self.impl_databasemodel_fetch_many_fn();
        let fetch_all_impl = self.impl_databasemodel_fetch_all_fn();
        let delete_impl = self.impl_databasemodel_delete();
        let delete_many = self.impl_databasemodel_delete_many();
        let update_impl = self.impl_databasemodel_update();
        let insert_impl = self.impl_databasemodel_insert();
        let count_impl = self.databasemodel_impl_count();
        let query_builder_impl = self.impl_databasemodel_query_builder();
        let engine = &self.options.engine;
        parse_quote!(
            impl<'a> ::structqlx::traits::DatabaseModel<'a, #pk_type> for #ident {
                type Database = #engine;

                #fetch_one_impl
                #fetch_impl
                #fetch_all_impl
                #delete_impl
                #delete_many
                #update_impl
                #insert_impl
                #count_impl
                #query_builder_impl
            }
        )
    }
}

impl<'a> ToTokens for DatabaseModelStruct<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.struct_impl().to_tokens(tokens);
        self.new_row_struct().to_tokens(tokens);
        self.new_row_struct_impl().to_tokens(tokens);
        self.databasemodel_impl().to_tokens(tokens);
    }
}

pub(crate) fn expand_db_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemStruct);
    let model_struct = DatabaseModelStruct::try_from(&input).unwrap();
    model_struct.to_token_stream().into()
}
