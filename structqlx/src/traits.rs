//! Traits to be derived for structs which use structqlx.

use sqlx::QueryBuilder;

/// Represents the different SQL verbs for the sake of building queries.
pub enum QueryVerb {
    Delete,
    Insert,
    Select,
    Update,
}

impl From<QueryVerb> for String {
    fn from(verb: QueryVerb) -> Self {
        let verb = match verb {
            QueryVerb::Delete => "DELETE",
            QueryVerb::Insert => "INSERT",
            QueryVerb::Select => "SELECT",
            QueryVerb::Update => "UPDATE",
        };
        verb.into()
    }
}

pub trait DatabaseModel<'a, T>: Sized + Clone + std::fmt::Debug {
    type Database;

    /// Return the table name for the model.
    fn table_name() -> String;

    /// Return the fields for the model.
    fn field_names() -> Vec<&'static str>;

    #[cfg(feature = "postgres")]
    /// Return a schema for the model. Default is `public`.
    #[inline]
    fn schema() -> &'static str {
        "public"
    }

    /// Get a single row from the table.
    fn fetch_one(
        executor: impl sqlx::Executor<'a, Database = Self::Database>,
        id: &T,
    ) -> impl Future<Output = sqlx::Result<Self>>;

    /// Get multiple rows from the table.
    fn fetch_many(
        executor: impl sqlx::Executor<'a, Database = Self::Database>,
        id: &[T],
    ) -> impl Future<Output = sqlx::Result<Vec<Self>>>;

    /// Get all rows from the table.
    fn fetch_all(
        executor: impl sqlx::Executor<'a, Database = Self::Database>,
    ) -> impl Future<Output = sqlx::Result<Vec<Self>>>;

    /// Delete the row from the table.
    fn delete(
        &self,
        executor: impl sqlx::Executor<'a, Database = Self::Database>,
    ) -> impl Future<Output = sqlx::Result<()>>;

    /// Delete multiple rows from the table.
    fn delete_many(
        executor: impl sqlx::Executor<'a, Database = Self::Database>,
        ids: &[T],
    ) -> impl Future<Output = sqlx::Result<()>>;

    /// Update the current record in the database.
    fn update(
        &mut self,
        executor: impl sqlx::Executor<'a, Database = Self::Database> + Copy,
    ) -> impl Future<Output = sqlx::Result<()>>;

    /// Insert a new record in the database.
    fn insert(
        &mut self,
        executor: impl sqlx::Executor<'a, Database = Self::Database>,
    ) -> impl Future<Output = sqlx::Result<Self>>;

    /// Save the current record in the database.
    ///
    /// If the record does not exist, create it. Otherwise, update it.
    fn save(
        &mut self,
        executor: impl sqlx::Executor<'a, Database = Self::Database> + Copy,
    ) -> impl Future<Output = sqlx::Result<()>>;

    /// Return the count of rows in the table.
    fn count(
        executor: impl sqlx::Executor<'a, Database = Self::Database>,
    ) -> impl Future<Output = sqlx::Result<i64>>;

    /// Return a QueryBuilder for this model based on the verb.
    fn query_builder<'d, D: sqlx::Database>(query_verb: QueryVerb) -> QueryBuilder<'d, D> {
        let query = match query_verb {
            QueryVerb::Delete => format!("DELETE FROM {} WHERE", Self::table_name()),
            QueryVerb::Select => format!("SELECT * FROM {} WHERE", Self::table_name()),
            QueryVerb::Update => format!("UPDATE {}", Self::table_name()),
            QueryVerb::Insert => format!(
                "INSERT INTO {table} ({fields}) VALUES",
                table = Self::table_name(),
                fields = Self::field_names().join(", ")
            ),
        };
        QueryBuilder::new(query)
    }

    /// Return a QueryBuilder for SELECT.
    fn select_query<'d, D: sqlx::Database>() -> QueryBuilder<'d, D> {
        Self::query_builder(QueryVerb::Select)
    }

    /// Return a QueryBuilder for SELECT.
    fn insert_query<'d, D: sqlx::Database>() -> QueryBuilder<'d, D> {
        Self::query_builder(QueryVerb::Insert)
    }

    /// Return a QueryBuilder for SELECT.
    fn update_query<'d, D: sqlx::Database>() -> QueryBuilder<'d, D> {
        Self::query_builder(QueryVerb::Update)
    }

    /// Return a QueryBuilder for SELECT.
    fn delete_query<'d, D: sqlx::Database>() -> QueryBuilder<'d, D> {
        Self::query_builder(QueryVerb::Delete)
    }
}
