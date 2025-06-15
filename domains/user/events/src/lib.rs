use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Rich analytics events that user domain publishes for analytics consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserAnalyticsEvent {
    UserCreated {
        user_id: Uuid,
        name: String,
        created_at: DateTime<Utc>,
        registration_source: Option<String>,
    },
    UserNameUpdated {
        user_id: Uuid,
        old_name: String,
        new_name: String,
        updated_at: DateTime<Utc>,
    },
    UserDeleted {
        user_id: Uuid,
        deleted_at: DateTime<Utc>,
    },
    UserSessionStart {
        user_id: Uuid,
        session_id: Uuid,
        started_at: DateTime<Utc>,
        user_agent: Option<String>,
        ip_address: Option<String>,
        referrer: Option<String>,
    },
    UserSessionEnd {
        user_id: Uuid,
        session_id: Uuid,
        ended_at: DateTime<Utc>,
        duration_seconds: i64,
    },
}

impl UserAnalyticsEvent {
    /// Convert UserAnalyticsEvent to CreateEventCommand for storage in events
    /// table
    pub fn to_create_event_command(
        &self,
    ) -> events_commands::CreateEventCommand {
        match self {
            UserAnalyticsEvent::UserCreated {
                user_id,
                name,
                created_at,
                registration_source,
            } => {
                events_commands::CreateEventCommand {
                    user_id: *user_id,
                    event_type: "user_created".to_string(),
                    timestamp: Some(*created_at),
                    metadata: Some(serde_json::json!({
                        "name": name,
                        "registration_source": registration_source
                    })),
                }
            }
            UserAnalyticsEvent::UserNameUpdated {
                user_id,
                old_name,
                new_name,
                updated_at,
            } => {
                events_commands::CreateEventCommand {
                    user_id: *user_id,
                    event_type: "user_name_updated".to_string(),
                    timestamp: Some(*updated_at),
                    metadata: Some(serde_json::json!({
                        "old_name": old_name,
                        "new_name": new_name
                    })),
                }
            }
            UserAnalyticsEvent::UserDeleted {
                user_id,
                deleted_at,
            } => {
                events_commands::CreateEventCommand {
                    user_id: *user_id,
                    event_type: "user_deleted".to_string(),
                    timestamp: Some(*deleted_at),
                    metadata: None,
                }
            }
            UserAnalyticsEvent::UserSessionStart {
                user_id,
                session_id,
                started_at,
                user_agent,
                ip_address,
                referrer,
            } => {
                events_commands::CreateEventCommand {
                    user_id: *user_id,
                    event_type: "user_session_start".to_string(),
                    timestamp: Some(*started_at),
                    metadata: Some(serde_json::json!({
                        "session_id": session_id,
                        "user_agent": user_agent,
                        "ip_address": ip_address,
                        "referrer": referrer
                    })),
                }
            }
            UserAnalyticsEvent::UserSessionEnd {
                user_id,
                session_id,
                ended_at,
                duration_seconds,
            } => {
                events_commands::CreateEventCommand {
                    user_id: *user_id,
                    event_type: "user_session_end".to_string(),
                    timestamp: Some(*ended_at),
                    metadata: Some(serde_json::json!({
                        "session_id": session_id,
                        "duration_seconds": duration_seconds
                    })),
                }
            }
        }
    }
}
