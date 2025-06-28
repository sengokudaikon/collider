use tokio_postgres::types::ToSql;

#[derive(Debug, Clone)]
pub struct PaginationParams {
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

impl PaginationParams {
    pub fn new(limit: Option<u64>, offset: Option<u64>) -> Self {
        Self { limit, offset }
    }

    pub fn build_query_parts(
        &self, base_query: &str, order_by: &str,
    ) -> (String, Vec<i64>) {
        let mut query = format!("{base_query} {order_by}");
        let mut params = Vec::new();
        let mut param_count = 0;

        match (self.limit, self.offset) {
            (Some(l), Some(o)) => {
                param_count += 2;
                query.push_str(&format!(
                    " LIMIT ${} OFFSET ${}",
                    param_count - 1,
                    param_count
                ));
                params.extend([l as i64, o as i64]);
            }
            (Some(l), None) => {
                param_count += 1;
                query.push_str(&format!(" LIMIT ${param_count}"));
                params.push(l as i64);
            }
            (None, Some(o)) => {
                param_count += 1;
                query.push_str(&format!(" OFFSET ${param_count}"));
                params.push(o as i64);
            }
            (None, None) => {}
        }

        (query, params)
    }

    pub fn build_query_with_existing_params(
        &self, base_query: &str, order_by: &str, existing_param_count: usize,
    ) -> (String, Vec<i64>) {
        let mut query = format!("{base_query} {order_by}");
        let mut params = Vec::new();
        let mut param_count = existing_param_count;

        match (self.limit, self.offset) {
            (Some(l), Some(o)) => {
                param_count += 1;
                query.push_str(&format!(" LIMIT ${param_count}"));
                param_count += 1;
                query.push_str(&format!(" OFFSET ${param_count}"));
                params.extend([l as i64, o as i64]);
            }
            (Some(l), None) => {
                param_count += 1;
                query.push_str(&format!(" LIMIT ${param_count}"));
                params.push(l as i64);
            }
            (None, Some(o)) => {
                param_count += 1;
                query.push_str(&format!(" OFFSET ${param_count}"));
                params.push(o as i64);
            }
            (None, None) => {}
        }

        (query, params)
    }
}

#[derive(Debug, Clone)]
pub struct CursorPagination<T> {
    pub cursor: Option<T>,
    pub limit: u64,
}

impl<T> CursorPagination<T> {
    pub fn new(cursor: Option<T>, limit: u64) -> Self {
        Self {
            cursor,
            limit: limit.min(1000), // Cap at 1000
        }
    }

    pub fn limit_plus_one(&self) -> i64 { self.limit as i64 + 1 }
}

pub fn create_param_refs<T: ToSql + Sync>(
    params: &[T],
) -> Vec<&(dyn ToSql + Sync)> {
    params.iter().map(|p| p as &(dyn ToSql + Sync)).collect()
}
