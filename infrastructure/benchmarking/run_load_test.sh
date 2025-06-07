#!/bin/bash
# High-Performance Load Testing Script for Collider Event Processing
# Tests the system's ability to handle high-throughput event ingestion

set -e

# Configuration
TARGET_HOST=${1:-"localhost:8080"}
RESULTS_DIR="results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

echo "🚀 Collider Load Testing Suite"
echo "==============================="
echo "Target: $TARGET_HOST"
echo "Results: $RESULTS_DIR"
echo ""

# Create results directory
mkdir -p "$RESULTS_DIR"

# Install Vegeta if not present
if ! command -v vegeta &> /dev/null; then
    echo "📦 Installing Vegeta load testing tool..."
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        wget -q https://github.com/tsenart/vegeta/releases/latest/download/vegeta_12.11.1_linux_amd64.tar.gz
        tar xfz vegeta_12.11.1_linux_amd64.tar.gz
        sudo mv vegeta /usr/local/bin/
        rm vegeta_12.11.1_linux_amd64.tar.gz
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        if command -v brew &> /dev/null; then
            brew install vegeta
        else
            echo "❌ Please install Vegeta manually: https://github.com/tsenart/vegeta"
            exit 1
        fi
    fi
    echo "✅ Vegeta installed"
fi

# Create test payload
cat > "$RESULTS_DIR/payload.json" <<EOF
{
  "data": {
    "event_type": "user_action",
    "timestamp": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
    "user_id": "test_user_$(date +%s)",
    "session_id": "session_$(date +%s)",
    "action": "click",
    "element": "button_submit",
    "page": "/dashboard",
    "metadata": {
      "browser": "Chrome",
      "version": "91.0.4472.124",
      "platform": "Linux",
      "screen_resolution": "1920x1080",
      "referrer": "https://example.com"
    },
    "performance_data": {
      "load_time": 234,
      "dom_ready": 189,
      "first_paint": 156
    }
  }
}
EOF

# Create target file for POST requests
cat > "$RESULTS_DIR/targets.txt" <<EOF
POST http://$TARGET_HOST/api/events
Content-Type: application/json
@$RESULTS_DIR/payload.json

EOF

# Create target file for GET requests (health check)
cat > "$RESULTS_DIR/health_targets.txt" <<EOF
GET http://$TARGET_HOST/health

EOF

# Test functions
run_test() {
    local rate=$1
    local duration=$2
    local description=$3
    local targets_file=$4
    local output_file="$RESULTS_DIR/${description}_${rate}rps_${TIMESTAMP}"
    
    echo "📊 Testing: $description at $rate RPS for $duration"
    
    vegeta attack \
        -targets="$targets_file" \
        -rate="$rate" \
        -duration="$duration" \
        -workers=50 \
        -max-workers=100 \
        -connections=100 \
        -timeout=30s > "${output_file}.bin"
    
    # Generate reports
    vegeta report < "${output_file}.bin" > "${output_file}.txt"
    vegeta report -type=json < "${output_file}.bin" > "${output_file}.json"
    
    # Show quick summary
    echo "Results:"
    vegeta report < "${output_file}.bin" | head -n 10
    echo ""
}

# Health check test
echo "🔍 Verifying server is healthy..."
if ! curl -f "http://$TARGET_HOST/health" &>/dev/null; then
    echo "❌ Server health check failed. Is the server running?"
    exit 1
fi
echo "✅ Server is healthy"
echo ""

# Warmup
echo "🔥 Warming up the server..."
run_test 100 10s "warmup" "$RESULTS_DIR/targets.txt"

# Progressive load tests for event ingestion
echo "📈 Starting progressive load tests for event ingestion..."

# Light load
run_test 500 30s "light_load" "$RESULTS_DIR/targets.txt"

# Medium load
run_test 2000 30s "medium_load" "$RESULTS_DIR/targets.txt"

# Heavy load
run_test 5000 30s "heavy_load" "$RESULTS_DIR/targets.txt"

# Stress test
run_test 10000 30s "stress_test" "$RESULTS_DIR/targets.txt"

# Peak load test
run_test 25000 30s "peak_load" "$RESULTS_DIR/targets.txt"

# Extreme load test (if server can handle it)
echo "🚨 Running extreme load test..."
run_test 50000 30s "extreme_load" "$RESULTS_DIR/targets.txt"

# Sustained load test
echo "⏱️  Running sustained load test..."
run_test 10000 300s "sustained_load" "$RESULTS_DIR/targets.txt"

# Health endpoint performance test
echo "💓 Testing health endpoint performance..."
run_test 5000 30s "health_check" "$RESULTS_DIR/health_targets.txt"

# Cleanup
rm -f "$RESULTS_DIR/payload.json" "$RESULTS_DIR/targets.txt" "$RESULTS_DIR/health_targets.txt"

# Generate summary report
echo "📋 Generating summary report..."
cat > "$RESULTS_DIR/summary_${TIMESTAMP}.md" <<EOF
# Collider Load Test Summary

**Test Date:** $(date)
**Target:** $TARGET_HOST
**Test Duration:** $(date)

## Test Results

| Test Type | RPS | Duration | Success Rate | P50 Latency | P95 Latency | P99 Latency |
|-----------|-----|----------|--------------|-------------|-------------|-------------|
EOF

# Parse results and add to summary
for json_file in "$RESULTS_DIR"/*_"$TIMESTAMP".json; do
    if [[ -f "$json_file" ]]; then
        test_name=$(basename "$json_file" .json | sed "s/_${TIMESTAMP}//")
        rate=$(echo "$test_name" | grep -o '[0-9]*rps' | sed 's/rps//')
        
        # Extract metrics from JSON (requires jq, fallback to simple parsing)
        if command -v jq &> /dev/null; then
            success_rate=$(jq -r '.success' "$json_file" 2>/dev/null || echo "N/A")
            p50=$(jq -r '.latencies."50th"' "$json_file" 2>/dev/null || echo "N/A")
            p95=$(jq -r '.latencies."95th"' "$json_file" 2>/dev/null || echo "N/A")
            p99=$(jq -r '.latencies."99th"' "$json_file" 2>/dev/null || echo "N/A")
        else
            success_rate="N/A"
            p50="N/A"
            p95="N/A"
            p99="N/A"
        fi
        
        echo "| $test_name | $rate | 30s | $success_rate | $p50 | $p95 | $p99 |" >> "$RESULTS_DIR/summary_${TIMESTAMP}.md"
    fi
done

echo ""
echo "✅ Load testing completed!"
echo "📊 Results saved to: $RESULTS_DIR/"
echo "📋 Summary report: $RESULTS_DIR/summary_${TIMESTAMP}.md"
echo ""
echo "🔍 Quick Performance Summary:"
echo "=============================="

# Show the most important results
if [[ -f "$RESULTS_DIR/stress_test_10000rps_${TIMESTAMP}.txt" ]]; then
    echo "Stress Test (10,000 RPS):"
    head -n 8 "$RESULTS_DIR/stress_test_10000rps_${TIMESTAMP}.txt" | tail -n 6
fi

echo ""
echo "💡 Next steps:"
echo "   - Review detailed results in $RESULTS_DIR/"
echo "   - Check application logs for any errors"
echo "   - Monitor system metrics during high load"
echo "   - Tune configuration based on bottlenecks found"