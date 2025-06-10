use anyhow::{Result, anyhow};
use tracing::{warn, info};

pub fn handle_seeder_error(error: anyhow::Error, seeder_name: &str) -> anyhow::Error {
    let error_msg = error.to_string().to_lowercase();
    
    if error_msg.contains("duplicate key") && error_msg.contains("unique constraint") {
        if error_msg.contains("event_types_name_key") {
            warn!("Event types already exist in database - this may indicate the database already has seeded data");
            info!("Tip: Use a fresh database or run migrations to clear existing data");
            anyhow!("Database already contains event types. Use a fresh database or clear existing data.")
        } else if error_msg.contains("users") {
            warn!("Users already exist in database - this may indicate the database already has seeded data");
            info!("Tip: Use a fresh database or run migrations to clear existing data");
            anyhow!("Database already contains users. Use a fresh database or clear existing data.")
        } else {
            warn!("Database constraint violation in {}: duplicate data detected", seeder_name);
            info!("Tip: Use a fresh database or clear existing data before seeding");
            anyhow!("Database constraint violation: {}. Use a fresh database.", error)
        }
    } else if error_msg.contains("connection") {
        anyhow!("Database connection error in {}: {}. Check your DATABASE_URL and ensure the database is running.", seeder_name, error)
    } else if error_msg.contains("timeout") {
        anyhow!("Database operation timed out in {}: {}. Consider reducing batch size or checking database performance.", seeder_name, error)
    } else {
        // Return original error for other cases
        anyhow!("Seeder '{}' failed: {}", seeder_name, error)
    }
}