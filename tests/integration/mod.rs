// Integration tests have been moved to their appropriate persistence library locations:
// - Redis tests: libs/persistence/redis_connection/tests/
// - SQL tests: libs/persistence/sql_connection/tests/

// Common test configurations
pub const TEST_DATABASE_NAME: &str = "collider_test";
pub const TEST_REDIS_DB: u8 = 15; // Use a separate DB for tests