use sqlx::prelude::FromRow;
use sqlx::{PgPool, Postgres};
use structqlx::DatabaseModel;
use structqlx_macros::DatabaseModel;

#[derive(Clone, DatabaseModel, FromRow, Debug, PartialEq)]
#[structqlx(engine = sqlx::Postgres)]
struct ExampleDatabaseModel {
    #[structqlx(primary_key)]
    id: i32,
    name: String,
    age: i32,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().unwrap();
    let pool = PgPool::connect(std::env::var("DATABASE_URL").unwrap().as_str())
        .await
        .unwrap();

    let txn = pool.begin().await.unwrap();
    let q = sqlx::query::<Postgres>(
        "CREATE TABLE IF NOT EXISTS example_database_model (id int not null, name text not null, age int not null)",
    );
    q.execute(&pool).await.unwrap();

    let mut new = ExampleDatabaseModel {
        id: 1,
        name: "Graham".into(),
        age: 45,
    };
    new.insert(&pool).await.unwrap();
    let saved = ExampleDatabaseModel::fetch_one_by_primary_key(&pool, &1)
        .await
        .unwrap();
    assert_eq!(saved, new);
    txn.rollback().await.unwrap();
}
