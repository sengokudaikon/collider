# API Documentation

Collider provides a high-performance REST API for event tracking and analytics built with Rust and Axum.

## Base URL

```
http://localhost:8080
```

## Authentication

Currently, the API operates without authentication. In production, you should implement proper authentication and authorization.

## Interactive Documentation

The API includes interactive documentation powered by RapiDoc:

- **API Docs**: `http://localhost:8080/docs`
- **OpenAPI Spec**: `http://localhost:8080/api-docs/openapi.json`

## Health Check

### GET /health

Check if the service is running.

**Response:**
```
200 OK
```

## Users API

Base path: `/api/users`

### List Users

**GET** `/api/users`

Query parameters:
- `include_metrics` (boolean, default: false) - Include user analytics metrics
- `limit` (integer) - Maximum number of users to return
- `offset` (integer) - Number of users to skip

**Example:**
```bash
curl "http://localhost:8080/api/users?include_metrics=true&limit=10"
```

### Create User

**POST** `/api/users`

**Request Body:**
```json
{
  "username": "john_doe",
  "full_name": "John Doe"
}
```

**Response:**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "username": "john_doe",
  "full_name": "John Doe",
  "created_at": "2024-01-15T10:30:00Z",
  "updated_at": "2024-01-15T10:30:00Z"
}
```

### Get User by ID

**GET** `/api/users/{id}`

Query parameters:
- `include_metrics` (boolean, default: false) - Include user analytics metrics

**Example:**
```bash
curl "http://localhost:8080/api/users/550e8400-e29b-41d4-a716-446655440000?include_metrics=true"
```

### Get User by Username

**GET** `/api/users/by-name/{username}`

**Example:**
```bash
curl "http://localhost:8080/api/users/by-name/john_doe"
```

### Update User

**PUT** `/api/users/{id}`

**Request Body:**
```json
{
  "name": "John Doe Updated"
}
```

### Delete User

**DELETE** `/api/users/{id}`

**Response:**
```
204 No Content
```

### Get User Metrics

**GET** `/api/users/{id}/metrics`

Returns detailed analytics metrics for a specific user.

## Events API

Base path: `/api/events`

### List Events

**GET** `/api/events`

Query parameters:
- `user_id` (UUID) - Filter by user ID
- `event_type_id` (integer) - Filter by event type ID
- `limit` (integer, max: 1000, default: 100) - Number of events to return
- `offset` (integer) - Number of events to skip
- `page` (integer) - Page number (alternative to offset)

**Example:**
```bash
curl "http://localhost:8080/api/events?user_id=550e8400-e29b-41d4-a716-446655440000&limit=50"
```

### Create Event

**POST** `/api/events`

**Request Body:**
```json
{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "event_type_id": 1,
  "metadata": {
    "page": "/home",
    "action": "click",
    "element": "button"
  }
}
```

**Response:**
```json
{
  "id": "660e8400-e29b-41d4-a716-446655440001",
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "event_type_id": 1,
  "metadata": {
    "page": "/home",
    "action": "click",
    "element": "button"
  },
  "created_at": "2024-01-15T10:30:00Z",
  "updated_at": "2024-01-15T10:30:00Z"
}
```

### Get Event by ID

**GET** `/api/events/{id}`

**Example:**
```bash
curl "http://localhost:8080/api/events/660e8400-e29b-41d4-a716-446655440001"
```

### Update Event

**PUT** `/api/events/{id}`

**Request Body:**
```json
{
  "event_type_id": 2,
  "metadata": {
    "page": "/about",
    "action": "view"
  }
}
```

### Delete Event

**DELETE** `/api/events/{id}`

**Response:**
```
204 No Content
```

### Bulk Delete Events

**DELETE** `/api/events?before={timestamp}`

Delete events created before a specific timestamp.

**Query Parameters:**
- `before` (ISO 8601 timestamp) - Delete events created before this time

**Example:**
```bash
curl -X DELETE "http://localhost:8080/api/events?before=2024-01-01T00:00:00Z"
```

## Analytics API

Base path: `/api/analytics`

### Get Statistics

**GET** `/api/analytics/stats`

Query parameters:
- `from` (ISO 8601 timestamp) - Start time (default: 24 hours ago)
- `to` (ISO 8601 timestamp) - End time (default: now)
- `type` (string) - Filter by event type

**Response:**
```json
{
  "total_events": 1000000,
  "unique_users": 50000,
  "top_pages": {
    "/home": 500000,
    "/about": 250000,
    "/contact": 100000
  },
  "period": {
    "from": "2024-01-14T10:30:00Z",
    "to": "2024-01-15T10:30:00Z"
  }
}
```

### Get User Events

**GET** `/api/analytics/users/{user_id}/events`

Query parameters:
- `limit` (integer, max: 1000, default: 1000) - Number of events to return

### Get Real-time Metrics

**GET** `/api/analytics/metrics/realtime`

Query parameters:
- `bucket` (string: "minute", "hour", "day", default: "hour") - Time bucket
- `timestamp` (ISO 8601 timestamp, default: now) - Target timestamp
- `event_type` (string) - Filter by event type
- `user_ids` (comma-separated UUIDs) - Filter by user IDs

**Response:**
```json
{
  "total_events": 1500,
  "unique_users": 350,
  "event_types": {
    "page_view": 800,
    "click_event": 400,
    "form_submit": 300
  },
  "time_bucket": "2024-01-15T10:00:00Z"
}
```

### Get Time Series Data

**GET** `/api/analytics/metrics/timeseries`

Query parameters:
- `bucket` (string: "minute", "hour", "day", default: "hour") - Time bucket
- `from` (ISO 8601 timestamp, required) - Start time
- `to` (ISO 8601 timestamp, required) - End time
- `event_type` (string) - Filter by event type

**Response:**
```json
[
  [
    "2024-01-15T10:00:00Z",
    {
      "total_events": 1500,
      "unique_users": 350,
      "event_types": {
        "page_view": 800,
        "click_event": 400
      },
      "time_bucket": "2024-01-15T10:00:00Z"
    }
  ]
]
```

### Get Hourly Summaries

**GET** `/api/analytics/summaries/hourly`

Query parameters:
- `from` (ISO 8601 timestamp, required) - Start time
- `to` (ISO 8601 timestamp, required) - End time
- `event_type_ids` (comma-separated integers) - Filter by event type IDs

### Get User Activity

**GET** `/api/analytics/activity/users`

Query parameters:
- `user_id` (UUID) - Filter by specific user
- `from` (ISO 8601 timestamp, required) - Start time
- `to` (ISO 8601 timestamp, required) - End time

### Get Popular Events

**GET** `/api/analytics/events/popular`

Query parameters:
- `period` (string: "daily", "weekly", "monthly", default: "daily") - Time period
- `limit` (integer) - Number of results to return

**Response:**
```json
[
  {
    "event_type": "page_view",
    "total_count": 1000000,
    "unique_users": 25000,
    "period": "daily"
  }
]
```

### Refresh Materialized Views

**POST** `/api/analytics/refresh`

Manually refresh materialized views for analytics.

**Response:**
```
200 OK
```

## Error Responses

All endpoints return consistent error responses:

```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Invalid request data",
    "details": "Field 'name' is required"
  }
}
```

Common HTTP status codes:
- `200` - Success
- `201` - Created
- `204` - No Content
- `400` - Bad Request
- `404` - Not Found
- `422` - Unprocessable Entity
- `500` - Internal Server Error

## Rate Limiting

Currently no rate limiting is implemented. In production, consider implementing rate limiting based on your use case.

## Performance Notes

- The API is optimized for high throughput event ingestion
- Analytics endpoints use materialized views for fast queries
- Use pagination for large result sets
- Bulk operations are preferred for high-volume event creation
- Connection pooling is configured for optimal database performance

## Example Use Cases

### Track Page Views

```bash
curl -X POST "http://localhost:8080/api/events" \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "550e8400-e29b-41d4-a716-446655440000",
    "event_type_id": 1,
    "metadata": {
      "page": "/dashboard",
      "referrer": "https://google.com",
      "user_agent": "Mozilla/5.0..."
    }
  }'
```

### Get Daily Analytics

```bash
curl "http://localhost:8080/api/analytics/stats?from=2024-01-15T00:00:00Z&to=2024-01-15T23:59:59Z"
```

### Monitor Real-time Activity

```bash
curl "http://localhost:8080/api/analytics/metrics/realtime?bucket=minute"
```