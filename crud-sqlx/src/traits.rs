//! DB traits for Nexus DB objects.

pub trait DatabaseModel<'a, T>: Sized {
    type Database;

    /// Return the table name for the model.
    fn table_name() -> String;

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
        executor: impl sqlx::Executor<'a, Database = Self::Database>,
    ) -> impl Future<Output = sqlx::Result<()>>;

    /// Create a new record in the database.
    fn create(
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
}

/// A trait to be implemented on models which relate to other models.
pub trait HasRelated<'a, R, T>
where
    R: DatabaseModel<'a, T>,
{
    fn fetch_all_related(&self) -> impl Future<Output = sqlx::Result<Vec<R>>>;
    fn fetch_many_related(&self) -> impl Future<Output = sqlx::Result<Vec<R>>>;
    fn fetch_one_related(&self) -> impl Future<Output = sqlx::Result<R>>;
}
