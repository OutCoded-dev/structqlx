use sqlx::prelude::FromRow;
use structqlx::macros::DatabaseModel;

#[test]
fn generate_getters_adds_getter_fns() {
    #[allow(unused)]
    #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
    #[structqlx(with_getters, engine = ::sqlx::Sqlite)]
    struct MyStruct {
        #[structqlx(primary_key = true)]
        id: u8,
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
    #[structqlx(with_constructor, engine = ::sqlx::Sqlite)]
    struct MyStruct {
        #[structqlx(primary_key = true)]
        id: u8,
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
            id: u8,
            name: String,
        }

        let mut record = TestUser {
            id: 1,
            name: "Test".into(),
        };
        record.insert(&pool).await.unwrap();

        let stored = TestUser::fetch_one_by_primary_key(&pool, &1).await.unwrap();
        assert_eq!(stored, record);
    }

    #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
    #[structqlx(engine = sqlx::Sqlite)]
    struct TestUser {
        #[structqlx(primary_key = true)]
        id: u8,
        name: String,
    }

    #[sqlx::test(fixtures("test_user"))]
    async fn can_select_into_model(pool: sqlx::SqlitePool) {
        sqlx::query("INSERT INTO test_user (id, name) VALUES (1, 'Test User')")
            .execute(&pool)
            .await
            .unwrap();

        let user = TestUser::fetch_one_by_primary_key(&pool, &1).await.unwrap();
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

        let user_2 = TestUser::fetch_one_by_primary_key(&pool, &user.id)
            .await
            .unwrap();
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
        match TestUser::fetch_one_by_primary_key(&pool, &user.id).await {
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
        let res = user.update(&pool).await;
        if let Err(err) = res {
            panic!("Unexpected error from database on save(): {err:#?}");
        };

        let user_2 = TestUser::fetch_one_by_primary_key(&pool, &user.id)
            .await
            .unwrap();
        assert_eq!(user_2.name, "Sharkey");
    }
}

mod with_named_table {
    use super::*;

    #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
    #[structqlx(table_name = "a_different_table_name", engine = sqlx::Sqlite)]
    struct TestUser {
        #[structqlx(primary_key = true)]
        id: u8,
        name: String,
    }

    #[sqlx::test(fixtures("named_table"))]
    async fn can_select_into_model(pool: sqlx::SqlitePool) {
        sqlx::query("INSERT INTO a_different_table_name (id, name) VALUES (1, 'Test User')")
            .execute(&pool)
            .await
            .unwrap();

        let user = TestUser::fetch_one_by_primary_key(&pool, &1).await.unwrap();
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

        let user_2 = TestUser::fetch_one_by_primary_key(&pool, &user.id)
            .await
            .unwrap();
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
        match TestUser::fetch_one_by_primary_key(&pool, &user.id).await {
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
        let res = user.insert(&pool).await;
        if let Err(err) = res {
            panic!("Unexpected error from database on save(): {err:#?}");
        };

        let user_2 = TestUser::fetch_one_by_primary_key(&pool, &user.id)
            .await
            .unwrap();
        assert_eq!(user_2.name, "Sharkey");
    }
}
