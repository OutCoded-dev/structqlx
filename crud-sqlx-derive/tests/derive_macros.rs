use crud_sqlx::traits::DatabaseModel;
use crud_sqlx_derive::db_model;

#[sqlx::test]
fn model_with_non_optional_id() {
    #[allow(dead_code)]
    #[db_model]
    struct MyStruct {
        id: i32,
        name: String,
        created_at: String,
    }
}

#[test]
fn model_with_optional_id() {
    #[allow(dead_code)]
    #[db_model]
    struct MyStruct {
        id: Option<i32>,
        name: String,
        created_at: String,
    }
}

#[sqlx::test]
async fn can_select_into_model(pool: sqlx::SqlitePool) {
    #[db_model]
    struct TestUser {
        id: i32,
        name: String,
    }
    sqlx::query("CREATE TABLE IF NOT EXISTS test_user (id INTEGER PRIMARY KEY, name TEXT)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO test_user (id, name) VALUES (1, 'Test User')")
        .execute(&pool)
        .await
        .unwrap();

    let user = TestUser::fetch_one(&pool, &1).await.unwrap();
    assert_eq!(user.id, 1);
    assert_eq!(user.name, "Test User");
}

#[sqlx::test]
async fn can_insert_from_model_to_table(pool: sqlx::SqlitePool) {
    #[db_model]
    struct TestUser {
        id: i32,
        name: String,
    }

    sqlx::query("CREATE TABLE IF NOT EXISTS test_user (id INTEGER PRIMARY KEY, name TEXT)")
        .execute(&pool)
        .await
        .unwrap();

    let mut user = TestUser {
        id: 1,
        name: "Test User".to_string(),
    };
    let res = user.create(&pool).await;
    assert!(res.is_ok(), "{res:#?}");

    let user_2 = TestUser::fetch_one(&pool, &user.id).await.unwrap();
    assert_eq!(user_2, user);
}

#[sqlx::test]
async fn can_delete_rows(pool: sqlx::SqlitePool) {
    #[db_model]
    struct TestUser {
        id: i32,
        name: String,
    }

    sqlx::query("CREATE TABLE IF NOT EXISTS test_user (id INTEGER PRIMARY KEY, name TEXT)")
        .execute(&pool)
        .await
        .unwrap();

    let mut user = TestUser {
        id: 1,
        name: "Test User".to_string(),
    };
    let res = user.create(&pool).await;
    assert!(res.is_ok(), "{res:#?}");

    user.delete(&pool).await.unwrap();
    match TestUser::fetch_one(&pool, &user.id).await {
        Err(sqlx::Error::RowNotFound) => (),
        res => panic!("Unexpected result {res:#?}"),
    }
}

#[sqlx::test]
async fn can_update_rows(pool: sqlx::SqlitePool) {
    #[db_model]
    struct TestUser {
        id: i32,
        name: String,
    }

    sqlx::query("CREATE TABLE IF NOT EXISTS test_user (id INTEGER PRIMARY KEY, name TEXT)")
        .execute(&pool)
        .await
        .unwrap();

    let mut user = TestUser {
        id: 1,
        name: "Test User".to_string(),
    };

    user.name = "Sharkey".into();
    let res = user.save(&pool).await;
    if let Err(err) = res {
        panic!("Unexpected error from database on save(): {err:#?}");
    };

    let user_2 = TestUser::fetch_one(&pool, &user.id).await.unwrap();
    assert_eq!(user_2.name, "Sharkey");
}
