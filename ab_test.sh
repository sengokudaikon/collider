#!/bin/bash

set -e

HOST="$1"
USER_ID="${2:-1}"
TYPE="${3:-}"

REQUESTS="${AB_REQUESTS:-1000}"
CONCURRENCY="${AB_CONCURRENCY:-50}"
TIMEOUT="${AB_TIMEOUT:-30}"

PASSED_COUNT=0
FAILED_COUNT=0

# Data export settings
EXPORT_DIR="${AB_EXPORT_DIR:-./load_testing/data}"
USE_LIVE_DATA="${AB_USE_LIVE_DATA:-true}"

usage() {
    cat << EOF
Usage: $0 <host> [user_id] [event_type]

Arguments:
  host        Server URL (required) - e.g., http://127.0.0.1:8880
  user_id     User ID for testing (default: 1) - must be positive integer
  event_type  Event type to test (default: auto-detected from server)

Environment Variables:
  AB_REQUESTS      Number of requests (default: 1000)
  AB_CONCURRENCY   Concurrent requests (default: 50)
  AB_TIMEOUT       Request timeout in seconds (default: 30)
  AB_USE_LIVE_DATA Use csv-exporter to fetch live data (default: true)
  AB_EXPORT_DIR    Directory for exported data (default: ./load_testing/data)

Examples:
  $0 http://127.0.0.1:8880
  $0 http://127.0.0.1:8880 123 event_type_42
  AB_REQUESTS=500 $0 http://127.0.0.1:8880
  AB_USE_LIVE_DATA=false $0 http://127.0.0.1:8880  # Use hardcoded data
  AB_EXPORT_DIR=/tmp/test-data $0 http://127.0.0.1:8880  # Custom export dir

EOF
}

