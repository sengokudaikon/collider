use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use chrono::{DateTime, Datelike, Duration, Utc};
use hyperloglogplus::HyperLogLog;
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
            TimeBucket::Month => Duration::days(30),
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
            TimeBucket::Minute => Some(3600),
            TimeBucket::Hour => Some(86400 * 7),
            TimeBucket::Day => Some(86400 * 90),
            TimeBucket::Week => Some(86400 * 365),
            TimeBucket::Month => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BucketMetrics {
    pub total_events: u64,
    pub unique_users: u64,
    pub event_type_counts: std::collections::HashMap<i32, u64>,
    pub properties: std::collections::HashMap<String, serde_json::Value>,
    user_hll:
        Option<hyperloglogplus::HyperLogLogPlus<u64, fnv::FnvBuildHasher>>,
}

impl Default for BucketMetrics {
    fn default() -> Self {
        Self {
            total_events: 0,
            unique_users: 0,
            event_type_counts: std::collections::HashMap::new(),
            properties: std::collections::HashMap::new(),
            user_hll: Some(
                hyperloglogplus::HyperLogLogPlus::new(
                    14,
                    fnv::FnvBuildHasher::default(),
                )
                .unwrap(),
            ),
        }
    }
}

impl Serialize for BucketMetrics {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("BucketMetrics", 4)?;
        state.serialize_field("total_events", &self.total_events)?;
        state.serialize_field("unique_users", &self.unique_users)?;
        state
            .serialize_field("event_type_counts", &self.event_type_counts)?;
        state.serialize_field("properties", &self.properties)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for BucketMetrics {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct BucketMetricsHelper {
            total_events: u64,
            unique_users: u64,
            event_type_counts: std::collections::HashMap<i32, u64>,
            properties: std::collections::HashMap<String, serde_json::Value>,
        }

        let helper = BucketMetricsHelper::deserialize(deserializer)?;
        Ok(BucketMetrics {
            total_events: helper.total_events,
            unique_users: helper.unique_users,
            event_type_counts: helper.event_type_counts,
            properties: helper.properties,
            user_hll: Some(
                hyperloglogplus::HyperLogLogPlus::new(
                    14,
                    fnv::FnvBuildHasher::default(),
                )
                .unwrap(),
            ),
        })
    }
}

impl BucketMetrics {
    pub fn increment_event(&mut self, event_type_id: i32) {
        self.total_events += 1;
        *self.event_type_counts.entry(event_type_id).or_insert(0) += 1;
    }

    pub fn add_user(&mut self, user_id: uuid::Uuid) {
        if let Some(ref mut hll) = self.user_hll {
            let mut hasher = DefaultHasher::new();
            user_id.hash(&mut hasher);
            let hash = hasher.finish();

            hll.insert(&hash);
            self.unique_users = hll.count() as u64;
        }
        else {
            self.unique_users += 1;
        }
    }

    pub fn merge(&mut self, other: &BucketMetrics) {
        self.total_events += other.total_events;

        match (&mut self.user_hll, &other.user_hll) {
            (Some(self_hll), Some(other_hll)) => {
                if self_hll.merge(other_hll).is_err() {
                    let self_count = self_hll.count() as u64;
                    self.unique_users =
                        std::cmp::max(self_count, other.unique_users);
                }
                else {
                    self.unique_users = self_hll.count() as u64;
                }
            }
            (Some(self_hll), None) => {
                self.unique_users = self_hll.count() as u64;
            }
            (None, Some(other_hll)) => {
                let mut cloned_hll = other_hll.clone();
                self.unique_users = cloned_hll.count() as u64;
                self.user_hll = Some(cloned_hll);
            }
            (None, None) => {
                self.unique_users += other.unique_users;
            }
        }

        for (event_type, count) in &other.event_type_counts {
            *self.event_type_counts.entry(*event_type).or_insert(0) += count;
        }

        for (key, value) in &other.properties {
            self.properties.insert(key.clone(), value.clone());
        }
    }
}
