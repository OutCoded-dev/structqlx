//! structqlx - basic CRUD (Create, Read, Update, Delete) model generation for SQLx.

pub mod traits;
use std::{any::Any, collections::HashMap, ops, path::Display};

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
        f.write_str(&self.to_sql())
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

pub trait SqlClause {
    type Value;

    /// Return the query field as SQL, e.g.
    ///
    /// ```no_run
    /// let field = QueryField<String> {
    ///     field_name: "my_field".to_string(),
    ///     value: Some("Some value".to_string()),
    ///     op:
    /// }
    ///
    /// assert_eq!(field.to_sql(None), )
    /// ```
    fn to_sql(&self) -> String;

    /// Return the bound values.
    fn binds(&self) -> Vec<(usize, &Option<Self::Value>)>;

    /// Return the last arg index for this clause.
    fn last_arg_offset(&self) -> Option<usize>;
}

/// Represents a single field in a query.
pub struct QueryField<T> {
    pub field_name: String,
    pub value: Option<T>,
    pub op: Comparison,
    pub arg_offset: Option<usize>,
}

impl SqlClause for QueryField<T> {
    type Value = T;

    fn to_sql(&self) -> String {
        format!(
            "{} {} ${}",
            self.field_name,
            self.op,
            self.arg_offset.unwrap_or(0) + 1
        )
    }

    fn binds(&self) -> Vec<(usize, &Option<Self::Value>)> {
        let offset = self.arg_offset.unwrap_or(0);
        vec![(offset + 1, &self.value)]
    }

    fn last_arg_offset(&self) -> Option<usize> {
        self.arg_offset.map(|start| start + 1)
    }
}

impl<T> std::ops::BitAnd for QueryField<T> {
    type Output = QueryStatement;

    fn bitand(self, rhs: Self) -> Self::Output {
        QueryStatement {
            subclauses: vec![Box::new(self), Box::new(rhs)],
            op: JoinOperator::And,
        }
    }
}

pub struct QueryStatement {
    pub subclauses: Vec<Box<impl SqlClause>>,
    pub op: JoinOperator,
}

impl SqlClause for QueryStatement {
    fn to_sql(&self) -> String {
        self.subclauses
            .iter()
            .map(|f| f.to_sql())
            .collect::<Vec<String>>()
            .join(&self.op.to_sql())
    }
}

impl SqlClause for String {
    fn to_sql(&self) -> String {
        self.clone()
    }
}

impl SqlClause for &str {
    fn to_sql(&self) -> String {
        self.to_string()
    }
}

#[cfg(test)]
mod test {
    use crate::{QueryField, QueryStatement, SqlClause};

    #[test]
    fn string_to_sql() {
        let statement = QueryStatement {
            subclauses: vec![
                Box::new("my_jsonb_field->'foo'".to_string()),
                Box::new("baz = 'BANG'".to_string()),
            ],
            op: crate::JoinOperator::And,
        };

        assert_eq!(statement.to_sql(), "my_jsonb_field->'foo' AND baz = 'BANG'");
    }

    #[test]
    fn query_field_to_sql() {
        let field = QueryField {
            arg_offset: None,
            field_name: "baz".to_string(),
            value: Some(Box::new(1234)),
            op: crate::Comparison::Eq,
        };

        assert_eq!(field.to_sql(), "baz = $1");
    }
}
