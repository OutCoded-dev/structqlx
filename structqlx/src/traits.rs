//! Traits to be derived for structs which use structqlx.

use crate::QueryVerb;
use sqlx::QueryBuilder;

pub trait DatabaseModel<'a, T>: Sized + Clone + std::fmt::Debug {
    type Database;

    /// Get a single row from the table.
    fn fetch_one<'e: 'a>(
        executor: impl sqlx::Executor<'e, Database = Self::Database>,
        id: &T,
    ) -> impl Future<Output = sqlx::Result<Self>>;

    /// Get multiple rows from the table.
    fn fetch<'e, 'c, E>(
        executor: E,
        id: &'c [T],
    ) -> futures_core::stream::BoxStream<'e, sqlx::Result<Self>>
    where
        'c: 'e,
        E: 'e + sqlx::Executor<'c, Database = Self::Database>,
        Self::Database: 'e,
        Self: 'e;

    /// Get all rows from the table.
    fn fetch_all<'e, 'c, E>(executor: E) -> futures_core::stream::BoxStream<'e, sqlx::Result<Self>>
    where
        'c: 'e,
        E: 'e + sqlx::Executor<'c, Database = Self::Database>,
        Self::Database: 'e,
        Self: 'e;

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
    ) -> impl Future<Output = sqlx::Result<()>>;

    /// Return the count of rows in the table.
    fn count(
        executor: impl sqlx::Executor<'a, Database = Self::Database>,
    ) -> impl Future<Output = sqlx::Result<i64>>;

    /// Return a QueryBuilder for this model based on the verb.
    fn query_builder<D: sqlx::Database>(query_verb: QueryVerb) -> QueryBuilder<D>;

    /// Return a QueryBuilder for SELECT.
    fn select_query<D: sqlx::Database>() -> QueryBuilder<D> {
        Self::query_builder(QueryVerb::Select)
    }

    /// Return a QueryBuilder for SELECT.
    fn insert_query<D: sqlx::Database>() -> QueryBuilder<D> {
        Self::query_builder(QueryVerb::Insert)
    }

    /// Return a QueryBuilder for SELECT.
    fn update_query<D: sqlx::Database>() -> QueryBuilder<D> {
        Self::query_builder(QueryVerb::Update)
    }

    /// Return a QueryBuilder for SELECT.
    fn delete_query<D: sqlx::Database>() -> QueryBuilder<D> {
        Self::query_builder(QueryVerb::Delete)
    }
}
