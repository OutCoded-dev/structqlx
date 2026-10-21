use sqlx::prelude::FromRow;

use structqlx::macros::DatabaseModel;

#[test]
fn generate_getters_adds_getter_fns() {
    #[allow(unused)]
    #[derive(Clone, DatabaseModel, Debug, FromRow)]
    #[structqlx(with_getters, engine = ::sqlx::Postgres)]
    struct MyStruct {
        #[structqlx(primary_key = true)]
        id: i32,
        foo: String,
    }

    let val = MyStruct {
        id: 1,
        foo: "Bar".to_string(),
    };

    assert_eq!(val.id(), 1);
    assert_eq!(val.foo(), "Bar");
}

#[test]
fn with_setters_adds_setter_fns() {
    #[allow(unused)]
    #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
    #[structqlx(with_setters, engine = ::sqlx::Postgres)]
    struct MyStruct {
        #[structqlx(primary_key = true)]
        id: i32,
        foo: String,
    }

    let mut val = MyStruct {
        id: 1,
        foo: "Bar".to_string(),
    };

    val.set_id(2);
    assert_eq!(val.id, 2);
}

#[test]
fn getter_prefix_alters_getter_fn_name() {
    #[allow(unused)]
    #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
    #[structqlx(with_getters, getter_prefix = "get_", engine = ::sqlx::Postgres)]
    struct MyStruct {
        #[structqlx(primary_key = true)]
        id: i32,
        foo: String,
    }

    let val = MyStruct {
        id: 1,
        foo: "Bar".to_string(),
    };

    assert_eq!(1, val.get_id());
}

#[test]
fn setter_prefix_alters_setter_fn_name() {
    #[allow(unused)]
    #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
    #[structqlx(with_setters, setter_prefix = "with_", engine = ::sqlx::Postgres)]
    struct MyStruct {
        #[structqlx(primary_key = true)]
        id: i32,
        foo: String,
    }

    let mut val = MyStruct {
        id: 1,
        foo: "Bar".to_string(),
    };

    val.with_id(2);
    assert_eq!(val.id, 2);
}

#[test]
fn constructor_constructs() {
    #[allow(unused)]
    #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
    #[structqlx(with_constructor, engine = ::sqlx::Postgres)]
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
    use futures::StreamExt;

    use super::*;

    #[sqlx::test(fixtures("test_user"))]
    async fn model_may_have_required_id(pool: sqlx::PgPool) {
        #[allow(dead_code)]
        #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
        #[structqlx(engine = ::sqlx::Postgres)]
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

        assert_eq!(
            TestUser::fetch_one_by_primary_key(&pool, &1).await.unwrap(),
            record
        );
    }

    #[sqlx::test(fixtures("test_user_non_id_pk"))]
    async fn model_may_specify_pk_field(pool: sqlx::PgPool) {
        #[allow(dead_code)]
        #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
        #[structqlx(engine = ::sqlx::Postgres)]
        struct TestUser {
            #[structqlx(primary_key = true)]
            user_id: i32,
            name: String,
        }

        let mut record = TestUser {
            user_id: 1,
            name: "Test".into(),
        };
        record.insert(&pool).await.unwrap();

        assert_eq!(
            TestUser::fetch_one_by_primary_key(&pool, &1).await.unwrap(),
            record
        );
    }

    #[sqlx::test(fixtures("test_user_autoincrement_pg"))]
    fn model_may_have_optional_id(pool: sqlx::PgPool) {
        #[allow(dead_code)]
        #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq, Default)]
        #[structqlx(engine = ::sqlx::Postgres)]
        struct TestUser {
            #[structqlx(primary_key, set_on_insert)]
            id: Option<i32>,
            name: String,
        }

        let mut record = TestUser {
            name: "Is this number got curves?".into(),
            ..Default::default()
        };
        record.insert(&pool).await.unwrap();

        let records = TestUser::fetch_all(&pool)
            .filter_map(async |item: sqlx::Result<TestUser>| item.ok())
            .collect::<Vec<TestUser>>()
            .await;
        assert_eq!(1, records.len());
    }

    #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
    #[structqlx(engine = ::sqlx::Postgres)]
    struct TestUser {
        #[structqlx(primary_key = true)]
        id: i32,
        name: String,
    }

    #[sqlx::test(fixtures("test_user"))]
    async fn can_select_into_model(pool: sqlx::PgPool) {
        sqlx::query("INSERT INTO test_user (id, name) VALUES (1, 'Test User')")
            .execute(&pool)
            .await
            .unwrap();

        let user = TestUser::fetch_one_by_primary_key(&pool, &1).await.unwrap();
        assert_eq!(user.id, 1);
        assert_eq!(user.name, "Test User");
    }

    #[sqlx::test(fixtures("test_user"))]
    async fn can_insert_from_model_to_table(pool: sqlx::PgPool) {
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
    async fn can_delete_rows(pool: sqlx::PgPool) {
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
    async fn can_update_rows(pool: sqlx::PgPool) {
        let mut user = TestUser {
            id: 1,
            name: "Test User".to_string(),
        };
        user.insert(&pool).await.unwrap();

        let user_2 = TestUser::fetch_one_by_primary_key(&pool, &user.id)
            .await
            .unwrap();
        assert_eq!(user_2, user);

        user.name = "Sharkey".into();
        user.update(&pool).await.unwrap();

        let user_2 = TestUser::fetch_one_by_primary_key(&pool, &user.id)
            .await
            .unwrap();
        assert_eq!(user_2.name, "Sharkey");
    }
}

mod with_named_table {
    use super::*;

    #[derive(Clone, DatabaseModel, Debug, FromRow, PartialEq)]
    #[structqlx(table_name = "a_different_table_name", engine = ::sqlx::Postgres)]
    struct TestUser {
        #[structqlx(primary_key = true)]
        id: i32,
        name: String,
    }

    #[sqlx::test(fixtures("named_table"))]
    async fn can_select_into_model(pool: sqlx::PgPool) {
        sqlx::query("INSERT INTO a_different_table_name (id, name) VALUES (1, 'Test User')")
            .execute(&pool)
            .await
            .unwrap();

        let user = TestUser::fetch_one_by_primary_key(&pool, &1).await.unwrap();
        assert_eq!(user.id, 1);
        assert_eq!(user.name, "Test User");
    }

    #[sqlx::test(fixtures("named_table"))]
    async fn can_insert_from_model_to_table(pool: sqlx::PgPool) {
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
    async fn can_delete_rows(pool: sqlx::PgPool) {
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
    async fn can_update_rows(pool: sqlx::PgPool) {
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
