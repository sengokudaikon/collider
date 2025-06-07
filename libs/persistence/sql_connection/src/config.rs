pub trait DbConnectConfig: serde::de::DeserializeOwned {
    fn scheme(&self) -> &str;
    fn username(&self) -> &str;
    fn password(&self) -> &str;
    fn host(&self) -> &str;
    fn port(&self) -> u16;
    fn name(&self) -> &str;

    fn uri(&self) -> &str { "" }
}

/// Configure database connection pool data
pub trait DbOptionsConfig {
    fn max_conn(&self) -> Option<u32> { None }
    fn min_conn(&self) -> Option<u32> { None }
    fn sql_logger(&self) -> bool { false }
}

#[derive(Debug, serde::Deserialize)]
pub struct PostgresDbConfig {
    pub uri: String,
    pub max_conn: Option<u32>,
    pub min_conn: Option<u32>,
    #[serde(default = "logger_default")]
    pub logger: bool,
}

impl DbConnectConfig for PostgresDbConfig {
    fn scheme(&self) -> &str { "postgresql" }

    fn username(&self) -> &str { "" }

    fn password(&self) -> &str { "" }

    fn host(&self) -> &str { "" }

    fn port(&self) -> u16 { 5432 }

    fn name(&self) -> &str { "" }

    fn uri(&self) -> &str { &self.uri }
}

impl DbOptionsConfig for PostgresDbConfig {
    fn max_conn(&self) -> Option<u32> { self.max_conn }

    fn min_conn(&self) -> Option<u32> { self.min_conn }

    fn sql_logger(&self) -> bool { self.logger }
}

fn logger_default() -> bool { false }
