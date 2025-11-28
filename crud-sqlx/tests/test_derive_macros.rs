use crud_sqlx::derive::DatabaseModel;
use sqlx::prelude::FromRow;

#[test]
fn generate_getters_adds_getter_fns() {
    #[allow(unused)]
    #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
    #[db_model(generate_getters)]
    struct MyStruct {
        id: i32,
        foo: String,
    }

    let val = MyStruct {
        id: 1,
        foo: "Bar".to_string(),
    };

    assert_eq!(val.id(), &1);
    assert_eq!(val.foo(), "Bar");
}

mod bare_derive_macro {
    use super::*;

    #[test]
    fn model_may_have_non_optional_id() {
        #[allow(dead_code)]
        #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
        struct MyStruct {
            id: i32,
            name: String,
            created_at: String,
        }
    }

    #[test]
    fn model_may_have_optional_id() {
        #[allow(dead_code)]
        #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
        struct MyStruct {
            id: Option<i32>,
            name: String,
            created_at: String,
        }
    }

    #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
    struct TestUser {
        id: i32,
        name: String,
    }

    #[sqlx::test(fixtures("test_user"))]
    async fn can_select_into_model(pool: sqlx::SqlitePool) {
        sqlx::query("INSERT INTO test_user (id, name) VALUES (1, 'Test User')")
            .execute(&pool)
            .await
            .unwrap();

        let user = TestUser::fetch_one(&pool, &1).await.unwrap();
        assert_eq!(user.id, 1);
        assert_eq!(user.name, "Test User");
    }

    #[sqlx::test(fixtures("test_user"))]
    async fn can_insert_from_model_to_table(pool: sqlx::SqlitePool) {
        let mut user = TestUser {
            id: 1,
            name: "Test User".to_string(),
        };
        let res = user.create(&pool).await;
        assert!(res.is_ok(), "{res:#?}");

        let user_2 = TestUser::fetch_one(&pool, &user.id).await.unwrap();
        assert_eq!(user_2, user);
    }

    #[sqlx::test(fixtures("test_user"))]
    async fn can_delete_rows(pool: sqlx::SqlitePool) {
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

    #[sqlx::test(fixtures("test_user"))]
    async fn can_update_rows(pool: sqlx::SqlitePool) {
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
}

mod with_named_table {
    use super::*;

    #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
    #[db_model(table_name = "a_different_table_name")]
    struct TestUser {
        id: i32,
        name: String,
    }

    #[test]
    fn table_name_matches_declared() {
        assert_eq!(TestUser::table_name(), "a_different_table_name");
    }

    #[sqlx::test(fixtures("named_table"))]
    async fn can_select_into_model(pool: sqlx::SqlitePool) {
        sqlx::query("INSERT INTO a_different_table_name (id, name) VALUES (1, 'Test User')")
            .execute(&pool)
            .await
            .unwrap();

        let user = TestUser::fetch_one(&pool, &1).await.unwrap();
        assert_eq!(user.id, 1);
        assert_eq!(user.name, "Test User");
    }

    #[sqlx::test(fixtures("named_table"))]
    async fn can_insert_from_model_to_table(pool: sqlx::SqlitePool) {
        let mut user = TestUser {
            id: 1,
            name: "Test User".to_string(),
        };
        let res = user.create(&pool).await;
        assert!(res.is_ok(), "{res:#?}");

        let user_2 = TestUser::fetch_one(&pool, &user.id).await.unwrap();
        assert_eq!(user_2, user);
    }

    #[sqlx::test(fixtures("named_table"))]
    async fn can_delete_rows(pool: sqlx::SqlitePool) {
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

    #[sqlx::test(fixtures("named_table"))]
    async fn can_update_rows(pool: sqlx::SqlitePool) {
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
}
