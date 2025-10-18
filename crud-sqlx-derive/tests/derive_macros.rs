use crud_sqlx::traits::DbModel;
use crud_sqlx_derive::db_model;
use sqlx::{Row, Sqlite};
use uuid::Uuid;

#[sqlx::test]
fn model_with_non_optional_id() {
    #[db_model]
    struct MyStruct {
        id: Uuid,
        name: String,
        created_at: String,
    }
}

#[test]
fn model_with_optional_id() {
    #[db_model(table_name = "some_other_table")]
    struct MyStruct {
        id: Option<i32>,
        name: String,
        created_at: String,
    }
}

#[sqlx::test]
async fn can_select_into_model(pool: sqlx::SqlitePool) {
    #[db_model(table_name = "test_table")]
    struct TestUser {
        id: Uuid,
        created_at: Option<time::OffsetDateTime>,
    }
    sqlx::query!("CREATE TABLE IF NOT EXISTS test_schema.test_table (id UUID PRIMARY KEY, created_at TIMESTAMP WITH TIME ZONE DEFAULT now())").execute(&pool).await.unwrap();

    let mut user = TestUser {
        id: Uuid::new_v4(),
        created_at: None,
    };
    let res = user.create(&pool).await;
    assert!(res.is_ok(), "{res:#?}");

    let user_2 = TestUser::get_one(&pool, &user.id).await.unwrap();
    assert_eq!(user_2, user);
}
