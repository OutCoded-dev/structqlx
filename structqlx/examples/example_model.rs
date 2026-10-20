use sqlx::Postgres;
use sqlx::prelude::FromRow;
use structqlx_macros::DatabaseModel;

#[derive(Clone, DatabaseModel, FromRow, Debug)]
#[structqlx(engine = sqlx::Postgres)]
struct ExampleDatabaseModel {
    #[structqlx(primary_key)]
    id: i32,
    name: String,
    age: i32,
}

fn main() {
    let _ =
        sqlx::query_as::<Postgres, ExampleDatabaseModel>("SELECT * FROM example_database_model");
}
