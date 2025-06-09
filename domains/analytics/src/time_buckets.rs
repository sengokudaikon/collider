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
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::{Datelike, TimeZone, Weekday};
    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn test_time_bucket_duration() {
        assert_eq!(TimeBucket::Minute.duration(), Duration::minutes(1));
        assert_eq!(TimeBucket::Hour.duration(), Duration::hours(1));
        assert_eq!(TimeBucket::Day.duration(), Duration::days(1));
        assert_eq!(TimeBucket::Week.duration(), Duration::weeks(1));
        assert_eq!(TimeBucket::Month.duration(), Duration::days(30));
    }

    #[test]
    fn test_time_bucket_key_minute() {
        let timestamp =
            Utc.with_ymd_and_hms(2024, 1, 15, 14, 30, 45).unwrap();
        let key = TimeBucket::Minute.bucket_key(timestamp);
        assert_eq!(key, "2024-01-15:14:30");
    }

    #[test]
    fn test_time_bucket_key_hour() {
        let timestamp =
            Utc.with_ymd_and_hms(2024, 1, 15, 14, 30, 45).unwrap();
        let key = TimeBucket::Hour.bucket_key(timestamp);
        assert_eq!(key, "2024-01-15:14");
    }

    #[test]
    fn test_time_bucket_key_day() {
        let timestamp =
            Utc.with_ymd_and_hms(2024, 1, 15, 14, 30, 45).unwrap();
        let key = TimeBucket::Day.bucket_key(timestamp);
        assert_eq!(key, "2024-01-15");
    }

    #[test]
    fn test_time_bucket_key_week() {
        // January 15, 2024 is a Monday
        let timestamp =
            Utc.with_ymd_and_hms(2024, 1, 15, 14, 30, 45).unwrap();
        assert_eq!(timestamp.weekday(), Weekday::Mon);

        let key = TimeBucket::Week.bucket_key(timestamp);
        assert!(key.starts_with("2024-W"));
    }

    #[test]
    fn test_time_bucket_key_month() {
        let timestamp =
            Utc.with_ymd_and_hms(2024, 1, 15, 14, 30, 45).unwrap();
        let key = TimeBucket::Month.bucket_key(timestamp);
        assert_eq!(key, "2024-01");
    }

    #[test]
    fn test_redis_key_prefix() {
        assert_eq!(TimeBucket::Minute.redis_key_prefix(), "events:minute");
        assert_eq!(TimeBucket::Hour.redis_key_prefix(), "events:hour");
        assert_eq!(TimeBucket::Day.redis_key_prefix(), "events:day");
        assert_eq!(TimeBucket::Week.redis_key_prefix(), "events:week");
        assert_eq!(TimeBucket::Month.redis_key_prefix(), "events:month");
    }

    #[test]
    fn test_expiry_seconds() {
        assert_eq!(TimeBucket::Minute.expiry_seconds(), Some(3600));
        assert_eq!(TimeBucket::Hour.expiry_seconds(), Some(86400 * 7));
        assert_eq!(TimeBucket::Day.expiry_seconds(), Some(86400 * 90));
        assert_eq!(TimeBucket::Week.expiry_seconds(), Some(86400 * 365));
        assert_eq!(TimeBucket::Month.expiry_seconds(), None);
    }

    #[test]
    fn test_bucket_metrics_default() {
        let metrics = BucketMetrics::default();
        assert_eq!(metrics.total_events, 0);
        assert_eq!(metrics.unique_users, 0);
        assert!(metrics.event_type_counts.is_empty());
        assert!(metrics.properties.is_empty());
        assert!(metrics.user_hll.is_some());
    }

    #[test]
    fn test_bucket_metrics_increment_event() {
        let mut metrics = BucketMetrics::default();

        metrics.increment_event(1);
        assert_eq!(metrics.total_events, 1);
        assert_eq!(metrics.event_type_counts.get(&1), Some(&1));

        metrics.increment_event(1);
        assert_eq!(metrics.total_events, 2);
        assert_eq!(metrics.event_type_counts.get(&1), Some(&2));

        metrics.increment_event(2);
        assert_eq!(metrics.total_events, 3);
        assert_eq!(metrics.event_type_counts.get(&1), Some(&2));
        assert_eq!(metrics.event_type_counts.get(&2), Some(&1));
    }

    #[test]
    fn test_bucket_metrics_add_user() {
        let mut metrics = BucketMetrics::default();
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        metrics.add_user(user1);
        assert_eq!(metrics.unique_users, 1);

        // Adding same user should not increase count significantly (HLL
        // approximation)
        metrics.add_user(user1);
        assert_eq!(metrics.unique_users, 1);

        // Adding different user should increase count
        metrics.add_user(user2);
        assert!(metrics.unique_users >= 2);
    }

    #[test]
    fn test_bucket_metrics_add_user_without_hll() {
        let mut metrics = BucketMetrics {
            total_events: 0,
            unique_users: 5,
            event_type_counts: HashMap::new(),
            properties: HashMap::new(),
            user_hll: None,
        };

        let user = Uuid::new_v4();
        metrics.add_user(user);
        assert_eq!(metrics.unique_users, 6);
    }

    #[test]
    fn test_bucket_metrics_merge_with_hll() {
        let mut metrics1 = BucketMetrics::default();
        let mut metrics2 = BucketMetrics::default();

        // Add events and users to first metrics
        metrics1.increment_event(1);
        metrics1.increment_event(2);
        metrics1.add_user(Uuid::new_v4());

        // Add different events and users to second metrics
        metrics2.increment_event(2);
        metrics2.increment_event(3);
        metrics2.add_user(Uuid::new_v4());

        let original_total1 = metrics1.total_events;
        let original_total2 = metrics2.total_events;

        metrics1.merge(&metrics2);

        assert_eq!(metrics1.total_events, original_total1 + original_total2);
        assert_eq!(metrics1.event_type_counts.get(&1), Some(&1));
        assert_eq!(metrics1.event_type_counts.get(&2), Some(&2));
        assert_eq!(metrics1.event_type_counts.get(&3), Some(&1));
        assert!(metrics1.unique_users >= 2);
    }

    #[test]
    fn test_bucket_metrics_merge_without_hll() {
        let mut metrics1 = BucketMetrics {
            total_events: 5,
            unique_users: 3,
            event_type_counts: HashMap::from([(1, 2), (2, 3)]),
            properties: HashMap::from([(
                "key1".to_string(),
                json!("value1"),
            )]),
            user_hll: None,
        };

        let metrics2 = BucketMetrics {
            total_events: 3,
            unique_users: 2,
            event_type_counts: HashMap::from([(2, 1), (3, 2)]),
            properties: HashMap::from([(
                "key2".to_string(),
                json!("value2"),
            )]),
            user_hll: None,
        };

        metrics1.merge(&metrics2);

        assert_eq!(metrics1.total_events, 8);
        assert_eq!(metrics1.unique_users, 5);
        assert_eq!(metrics1.event_type_counts.get(&1), Some(&2));
        assert_eq!(metrics1.event_type_counts.get(&2), Some(&4));
        assert_eq!(metrics1.event_type_counts.get(&3), Some(&2));
        assert_eq!(metrics1.properties.len(), 2);
        assert_eq!(metrics1.properties.get("key1"), Some(&json!("value1")));
        assert_eq!(metrics1.properties.get("key2"), Some(&json!("value2")));
    }

    #[test]
    fn test_bucket_metrics_merge_mixed_hll() {
        let mut metrics1 = BucketMetrics::default();
        let metrics2 = BucketMetrics {
            total_events: 3,
            unique_users: 2,
            event_type_counts: HashMap::from([(1, 3)]),
            properties: HashMap::new(),
            user_hll: None,
        };

        metrics1.add_user(Uuid::new_v4());
        let original_unique = metrics1.unique_users;

        metrics1.merge(&metrics2);

        assert_eq!(metrics1.total_events, 3);
        assert_eq!(metrics1.unique_users, original_unique);
        assert!(metrics1.user_hll.is_some());
    }

    #[test]
    fn test_bucket_metrics_merge_hll_to_none() {
        let mut metrics1 = BucketMetrics {
            total_events: 2,
            unique_users: 1,
            event_type_counts: HashMap::new(),
            properties: HashMap::new(),
            user_hll: None,
        };

        let mut metrics2 = BucketMetrics::default();
        metrics2.add_user(Uuid::new_v4());
        metrics2.add_user(Uuid::new_v4());

        let other_unique = metrics2.unique_users;

        metrics1.merge(&metrics2);

        assert_eq!(metrics1.total_events, 2);
        assert_eq!(metrics1.unique_users, other_unique);
        assert!(metrics1.user_hll.is_some());
    }

    #[test]
    fn test_bucket_metrics_serialization() {
        let mut metrics = BucketMetrics::default();
        metrics.increment_event(1);
        metrics.add_user(Uuid::new_v4());
        metrics
            .properties
            .insert("test_key".to_string(), json!("test_value"));

        let serialized = serde_json::to_string(&metrics).unwrap();
        assert!(serialized.contains("total_events"));
        assert!(serialized.contains("unique_users"));
        assert!(serialized.contains("event_type_counts"));
        assert!(serialized.contains("properties"));
        assert!(serialized.contains("test_key"));
        assert!(serialized.contains("test_value"));
    }

    #[test]
    fn test_bucket_metrics_deserialization() {
        let json_data = r#"{
            "total_events": 10,
            "unique_users": 5,
            "event_type_counts": {"1": 3, "2": 7},
            "properties": {"key1": "value1", "key2": 42}
        }"#;

        let metrics: BucketMetrics = serde_json::from_str(json_data).unwrap();

        assert_eq!(metrics.total_events, 10);
        assert_eq!(metrics.unique_users, 5);
        assert_eq!(metrics.event_type_counts.get(&1), Some(&3));
        assert_eq!(metrics.event_type_counts.get(&2), Some(&7));
        assert_eq!(metrics.properties.get("key1"), Some(&json!("value1")));
        assert_eq!(metrics.properties.get("key2"), Some(&json!(42)));
        assert!(metrics.user_hll.is_some());
    }

    #[test]
    fn test_time_bucket_edge_cases() {
        // Test edge cases for different time buckets

        // Start of year
        let start_of_year =
            Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        assert_eq!(TimeBucket::Month.bucket_key(start_of_year), "2024-01");
        assert_eq!(TimeBucket::Day.bucket_key(start_of_year), "2024-01-01");

        // End of year
        let end_of_year =
            Utc.with_ymd_and_hms(2024, 12, 31, 23, 59, 59).unwrap();
        assert_eq!(TimeBucket::Month.bucket_key(end_of_year), "2024-12");
        assert_eq!(TimeBucket::Day.bucket_key(end_of_year), "2024-12-31");

        // Leap year February
        let leap_feb = Utc.with_ymd_and_hms(2024, 2, 29, 12, 0, 0).unwrap();
        assert_eq!(TimeBucket::Month.bucket_key(leap_feb), "2024-02");
        assert_eq!(TimeBucket::Day.bucket_key(leap_feb), "2024-02-29");
    }

    #[test]
    fn test_time_bucket_week_consistency() {
        // Test that week bucket keys are consistent across different days of
        // the same week
        let monday = Utc.with_ymd_and_hms(2024, 1, 15, 12, 0, 0).unwrap(); // Monday
        let tuesday = Utc.with_ymd_and_hms(2024, 1, 16, 12, 0, 0).unwrap(); // Tuesday
        let sunday = Utc.with_ymd_and_hms(2024, 1, 21, 12, 0, 0).unwrap(); // Sunday

        let monday_key = TimeBucket::Week.bucket_key(monday);
        let tuesday_key = TimeBucket::Week.bucket_key(tuesday);
        let sunday_key = TimeBucket::Week.bucket_key(sunday);

        assert_eq!(monday_key, tuesday_key);
        assert_eq!(tuesday_key, sunday_key);
    }

    #[test]
    fn test_bucket_metrics_large_numbers() {
        let mut metrics = BucketMetrics::default();

        // Test with large numbers
        for _ in 0..1000 {
            metrics.increment_event(1);
        }

        assert_eq!(metrics.total_events, 1000);
        assert_eq!(metrics.event_type_counts.get(&1), Some(&1000));

        // Add many unique users
        for i in 0..100 {
            let user_id = Uuid::from_u128(i);
            metrics.add_user(user_id);
        }

        // HyperLogLog should approximate 100 unique users
        assert!(metrics.unique_users >= 90 && metrics.unique_users <= 110);
    }

    #[test]
    fn test_bucket_metrics_properties_overwrite() {
        let mut metrics1 = BucketMetrics::default();
        metrics1
            .properties
            .insert("shared_key".to_string(), json!("value1"));
        metrics1
            .properties
            .insert("unique_key1".to_string(), json!("unique1"));

        let mut metrics2 = BucketMetrics::default();
        metrics2
            .properties
            .insert("shared_key".to_string(), json!("value2"));
        metrics2
            .properties
            .insert("unique_key2".to_string(), json!("unique2"));

        metrics1.merge(&metrics2);

        // shared_key should be overwritten with value2
        assert_eq!(
            metrics1.properties.get("shared_key"),
            Some(&json!("value2"))
        );
        assert_eq!(
            metrics1.properties.get("unique_key1"),
            Some(&json!("unique1"))
        );
        assert_eq!(
            metrics1.properties.get("unique_key2"),
            Some(&json!("unique2"))
        );
        assert_eq!(metrics1.properties.len(), 3);
    }

    #[test]
    fn test_time_bucket_hash_eq() {
        // Test that TimeBucket implements Hash and Eq correctly
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(TimeBucket::Minute);
        set.insert(TimeBucket::Hour);
        set.insert(TimeBucket::Minute); // Duplicate

        assert_eq!(set.len(), 2);
        assert!(set.contains(&TimeBucket::Minute));
        assert!(set.contains(&TimeBucket::Hour));
        assert!(!set.contains(&TimeBucket::Day));
    }
}
