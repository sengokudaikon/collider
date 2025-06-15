use tokio_postgres::{Client, Row};

// Type aliases for PostgreSQL parameter types
pub type PgParam = dyn tokio_postgres::types::ToSql + Sync;
pub type PgSendParam = dyn tokio_postgres::types::ToSql + Sync + Send;
pub type PgParamBox = Box<PgSendParam>;
pub type PgParamVec = Vec<PgParamBox>;

pub async fn execute_with_not_found_check(
    client: &Client, stmt: &tokio_postgres::Statement, params: &[&PgParam],
) -> Result<u64, tokio_postgres::Error> {
    let affected = client.execute(stmt, params).await?;
    Ok(affected)
}

pub fn first_row_or_not_found<T, E, F>(
    rows: &[Row], mapper: F, not_found_error: E,
) -> Result<T, E>
where
    F: FnOnce(&Row) -> T,
{
    rows.first().map(mapper).ok_or(not_found_error)
}

pub async fn count_query(
    client: &Client, table_name: &str,
) -> Result<i64, tokio_postgres::Error> {
    let query = format!("SELECT COUNT(*) FROM {}", table_name);
    let stmt = client.prepare(&query).await?;
    let rows = client.query(&stmt, &[]).await?;
    let count: i64 = rows.first().map(|row| row.get(0)).unwrap_or(0);
    Ok(count)
}

pub fn build_where_clause_with_params<'a>(
    filters: &'a [(&'a str, &'a PgParam)],
) -> (String, Vec<&'a PgParam>) {
    if filters.is_empty() {
        return (String::new(), Vec::new());
    }

    let mut where_clauses = Vec::new();
    let mut params = Vec::new();

    for (i, (column, param)) in filters.iter().enumerate() {
        where_clauses.push(format!("{} = ${}", column, i + 1));
        params.push(*param);
    }

    let where_clause = format!(" WHERE {}", where_clauses.join(" AND "));
    (where_clause, params)
}

pub struct CursorResult<T, C> {
    pub items: Vec<T>,
    pub next_cursor: Option<C>,
}

impl<T, C> CursorResult<T, C> {
    pub fn new(items: Vec<T>, next_cursor: Option<C>) -> Self {
        Self { items, next_cursor }
    }
}
