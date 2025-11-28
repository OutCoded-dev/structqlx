//! crud-sqlx - basic CRUD (Create, Read, Update, Delete) model generation for SQLx.

pub mod traits;
pub use traits::*;

#[cfg(feature = "derive")]
pub mod derive {
    pub use crate::traits::DatabaseModel;
    pub use crud_sqlx_derive::DatabaseModel;
}
