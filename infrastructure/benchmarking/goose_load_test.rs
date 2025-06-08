use goose::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), GooseError> {
    GooseAttack::initialize()?
        .register_scenario(
            scenario!("HealthCheck")
                .register_transaction(transaction!(health_check).set_weight(1)?),
        )
        .register_scenario(
            scenario!("EventCreation")
                .register_transaction(transaction!(create_event).set_weight(10)?),
        )
        .set_default(GooseDefault::Host, "http://app:8080")?
        .set_default(GooseDefault::Users, 1000)?
        .set_default(GooseDefault::HatchRate, "100/1s")?
        .set_default(GooseDefault::RunTime, 300)?
        .set_default(GooseDefault::LogLevel, 1)?
        .execute()
        .await?;

    Ok(())
}

async fn health_check(user: &mut GooseUser) -> TransactionResult {
    let _response = user.get("/health").await?;
    Ok(())
}

async fn create_event(user: &mut GooseUser) -> TransactionResult {
    let event_payload = serde_json::json!({
        "data": {
            "event_type": "user_action",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "user_id": format!("user_{}", user.weighted_users_index),
            "session_id": format!("session_{}", fastrand::u64(..)),
            "action": "click",
            "element": "button_submit",
            "page": "/dashboard",
            "metadata": {
                "browser": "Chrome",
                "version": "91.0.4472.124",
                "platform": "Linux",
                "screen_resolution": "1920x1080"
            }
        }
    });

    let _response = user
        .post("/api/events")
        .json(&event_payload)
        .await?;

    Ok(())
}