validate_parameters() {
    if [ -z "$HOST" ]; then
        echo "❌ Error: Host parameter is required"
        usage
        exit 1
    fi

    if [[ ! "$HOST" =~ ^https?:// ]]; then
        echo "❌ Error: Invalid host format. Use http://hostname:port"
        usage
        exit 1
    fi

    if [[ ! "$USER_ID" =~ ^[0-9]+$ ]]; then
        echo "❌ Error: Invalid user_id format. Must be a positive integer"
        exit 1
    fi

    if [[ "$USER_ID" -le 0 ]]; then
        echo "❌ Error: user_id must be a positive integer (BIGSERIAL)"
        exit 1
    fi

    # TYPE validation will be done after fetching from server if needed
}

check_dependencies() {
    if ! command -v ab &> /dev/null; then
        echo "❌ Error: Apache Bench (ab) is not installed"
        echo "   Install with: apt-get install apache2-utils (Ubuntu/Debian)"
        echo "              or: brew install httpd (macOS)"
        exit 1
    fi

    if ! command -v curl &> /dev/null; then
        echo "❌ Error: curl is not installed"
        exit 1
    fi

    if ! command -v jq &> /dev/null; then
        echo "⚠  Warning: jq not found. JSON validation will be skipped"
    fi

    if [ "$USE_LIVE_DATA" = "true" ]; then
        if ! command -v cargo &> /dev/null; then
            echo "⚠  Warning: cargo not found. Cannot build csv-exporter for live data"
            echo "   Falling back to hardcoded data"
            USE_LIVE_DATA="false"
        fi
    fi
}

check_server_health() {
    echo "🔍 Checking server health..."
    
    if ! curl -s -m 10 "$HOST/" >/dev/null 2>&1; then
        echo "❌ Error: Server not responding at $HOST"
        echo "   Make sure the server is running and accessible"
        exit 1
    fi
    
    echo "✅ Server health check passed"
}

export_live_data() {
    if [ "$USE_LIVE_DATA" != "true" ]; then
        return 0
    fi

    echo "📦 Exporting live data from server..."
    
    # Create export directory if it doesn't exist
    mkdir -p "$EXPORT_DIR"
    
    # Build and run csv-exporter
    local csv_exporter_path="./binaries/csv-exporter"
    
    # Check if we're in the project root or need to go up
    if [ ! -d "$csv_exporter_path" ] && [ -d "../binaries/csv-exporter" ]; then
        csv_exporter_path="../binaries/csv-exporter"
    fi
    
    if [ ! -d "$csv_exporter_path" ]; then
        echo "⚠  Warning: csv-exporter not found. Using hardcoded data"
        USE_LIVE_DATA="false"
        return 0
    fi
    
    # Build csv-exporter if needed
    echo "🔨 Building csv-exporter..."
    if ! (cd "$csv_exporter_path" && cargo build --release --quiet); then
        echo "⚠  Warning: Failed to build csv-exporter. Using hardcoded data"
        USE_LIVE_DATA="false"
        return 0
    fi
    
    # Run csv-exporter to fetch data
    local exporter_bin="$csv_exporter_path/target/release/csv-exporter"
    if [ ! -f "$exporter_bin" ]; then
        echo "⚠  Warning: csv-exporter binary not found. Using hardcoded data"
        USE_LIVE_DATA="false"
        return 0
    fi
    
    echo "📊 Fetching event types and users from database..."
    if ! "$exporter_bin" all --output-dir "$EXPORT_DIR" > /dev/null 2>&1; then
        echo "⚠  Warning: Failed to export data. Using hardcoded data"
        USE_LIVE_DATA="false"
        return 0
    fi
    
    # Verify exported files exist
    if [ ! -f "$EXPORT_DIR/event_types.csv" ] || [ ! -f "$EXPORT_DIR/users.csv" ]; then
        echo "⚠  Warning: Exported files not found. Using hardcoded data"
        USE_LIVE_DATA="false"
        return 0
    fi
    
    echo "✅ Successfully exported live data"
}

get_random_event_type() {
    echo "🔍 Fetching available event types..." >&2
    
    local event_types=""
    
    # Try to use live data first
    if [ "$USE_LIVE_DATA" = "true" ] && [ -f "$EXPORT_DIR/event_types.csv" ]; then
        # Read event types from CSV (skip header, get name column)
        event_types=$(tail -n +2 "$EXPORT_DIR/event_types.csv" | cut -d',' -f2 | tr -d '"' | grep -v '^$')
        
        if [ -z "$event_types" ]; then
            echo "⚠  Warning: No event types found in CSV. Using hardcoded data" >&2
            USE_LIVE_DATA="false"
        else
            echo "✅ Using live event types from database" >&2
        fi
    fi
    
    # Fallback to hardcoded event types
    if [ -z "$event_types" ]; then
        event_types="api.request
admin.login
api.rate_limited
admin.settings_updated
admin.updated_user
api.error
admin.logout
admin.generated_report
api.response
admin.deleted_user"
        echo "ℹ️  Using hardcoded event types" >&2
    fi
    
    # Select random event type
    local random_type=$(echo "$event_types" | shuf -n 1)
    echo "✅ Selected random event type: $random_type" >&2
    echo "$random_type"
}

get_random_user_id() {
    if [ "$USE_LIVE_DATA" = "true" ] && [ -f "$EXPORT_DIR/users.csv" ]; then
        # Read user IDs from CSV (skip header, get id column)
        local user_ids=$(tail -n +2 "$EXPORT_DIR/users.csv" | cut -d',' -f1 | tr -d '"' | grep -v '^$' | head -20)
        
        if [ -n "$user_ids" ]; then
            local random_user=$(echo "$user_ids" | shuf -n 1)
            echo "✅ Selected random user ID from database: $random_user" >&2
            echo "$random_user"
            return
        fi
    fi
    
    # Return the provided user ID as fallback
    echo "$USER_ID"
}

generate_test_event() {
    echo "📝 Generating test event payload..."
    
    # Use the actual user ID (might be from live data or provided)
    local test_user_id="$ACTUAL_USER_ID"
    
    # Create different metadata for variety
    local pages=("/home" "/dashboard" "/api/v1/test" "/admin" "/load-test" "/profile" "/settings")
    local random_page=${pages[$RANDOM % ${#pages[@]}]}
    
    cat > event_data.json << EOF
{
  "user_id": $test_user_id,
  "event_type": "$TYPE",
  "metadata": {
    "page": "$random_page",
    "source": "ab_test_script",
    "timestamp_generated": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "session_id": "test-$(date +%s)-$$",
    "test_run": true
  }
}
EOF
    
    if command -v jq &> /dev/null; then
        if ! jq empty event_data.json 2>/dev/null; then
            echo "❌ Error: Generated JSON is invalid"
            cat event_data.json
            exit 1
        fi
        echo "✅ JSON payload validated"
    fi
}

analyze_ab_results() {
    local test_name="$1"
    local output="$2"
    
    if echo "$output" | grep -q "Test aborted"; then
        echo "❌ $test_name FAILED"
        echo "   Reason: $(echo "$output" | grep "Test aborted" | head -1)"
        FAILED_COUNT=$((FAILED_COUNT + 1))
        return 1
    elif echo "$output" | grep -q "Requests per second"; then
        # Check for both failed requests and non-2xx responses
        local failed_requests=$(echo "$output" | grep "Failed requests:" | awk '{print $3}' || echo "0")
        local non2xx_responses=$(echo "$output" | grep "Non-2xx responses:" | awk '{print $3}' || echo "0")
        local total_requests=$(echo "$output" | grep "Complete requests:" | awk '{print $3}')
        
        # Convert to numbers, default to 0 if empty
        failed_requests=${failed_requests:-0}
        non2xx_responses=${non2xx_responses:-0}
        
        if [ "$failed_requests" -gt 0 ] || [ "$non2xx_responses" -gt 0 ]; then
            echo "❌ $test_name FAILED"
            
            if [ "$failed_requests" -gt 0 ]; then
                echo "   Connection failures: $failed_requests/$total_requests"
            fi
            
            if [ "$non2xx_responses" -gt 0 ]; then
                echo "   HTTP errors (500, 404, etc): $non2xx_responses/$total_requests"
                echo "   Error rate: $(( (non2xx_responses * 100) / total_requests ))%"
            fi
            
            FAILED_COUNT=$((FAILED_COUNT + 1))
            return 1
        else
            local rps=$(echo "$output" | grep "Requests per second" | awk '{print $4}')
            local avg_time=$(echo "$output" | grep "Time per request.*mean" | awk '{print $4}')
            echo "✅ $test_name PASSED"
            echo "   Performance: $rps req/sec, ${avg_time}ms avg"
            echo "   Success rate: $total_requests/$total_requests (100%)"
            PASSED_COUNT=$((PASSED_COUNT + 1))
            return 0
        fi
    else
        echo "⚠  $test_name UNCLEAR RESULT"
        FAILED_COUNT=$((FAILED_COUNT + 1))
        return 2
    fi
}

test_endpoint() {
    local test_name="$1"
    local url="$2"
    
    echo "📊 Testing: $test_name"
    echo "   URL: $url"
    
    # Add -v flag for verbose output and ensure we catch HTTP errors
    local result=$(ab -n "$REQUESTS" -c "$CONCURRENCY" -s "$TIMEOUT" -v 2 "$url" 2>&1)
    analyze_ab_results "$test_name" "$result"
    echo ""
}

test_post_endpoint() {
    local test_name="$1"
    local url="$2"
    local data_file="$3"
    
    echo "📊 Testing: $test_name"
    echo "   URL: $url"
    echo "   Data: $data_file"
    
    local result=$(ab -n "$REQUESTS" -c "$CONCURRENCY" -s "$TIMEOUT" \
                     -v 2 \
                     -T "application/json" \
                     -p "$data_file" \
                     "$url" 2>&1)
    analyze_ab_results "$test_name" "$result"
    echo ""
}

run_load_tests() {
    echo "🚀 Starting Collider Load Tests"
    echo "=================================="
    echo "Host: $HOST"
    echo "Requests: $REQUESTS"
    echo "Concurrency: $CONCURRENCY"
    echo "Timeout: ${TIMEOUT}s"
    echo "User ID: $ACTUAL_USER_ID"
    echo "Event Type: $TYPE"
    echo "Live Data: $USE_LIVE_DATA"
    echo "=================================="
    echo ""
    
    test_endpoint "Events List" "$HOST/events?limit=100"
    
    test_endpoint "User Events" "$HOST/user/$ACTUAL_USER_ID/events?limit=100"
    
    test_post_endpoint "Event Creation" "$HOST/event" "event_data.json"
    
    local stats_url="$HOST/stats?type=$TYPE&limit=10"
    test_endpoint "Stats Query" "$stats_url"
    
    test_endpoint "Health Check" "$HOST/"
    
    # If using live data, test with multiple random users and event types
    if [ "$USE_LIVE_DATA" = "true" ]; then
        echo "🔄 Testing with random users and event types..."
        
        for i in {1..3}; do
            local random_user=$(get_random_user_id)
            local random_type=$(get_random_event_type)
            
            ACTUAL_USER_ID="$random_user" TYPE="$random_type" generate_test_event
            
            test_endpoint "Random User Events" "$HOST/user/$random_user/events?limit=50"
            test_post_endpoint "Random Event Creation" "$HOST/event" "event_data.json"
        done
    fi
}

generate_test_report() {
    local total_tests=$((PASSED_COUNT + FAILED_COUNT))
    local success_rate=0
    
    if [ "$total_tests" -gt 0 ]; then
        success_rate=$(( (PASSED_COUNT * 100) / total_tests ))
    fi
    
    echo "=================================="
    echo "📋 LOAD TEST SUMMARY"
    echo "=================================="
    echo "Total Tests: $total_tests"
    echo "Passed: $PASSED_COUNT"
    echo "Failed: $FAILED_COUNT"
    echo "Success Rate: ${success_rate}%"
    echo ""
    
    if [ "$FAILED_COUNT" -eq 0 ]; then
        echo "🎉 All tests passed! System performance is good."
        exit 0
    else
        echo "⚠️  Some tests failed. Check server logs and performance."
        exit 1
    fi
}

cleanup() {
    echo "🧹 Cleaning up..."
    [ -f "event_data.json" ] && rm -f "event_data.json"
    
    # Optionally clean up exported data
    if [ "${AB_CLEANUP_EXPORT:-false}" = "true" ] && [ -d "$EXPORT_DIR" ]; then
        echo "   Removing exported data..."
        rm -rf "$EXPORT_DIR"
    fi
}

main() {
    trap cleanup EXIT

    validate_parameters
    check_dependencies
    check_server_health

    # Export live data if enabled
    export_live_data

    # Get random user ID if using live data
    if [ "$USE_LIVE_DATA" = "true" ] && [ "$USER_ID" = "1" ]; then
        # Only override default user ID if using the default
        ACTUAL_USER_ID=$(get_random_user_id)
    else
        ACTUAL_USER_ID="$USER_ID"
    fi

    # Get random event type if not provided
    if [ -z "$TYPE" ]; then
        TYPE=$(get_random_event_type)
        if [ -z "$TYPE" ]; then
            echo "❌ Error: Could not determine event type"
            exit 1
        fi
    fi

    generate_test_event

    run_load_tests

    generate_test_report
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
