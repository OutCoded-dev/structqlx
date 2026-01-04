//! structqlx - basic CRUD (Create, Read, Update, Delete) model generation for SQLx.

pub mod traits;
use std::{
    fmt::{Display, Write},
    marker::PhantomData,
};

use sqlx::{Database, Encode, QueryBuilder, query::Query};
pub use traits::*;

#[cfg(feature = "macros")]
pub mod macros {
    pub use crate::traits::DatabaseModel;
    pub use structqlx_macros::DatabaseModel;
}

pub enum Comparison {
    Eq,
    NotEq,
    Like,
    ILike,
    Is,
    IsNot,
}

impl Comparison {
    fn to_sql(&self) -> &'static str {
        match self {
            Self::Eq => "=",
            Self::NotEq => "!=",
            Self::Like => "LIKE",
            Self::ILike => "ILIKE",
            Self::Is => "IS",
            Self::IsNot => "IS NOT",
        }
    }
}

impl std::fmt::Display for Comparison {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_sql())
    }
}

pub enum JoinOperator {
    And,
    Or,
    Like,
    ILike,
    Not,
}

impl std::fmt::Display for JoinOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_sql())
    }
}

impl JoinOperator {
    fn to_sql(&self) -> &'static str {
        match self {
            JoinOperator::And => "AND",
            JoinOperator::ILike => "ILIKE",
            JoinOperator::Like => "LIKE",
            JoinOperator::Not => "NOT",
            JoinOperator::Or => "OR",
        }
    }
}

pub trait SqlClause<'a, DB>: Display
where
    DB: Database,
{
    /// Return the query field as SQL, e.g.
    ///
    /// ```no_run
    /// # use structqlx::{Comparison, SqlClause, QueryField};
    /// # fn field_to_sql() {
    /// let field: QueryField<_, String> = QueryField {
    ///     field_name: "my_field".to_string(),
    ///     value: Some("Some value".to_string()),
    ///     op: Comparison::Eq,
    ///     arg_offset: None,
    ///     phantom: std::marker::PhantomData::<&sqlx::Postgres>,
    /// };
    ///
    /// assert_eq!(field.to_sql(), "my_field = $1");
    /// # }
    /// ```
    fn to_sql(&self) -> String;

    /// Push a bind parameter onto a querybuilder.
    fn push_bind(
        &'a self,
        query_builder: &'a mut QueryBuilder<'a, DB>,
    ) -> &'a mut QueryBuilder<'a, DB>;
}

/// Represents a single field in a query.
pub struct QueryField<'a, DB, T>
where
    DB: Database,
    T: Encode<'a, DB> + sqlx::Type<DB> + Display,
{
    pub field_name: String,
    pub value: Option<T>,
    pub op: Comparison,
    pub arg_offset: Option<usize>,

    pub phantom: PhantomData<&'a DB>,
}

impl<'a, DB, T> std::fmt::Display for QueryField<'a, DB, T>
where
    DB: Database,
    T: Encode<'a, DB> + sqlx::Type<DB> + Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_sql())
    }
}

impl<'a, DB, T> SqlClause<'a, DB> for QueryField<'a, DB, T>
where
    DB: Database,
    T: Encode<'a, DB> + sqlx::Type<DB> + Display,
{
    fn to_sql(&self) -> String {
        format!(
            "{} {} ${}",
            self.field_name,
            self.op,
            self.arg_offset.unwrap_or(0) + 1
        )
    }

    fn push_bind(
        &'a self,
        query_builder: &'a mut QueryBuilder<'a, DB>,
    ) -> &'a mut QueryBuilder<'a, DB> {
        let Some(value) = self.value.as_ref() else {
            return query_builder.push(format!("{} {} NULL", &self.field_name, self.op));
        };

        query_builder
            .push(self.field_name.clone())
            .push(" ".to_string())
            .push(self.op.to_sql())
            .push(" ".to_string())
            .push_bind(value)
    }
}

impl<'a, 'b, DB, T, R> std::ops::BitAnd<QueryField<'b, DB, R>> for QueryField<'a, DB, T>
where
    DB: Database,
    T: Encode<'a, DB> + sqlx::Type<DB> + Display,
    R: Encode<'b, DB> + sqlx::Type<DB> + Display + 'a,
    Self: 'a,
    'a: 'b,
    'b: 'a,
{
    type Output = QueryStatement<'a, DB>;

    fn bitand(self, rhs: QueryField<'b, DB, R>) -> Self::Output {
        let mut statement = QueryStatement {
            subclauses: vec![],
            op: JoinOperator::And,
        };

        statement.subclauses.push(Box::new(self));
        statement.subclauses.push(Box::new(rhs));
        statement
    }
}

pub struct QueryStatement<'a, DB> {
    pub subclauses: Vec<Box<dyn SqlClause<'a, DB> + 'a>>,
    pub op: JoinOperator,
}

impl<'a, DB> Display for QueryStatement<'a, DB>
where
    DB: Database,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_sql())
    }
}

impl<'a, DB> SqlClause<'a, DB> for QueryStatement<'a, DB>
where
    DB: Database,
{
    fn to_sql(&self) -> String {
        let join = format!(" {} ", self.op.to_sql());
        self.subclauses
            .iter()
            .map(|f| f.to_sql())
            .collect::<Vec<String>>()
            .join(&join)
    }

    fn push_bind(
        &'a self,
        query_builder: &'a mut QueryBuilder<'a, DB>,
    ) -> &'a mut QueryBuilder<'a, DB> {
        let mut query_builder = query_builder;
        query_builder.push("(");
        for subclause in self.subclauses.iter() {
            query_builder = subclause.push_bind(query_builder);
        }

        query_builder
    }
}

// impl<'a, DB> SqlClause<DB> for String {
//     fn to_sql(&self) -> String {
//         self.clone()
//     }

//     fn push_bind<'args>(
//         &'a self,
//         query_builder: &'a mut QueryBuilder<'args, DB>,
//     ) -> &'a mut QueryBuilder<'args, DB> {
//         query_builder
//     }
// }

// impl<'a, DB: Database> SqlClause<DB> for &'a str {
//     fn to_sql(&self) -> String {
//         self.to_string()
//     }

//     fn push_bind<'args>(
//         &'a self,
//         query_builder: &'a mut QueryBuilder<'args, DB>,
//     ) -> &'a mut QueryBuilder<'args, DB> {
//         query_builder
//     }
// }
