//! structqlx - basic CRUD (Create, Read, Update, Delete) model generation for SQLx.

pub mod traits;
pub use traits::*;

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

#[cfg(feature = "macros")]
pub mod macros {
    pub use crate::traits::DatabaseModel;
    pub use structqlx_macros::DatabaseModel;
}
