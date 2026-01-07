use std::marker::PhantomData;

use sqlx::{Execute, QueryBuilder};

use structqlx::{QueryField, SqlClause};

// #[test]
// fn string_to_sql() {
//     let statement = QueryStatement {
//         subclauses: vec![
//             Box::new("my_jsonb_field->'foo'".to_string()),
//             Box::new("baz = 'BANG'".to_string()),
//         ],
//         op: crate::JoinOperator::And,
//     };

//     assert_eq!(statement.to_sql(), "my_jsonb_field->'foo' AND baz = 'BANG'");
// }

#[test]
fn query_field_to_sql() {
    let field: QueryField<'_, _, i32> = QueryField {
        arg_offset: None,
        field_name: "baz".to_string(),
        value: Some(1234),
        op: structqlx::Comparison::Eq,
        phantom: PhantomData::<&sqlx::Postgres>,
    };

    assert_eq!(field.to_sql(), "baz = $1");
}

#[sqlx::test(fixtures("test_user"))]
async fn push_bind_appends_to_query(_: sqlx::PgPool) {
    let id_field: QueryField<'_, _, i32> = QueryField {
        arg_offset: None,
        field_name: "id".to_string(),
        value: Some(1234),
        op: structqlx::Comparison::Eq,
        phantom: PhantomData::<&sqlx::Postgres>,
    };

    let mut qb = QueryBuilder::new("SELECT * FROM test_user WHERE ");
    let query = id_field.push_to_builder(&mut qb).build();
    assert_eq!(query.sql(), "SELECT * FROM test_user WHERE id = $1")
}

#[test]
fn bit_add_query_fields_makes_query_statement() {
    let id_field: QueryField<'_, _, i32> = QueryField {
        arg_offset: None,
        field_name: "id".to_string(),
        value: Some(1234),
        op: structqlx::Comparison::Eq,
        phantom: PhantomData::<&sqlx::Postgres>,
    };
    let name_field: QueryField<'_, _, String> = QueryField {
        arg_offset: None,
        field_name: "name".to_string(),
        value: Some("%Debbie from Accounting%".to_string()),
        op: structqlx::Comparison::Like,
        phantom: PhantomData::<&sqlx::Postgres>,
    };

    let mut qb = QueryBuilder::new("SELECT * FROM test_user WHERE ");
    let statement = id_field & name_field;
    let query = statement.push_to_builder(&mut qb).build();
    assert_eq!(
        query.sql(),
        "SELECT * FROM test_user WHERE (id = 1234 AND name LIKE '%Debbie from Accounting%')"
    );
}
