# structqlx

Simple CRUD operations for structs built on top of sqlx.

```rs
use structqlx::macros::DatabaseModel;

#[derive(DatabaseModel)]
#[structqlx(databae = ::sqlx::Sqlite, generate_getters)]
struct SampleUser {
    pub id: i32,
    pub username: String,
    pub age: u32,
    pub attitude: String,
}

let user = SampleUser::new()
    .set_username("Foo".into())
    .age(10000)
    .attitude("chaotic")
    .create()
    .await;

let user_2 = SampleUser::fetch_one(&pool, 2).await.unwrap();
```
