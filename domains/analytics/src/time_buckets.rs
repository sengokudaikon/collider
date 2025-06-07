use chrono::{DateTime, Datelike, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeBucket {
    Minute,
    Hour,
    Day,
    Week,
    Month,
}

impl TimeBucket {
    pub fn duration(&self) -> Duration {
        match self {
            TimeBucket::Minute => Duration::minutes(1),
            TimeBucket::Hour => Duration::hours(1),
            TimeBucket::Day => Duration::days(1),
            TimeBucket::Week => Duration::weeks(1),
            TimeBucket::Month => Duration::days(30), // Approximate
        }
    }

    pub fn bucket_key(&self, timestamp: DateTime<Utc>) -> String {
        match self {
            TimeBucket::Minute => {
                timestamp.format("%Y-%m-%d:%H:%M").to_string()
            }
            TimeBucket::Hour => timestamp.format("%Y-%m-%d:%H").to_string(),
            TimeBucket::Day => timestamp.format("%Y-%m-%d").to_string(),
            TimeBucket::Week => {
                let week_start = timestamp
                    - Duration::days(
                        timestamp.weekday().num_days_from_monday() as i64,
                    );
                week_start.format("%Y-W%U").to_string()
            }
            TimeBucket::Month => timestamp.format("%Y-%m").to_string(),
        }
    }

    pub fn redis_key_prefix(&self) -> &'static str {
        match self {
            TimeBucket::Minute => "events:minute",
            TimeBucket::Hour => "events:hour",
            TimeBucket::Day => "events:day",
            TimeBucket::Week => "events:week",
            TimeBucket::Month => "events:month",
        }
    }

    pub fn expiry_seconds(&self) -> Option<i64> {
        match self {
            TimeBucket::Minute => Some(3600),    // 1 hour
            TimeBucket::Hour => Some(86400 * 7), // 1 week
            TimeBucket::Day => Some(86400 * 90), // 3 months
            TimeBucket::Week => Some(86400 * 365), // 1 year
            TimeBucket::Month => None,           // No expiry
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BucketMetrics {
    pub total_events: u64,
    pub unique_users: u64,
    pub event_type_counts: std::collections::HashMap<i32, u64>,
    pub properties: std::collections::HashMap<String, serde_json::Value>,
}

impl BucketMetrics {
    pub fn increment_event(&mut self, event_type_id: i32) {
        self.total_events += 1;
        *self.event_type_counts.entry(event_type_id).or_insert(0) += 1;
    }

    pub fn add_user(&mut self, _user_id: uuid::Uuid) {
        // TODO real implementation, you'd track unique users with HyperLogLog
        // For now, just increment (not accurate for unique count)
        self.unique_users += 1;
    }

    pub fn merge(&mut self, other: &BucketMetrics) {
        self.total_events += other.total_events;
        self.unique_users += other.unique_users;

        for (event_type, count) in &other.event_type_counts {
            *self.event_type_counts.entry(*event_type).or_insert(0) += count;
        }

        // Merge properties (simple override for now)
        for (key, value) in &other.properties {
            self.properties.insert(key.clone(), value.clone());
        }
    }
}
