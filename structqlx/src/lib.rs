//! structqlx - basic CRUD (Create, Read, Update, Delete) model generation for SQLx.

pub mod traits;

pub use traits::*;

#[cfg(feature = "macros")]
pub mod macros {
    pub use crate::traits::DatabaseModel;
    pub use structqlx_macros::DatabaseModel;
}

#[cfg(test)]
mod test {}
