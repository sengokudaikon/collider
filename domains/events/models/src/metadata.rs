use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};
use utoipa::ToSchema;

/// Single flattened metadata type that matches materialized view structure
/// exactly
#[derive(
    Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq, Default,
)]
pub struct Metadata {
    /// The page or URL where the event occurred
    pub page: Option<String>,
    /// The referring URL or source
    pub referrer: Option<String>,
    /// Session identifier for grouping related events
    pub session_id: Option<String>,

    /// Product identifier (ecommerce events)
    pub product_id: Option<i32>,
    /// Price in cents (ecommerce events)
    pub price: Option<i64>,
    /// Currency code (ecommerce events)
    pub currency: Option<String>,
    /// Cart total in cents (ecommerce events)
    pub cart_total: Option<i64>,
    /// Order identifier (ecommerce events)
    pub order_id: Option<String>,
    /// Product category (ecommerce events)
    pub category: Option<String>,
    /// Quantity of items (ecommerce events)
    pub quantity: Option<i32>,

    /// User agent string (user events)
    pub user_agent: Option<String>,
    /// IP address (user events)
    pub ip_address: Option<String>,
    /// Device type (user events)
    pub device_type: Option<String>,
    /// Geographic location (user events)
    pub location: Option<String>,

    /// API endpoint path (api events)
    pub endpoint: Option<String>,
    /// HTTP method (api events)
    pub method: Option<String>,
    /// HTTP response status code (api events)
    pub response_code: Option<i32>,
    /// Response time in milliseconds (api events)
    pub response_time_ms: Option<i32>,
    /// Request size in bytes (api events)
    pub request_size: Option<i64>,
    /// Response size in bytes (api events)
    pub response_size: Option<i64>,
    /// API version (api events)
    pub api_version: Option<String>,

    /// A/B test variant identifier (analytics events)
    pub variant: Option<String>,
    /// Campaign identifier (analytics events)
    pub campaign_id: Option<String>,
    /// UTM source (analytics events)
    pub utm_source: Option<String>,
    /// UTM medium (analytics events)
    pub utm_medium: Option<String>,
    /// UTM campaign (analytics events)
    pub utm_campaign: Option<String>,
    /// Conversion value in cents (analytics events)
    pub conversion_value: Option<i64>,
}

impl ToSql for Metadata {
    tokio_postgres::types::to_sql_checked!();

    fn to_sql(
        &self, _ty: &Type, out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        let json = serde_json::to_value(self)?;
        json.to_sql(&Type::JSONB, out)
    }

    fn accepts(ty: &Type) -> bool { ty == &Type::JSONB || ty == &Type::JSON }
}

impl<'a> FromSql<'a> for Metadata {
    fn from_sql(
        _ty: &Type, raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        let json = serde_json::Value::from_sql(&Type::JSONB, raw)?;
        let metadata = serde_json::from_value(json)?;
        Ok(metadata)
    }

    fn accepts(ty: &Type) -> bool { ty == &Type::JSONB || ty == &Type::JSON }
}

impl Metadata {
    /// Create metadata for user events
    pub fn user() -> Self { Self::default() }

    /// Create metadata for ecommerce events
    pub fn ecommerce() -> Self { Self::default() }

    /// Create metadata for API events
    pub fn api() -> Self { Self::default() }

    /// Create metadata for analytics events
    pub fn analytics() -> Self { Self::default() }

    /// Validate the metadata structure
    pub fn validate(&self) -> Result<(), MetadataValidationError> {
        if let Some(ref referrer) = self.referrer {
            if !referrer.starts_with("http://")
                && !referrer.starts_with("https://")
            {
                return Err(MetadataValidationError::InvalidUrl(
                    referrer.clone(),
                ));
            }
        }

        if let Some(product_id) = self.product_id {
            if product_id <= 0 {
                return Err(MetadataValidationError::InvalidProductId);
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MetadataValidationError {
    InvalidUrl(String),
    InvalidProductId,
    InvalidSessionId,
    RequiredFieldMissing(String),
    InvalidValue(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_validation() {
        let metadata = Metadata {
            page: Some("/test".to_string()),
            referrer: Some("https://example.com".to_string()),
            session_id: Some("12345".to_string()),
            ..Default::default()
        };

        assert!(metadata.validate().is_ok());
    }

    #[test]
    fn test_invalid_referrer() {
        let metadata = Metadata {
            referrer: Some("invalid-url".to_string()),
            ..Default::default()
        };

        assert!(metadata.validate().is_err());
    }

    #[test]
    fn test_ecommerce_metadata() {
        let mut metadata = Metadata::ecommerce();
        metadata.page = Some("/product".to_string());
        metadata.product_id = Some(42);
        metadata.price = Some(1999);
        metadata.currency = Some("USD".to_string());
        metadata.category = Some("electronics".to_string());
        metadata.quantity = Some(1);

        assert!(metadata.validate().is_ok());
    }

    #[test]
    fn test_invalid_product_id() {
        let mut metadata = Metadata::ecommerce();
        metadata.product_id = Some(0);

        assert!(metadata.validate().is_err());
    }
}
