use crud_sqlx::traits::DbModel;
use crud_sqlx_derive::db_model;

#[sqlx::test]
fn model_with_non_optional_id() {
    #[allow(dead_code)]
    #[db_model(db = sqlx::Sqlite)]
    struct MyStruct {
        id: i32,
        name: String,
        created_at: String,
    }
}

#[test]
fn model_with_optional_id() {
    #[allow(dead_code)]
    #[db_model(db = sqlx::Sqlite, table_name = "some_other_table")]
    struct MyStruct {
        id: Option<i32>,
        name: String,
        created_at: String,
    }
}

#[sqlx::test]
async fn can_select_into_model(pool: sqlx::SqlitePool) {
    #[db_model(db = sqlx::Sqlite, table_name = "test_table")]
    struct TestUser {
        id: i32,
        name: String,
    }
    sqlx::query("CREATE TABLE IF NOT EXISTS test_table (id INTEGER PRIMARY KEY, name TEXT)")
        .execute(&pool)
        .await
        .unwrap();

    let mut user = TestUser {
        id: 1,
        name: "Test User".to_string(),
    };
    let res = user.create(&pool).await;
    assert!(res.is_ok(), "{res:#?}");

    let user_2 = TestUser::get_one(&pool, &user.id).await.unwrap();
    assert_eq!(user_2, user);
}
