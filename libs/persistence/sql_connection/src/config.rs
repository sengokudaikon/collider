pub trait DbConnectConfig: serde::de::DeserializeOwned {
    fn scheme(&self) -> &str;
    fn username(&self) -> &str;
    fn password(&self) -> &str;
    fn host(&self) -> &str;
    fn port(&self) -> u16;
    fn name(&self) -> &str;

    fn uri(&self) -> &str { "" }
}

pub trait DbOptionsConfig {
    fn max_conn(&self) -> Option<u32> { None }
    fn min_conn(&self) -> Option<u32> { None }
    fn sql_logger(&self) -> bool { false }
}

pub trait ReadReplicaConfig {
    fn read_replica_uri(&self) -> Option<&str>;
    fn read_max_conn(&self) -> Option<u32>;
    fn read_min_conn(&self) -> Option<u32>;
    fn enable_read_write_split(&self) -> bool;
}
#[derive(Debug, serde::Deserialize)]
pub struct PostgresDbConfig {
    pub uri: String,
    pub max_conn: Option<u32>,
    pub min_conn: Option<u32>,
    #[serde(default = "logger_default")]
    pub logger: bool,
    // Read replica configuration
    pub read_replica_uri: Option<String>,
    pub read_max_conn: Option<u32>,
    pub read_min_conn: Option<u32>,
    #[serde(default = "read_write_split_default")]
    pub enable_read_write_split: bool,
}

fn read_write_split_default() -> bool { false }

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

impl ReadReplicaConfig for PostgresDbConfig {
    fn read_replica_uri(&self) -> Option<&str> {
        self.read_replica_uri.as_deref()
    }

    fn read_max_conn(&self) -> Option<u32> { self.read_max_conn }

    fn read_min_conn(&self) -> Option<u32> { self.read_min_conn }

    fn enable_read_write_split(&self) -> bool {
        self.enable_read_write_split && self.read_replica_uri.is_some()
    }
}
fn logger_default() -> bool { false }
