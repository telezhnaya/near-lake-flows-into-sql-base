#[macro_export]
macro_rules! batch_insert {
    ($pool: expr, $query: expr, $rows: expr $(,)?) => {{
        if !$rows.is_empty() {
            // TODO find a better way to insert the objects to the DB
            // TODO add split into chunks logic
            let values = $rows.iter().map(|item| item.to_string()).join(", ");
            eprintln!("{}", &format!($query, values));
            sqlx::query(&format!($query, values)).execute($pool).await?;
            eprintln!($query, "finished");
        }
    }};
}

#[macro_export]
macro_rules! run_query {
    ($pool: expr, $query: expr $(,)?) => {{
        // TODO find a better way to communicate with the DB
        eprintln!("{}", $query);
        sqlx::query($query).execute($pool).await?;
        eprintln!("{} finished", $query);
    }};
}

// Categories for logging
// TODO naming
pub(crate) const INDEXER: &str = "indexer";
