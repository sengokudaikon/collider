use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};
use utoipa::ToSchema;
#[derive(
    Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq, Default,
)]
pub struct Metadata {
    /// The page or URL where the event occurred (used by page_analytics)
    pub page: Option<String>,
    /// The referring URL or source (used by referrer_analytics)
    pub referrer: Option<String>,
    /// Session identifier for grouping related events (used by
    /// page_analytics, referrer_analytics)
    pub session_id: Option<String>,
    /// Product identifier for ecommerce events (used by product_analytics)
    pub product_id: Option<i32>,
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
        metadata.referrer = Some("https://example.com".to_string());
        metadata.session_id = Some("12345".to_string());

        assert!(metadata.validate().is_ok());
    }

    #[test]
    fn test_invalid_product_id() {
        let mut metadata = Metadata::ecommerce();
        metadata.product_id = Some(0);

        assert!(metadata.validate().is_err());
    }
}
