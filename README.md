# structqlx

Simple CRUD operations for structs built on top of sqlx.

```rs
use structqlx::macros::DatabaseModel;

#[derive(DatabaseModel)]
#[db_model(generate_getters)]
struct SampleUser {
    username: String,
    age: u32,
    gender: String,
}

let mut user = SampleUser::new().set_username("Foo".into()).age(10000).gender("chaotic");
let mut user.save();

```
