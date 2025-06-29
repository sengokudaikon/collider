#!/bin/bash

set -e

HOST="$1"
USER_ID="${2:-1}"
TYPE="${3:-api.test}"

REQUESTS="${AB_REQUESTS:-1000}"
CONCURRENCY="${AB_CONCURRENCY:-50}"
TIMEOUT="${AB_TIMEOUT:-30}"

PASSED_COUNT=0
FAILED_COUNT=0

usage() {
    cat << EOF
Usage: $0 <host> [user_id] [event_type]

Arguments:
  host        Server URL (required) - e.g., http://localhost:8880
  user_id     User ID for testing (default: 1) - must be positive integer
  event_type  Event type to test (default: api.test)

Environment Variables:
  AB_REQUESTS     Number of requests (default: 1000)
  AB_CONCURRENCY  Concurrent requests (default: 20)
  AB_TIMEOUT      Request timeout in seconds (default: 30)

Examples:
  $0 http://localhost:8880
  $0 http://localhost:8880 123 api.test
  AB_REQUESTS=500 $0 http://localhost:8880

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

    if [ -z "$TYPE" ]; then
        echo "❌ Error: Event type cannot be empty"
        exit 1
    fi
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

generate_test_event() {
    echo "📝 Generating test event payload..."
    
    cat > event_data.json << EOF
{
  "user_id": $USER_ID,
  "event_type": "$TYPE",
  "metadata": {
    "page": "/load-test",
    "source": "ab_test_script",
    "timestamp_generated": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
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
        local rps=$(echo "$output" | grep "Requests per second" | awk '{print $4}')
        local avg_time=$(echo "$output" | grep "Time per request.*mean" | awk '{print $4}')
        echo "✅ $test_name PASSED"
        echo "   Performance: $rps req/sec, ${avg_time}ms avg"
        PASSED_COUNT=$((PASSED_COUNT + 1))
        return 0
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
    
    local result=$(ab -n "$REQUESTS" -c "$CONCURRENCY" -s "$TIMEOUT" "$url" 2>&1)
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
    echo "User ID: $USER_ID"
    echo "Event Type: $TYPE"
    echo "=================================="
    echo ""
    
    test_endpoint "Events List" "$HOST/events?limit=100"
    
    test_endpoint "User Events" "$HOST/user/$USER_ID/events?limit=100"
    
    test_post_endpoint "Event Creation" "$HOST/event" "event_data.json"
    
    local stats_url="$HOST/stats?type=$TYPE&limit=10"
    test_endpoint "Stats Query" "$stats_url"
    
    test_endpoint "Health Check" "$HOST/"
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
}

main() {
    trap cleanup EXIT

    validate_parameters
    check_dependencies
    check_server_health

    generate_test_event

    run_load_tests

    generate_test_report
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
