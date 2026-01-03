use sqlx::prelude::FromRow;
use structqlx::macros::DatabaseModel;

#[test]
fn generate_getters_adds_getter_fns() {
    #[allow(unused)]
    #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
    #[structqlx(generate_getters, engine = ::sqlx::Sqlite)]
    struct MyStruct {
        #[structqlx(primary_key = true)]
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

#[test]
fn constructor_constructs() {
    #[allow(unused)]
    #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
    #[structqlx(constructor, engine = ::sqlx::Sqlite)]
    struct MyStruct {
        #[structqlx(primary_key = true)]
        id: i32,
        foo: String,
    }

    let val = MyStruct::new(1, "Foo".to_string());
    assert_eq!(val.id, 1);
    assert_eq!(val.foo, "Foo");
}

mod bare_derive_macro {
    use super::*;

    #[sqlx::test(fixtures("test_user"))]
    async fn model_may_have_required_id(pool: sqlx::SqlitePool) {
        #[allow(dead_code)]
        #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
        #[structqlx(engine = ::sqlx::Sqlite)]
        struct TestUser {
            #[structqlx(primary_key = true)]
            id: i32,
            name: String,
        }

        let mut record = TestUser {
            id: 1,
            name: "Test".into(),
        };
        record.insert(&pool).await.unwrap();

        assert_eq!(TestUser::fetch_one(&pool, &1).await.unwrap(), record);
    }

    #[sqlx::test(fixtures("test_user"))]
    fn model_may_have_optional_id(pool: sqlx::SqlitePool) {
        #[allow(dead_code)]
        #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq, Default)]
        #[structqlx(engine = sqlx::Sqlite)]
        struct TestUser {
            #[structqlx(primary_key = true)]
            id: Option<i32>,
            name: String,
        }

        let mut record = TestUser {
            name: "Is this number got curves?".into(),
            ..Default::default()
        };
        record.insert(&pool).await.unwrap();

        let records = TestUser::fetch_all(&pool).await.unwrap();
        assert_eq!(1, records.len());
        assert_eq!(records.first().unwrap().name, "Is this number got curves?");
    }

    #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
    #[structqlx(engine = sqlx::Sqlite)]
    struct TestUser {
        #[structqlx(primary_key = true)]
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
        let res = user.insert(&pool).await;
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
        let res = user.insert(&pool).await;
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
        user.insert(&pool).await.unwrap();

        user.name = "Sharkey".into();
        let res = user.save(&pool).await;
        if let Err(err) = res {
            panic!("Unexpected error from database on save(): {err:#?}");
        };

        let user_2 = TestUser::fetch_one(&pool, &user.id).await.unwrap();
        assert_eq!(user_2.name, "Sharkey");
    }

    #[sqlx::test(fixtures("test_user"))]
    async fn can_find_by_non_id_fields(pool: sqlx::SqlitePool) {
        let mut user = TestUser {
            id: 1,
            name: "Test User".to_string(),
        };
        user.save(&pool).await.unwrap();

        let result = TestUser::find_by_name(&pool, "Test User".to_string())
            .await
            .unwrap();
        assert_eq!(result, vec![user]);
    }
}

mod with_named_table {
    use super::*;

    #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
    #[structqlx(table_name = "a_different_table_name", engine = sqlx::Sqlite)]
    struct TestUser {
        #[structqlx(primary_key = true)]
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
        let res = user.insert(&pool).await;
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
        let res = user.insert(&pool).await;
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
