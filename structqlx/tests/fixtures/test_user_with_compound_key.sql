CREATE TABLE IF NOT EXISTS test_user_with_composite_key (
     id             INTEGER NOT NULL
    ,group_id       INTEGER NOT NULL
    ,name           TEXT    NOT NULL
    ,PRIMARY KEY (id, group_id)
);
