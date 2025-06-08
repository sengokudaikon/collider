#!/bin/bash

# k6 Load Testing Script for Collider
# Comprehensive performance testing with detailed reporting

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_URL=${1:-"http://localhost:8080"}
TEST_TYPE=${2:-"load"}
RESULTS_DIR="k6_results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

echo "📊 k6 Load Testing Suite"
echo "========================"
echo "Target: $TARGET_URL"
echo "Test Type: $TEST_TYPE"
echo "Results: $RESULTS_DIR"
echo ""

# Check dependencies
check_dependencies() {
    echo "🔍 Checking dependencies..."
    
    # Check if k6 is installed
    if ! command -v k6 &> /dev/null; then
        echo "❌ k6 not found. Installing..."
        install_k6
    else
        echo "✅ k6 found: $(k6 version)"
    fi
    
    # Check if target server is running
    if ! curl -f "$TARGET_URL/health" &>/dev/null; then
        echo "❌ Target server health check failed: $TARGET_URL"
        echo "💡 Please ensure the server is running and accessible"
        exit 1
    fi
    echo "✅ Target server is healthy: $TARGET_URL"
}

install_k6() {
    echo "📦 Installing k6..."
    
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        # Install on Linux
        sudo gpg -k
        sudo gpg --no-default-keyring --keyring /usr/share/keyrings/k6-archive-keyring.gpg --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys C5AD17C747E3415A3642D57D77C6C491D6AC1D69
        echo "deb [signed-by=/usr/share/keyrings/k6-archive-keyring.gpg] https://dl.k6.io/deb stable main" | sudo tee /etc/apt/sources.list.d/k6.list
        sudo apt-get update
        sudo apt-get install k6
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        # Install on macOS
        if command -v brew &> /dev/null; then
            brew install k6
        else
            echo "❌ Homebrew not found. Installing via binary..."
            curl -O -L https://github.com/grafana/k6/releases/latest/download/k6-macos-amd64.tar.gz
            tar -xzf k6-macos-amd64.tar.gz
            sudo mv k6-macos-amd64/k6 /usr/local/bin/
            rm -rf k6-macos-amd64*
        fi
    else
        echo "❌ Unsupported OS. Please install k6 manually:"
        echo "   https://k6.io/docs/getting-started/installation/"
        exit 1
    fi
    
    echo "✅ k6 installed successfully"
}

# Prepare test environment
prepare_environment() {
    echo "🛠️  Preparing test environment..."
    
    # Create results directory
    mkdir -p "$RESULTS_DIR"
    
    echo "✅ Environment prepared"
}

# Run different types of tests
run_smoke_test() {
    echo "💨 Running smoke test..."
    
    k6 run \
        --vus 1 \
        --duration 30s \
        --env TARGET_URL="$TARGET_URL" \
        --out json="$RESULTS_DIR/smoke_test_${TIMESTAMP}.json" \
        --summary-export="$RESULTS_DIR/smoke_test_${TIMESTAMP}_summary.json" \
        "$SCRIPT_DIR/load-test.js" \
        --function smokeTest
    
    echo "✅ Smoke test completed"
}

run_load_test() {
    echo "📈 Running load test..."
    
    k6 run \
        --env TARGET_URL="$TARGET_URL" \
        --out json="$RESULTS_DIR/load_test_${TIMESTAMP}.json" \
        --summary-export="$RESULTS_DIR/load_test_${TIMESTAMP}_summary.json" \
        "$SCRIPT_DIR/load-test.js"
    
    echo "✅ Load test completed"
}

run_stress_test() {
    echo "💪 Running stress test..."
    
    k6 run \
        --vus 1000 \
        --duration 5m \
        --env TARGET_URL="$TARGET_URL" \
        --out json="$RESULTS_DIR/stress_test_${TIMESTAMP}.json" \
        --summary-export="$RESULTS_DIR/stress_test_${TIMESTAMP}_summary.json" \
        "$SCRIPT_DIR/load-test.js" \
        --function stressTest
    
    echo "✅ Stress test completed"
}

run_spike_test() {
    echo "⚡ Running spike test..."
    
    k6 run \
        --stages '[{"duration":"30s","target":100},{"duration":"10s","target":2000},{"duration":"30s","target":100}]' \
        --env TARGET_URL="$TARGET_URL" \
        --out json="$RESULTS_DIR/spike_test_${TIMESTAMP}.json" \
        --summary-export="$RESULTS_DIR/spike_test_${TIMESTAMP}_summary.json" \
        "$SCRIPT_DIR/load-test.js" \
        --function spikeTest
    
    echo "✅ Spike test completed"
}

# Generate comprehensive report
generate_report() {
    echo "📋 Generating test report..."
    
    local report_file="$RESULTS_DIR/k6_report_${TIMESTAMP}.md"
    
    cat > "$report_file" <<EOF
# k6 Load Test Report

**Test Date:** $(date)
**Target:** $TARGET_URL
**Test Type:** $TEST_TYPE
**Test ID:** $TIMESTAMP

## Test Configuration

### Load Test Profile
- **Ramp-up:** 30s to 100 VUs, 1m to 500 VUs, 2m to 1000 VUs
- **Peak Load:** 5000 VUs for 5 minutes
- **Cool-down:** Gradual ramp down over 1.5 minutes
- **Total Duration:** ~11 minutes

### Performance Thresholds
- **95th percentile response time:** < 500ms
- **99th percentile response time:** < 1000ms
- **Error rate:** < 5%
- **Event creation success rate:** > 95%

## Test Scenarios

1. **Health Check** - GET /health (continuous monitoring)
2. **Event Creation** - POST /api/events (primary load)
3. **Event Count** - GET /api/events/count (read operations)
4. **User Operations** - POST /api/users, GET /api/users/{id}/analytics

## Results Summary

EOF

    # Parse summary file for key metrics (if available)
    if [[ -f "$RESULTS_DIR/${TEST_TYPE}_test_${TIMESTAMP}_summary.json" ]]; then
        echo "### Key Metrics" >> "$report_file"
        echo "" >> "$report_file"
        
        # Extract metrics using jq if available
        if command -v jq &> /dev/null; then
            local summary_file="$RESULTS_DIR/${TEST_TYPE}_test_${TIMESTAMP}_summary.json"
            
            echo "- **Total Requests:** $(jq -r '.metrics.http_reqs.count // "N/A"' "$summary_file")" >> "$report_file"
            echo "- **Request Rate:** $(jq -r '.metrics.http_reqs.rate // "N/A"' "$summary_file") req/s" >> "$report_file"
            echo "- **Failed Requests:** $(jq -r '.metrics.http_req_failed.rate * 100 // "N/A"' "$summary_file")%" >> "$report_file"
            echo "- **Avg Response Time:** $(jq -r '.metrics.http_req_duration.avg // "N/A"' "$summary_file")ms" >> "$report_file"
            echo "- **95th Percentile:** $(jq -r '.metrics.http_req_duration.p95 // "N/A"' "$summary_file")ms" >> "$report_file"
            echo "- **99th Percentile:** $(jq -r '.metrics.http_req_duration.p99 // "N/A"' "$summary_file")ms" >> "$report_file"
            echo "- **Data Received:** $(jq -r '.metrics.data_received.count // "N/A"' "$summary_file") bytes" >> "$report_file"
            echo "- **Data Sent:** $(jq -r '.metrics.data_sent.count // "N/A"' "$summary_file") bytes" >> "$report_file"
        else
            echo "*(Install jq for detailed metrics parsing)*" >> "$report_file"
        fi
        
        echo "" >> "$report_file"
    fi
    
    cat >> "$report_file" <<EOF

## Files Generated

- Test results: \`${TEST_TYPE}_test_${TIMESTAMP}.json\`
- Summary: \`${TEST_TYPE}_test_${TIMESTAMP}_summary.json\`
- Report: \`k6_report_${TIMESTAMP}.md\`

## Analysis

### Performance Assessment
- Review response time percentiles vs thresholds
- Analyze error patterns and status codes
- Check resource utilization during peak load
- Validate custom metrics (event creation success rate)

### Bottleneck Identification
- Monitor for increased latency at specific load levels
- Check for error rate spikes during ramp-up
- Analyze correlation between VUs and response times

## Recommendations

1. **If thresholds passed:** System performed well, consider higher load testing
2. **If latency issues:** Check server configuration and database performance
3. **If errors occurred:** Review application logs and error patterns
4. **For optimization:** Focus on the slowest operations identified

## Next Steps

1. Compare results with Yandex Tank and Vegeta tests
2. Monitor system metrics during test execution
3. Review application logs for any warnings/errors
4. Use results to tune production configuration

EOF

    echo "✅ Report generated: $report_file"
}

# Cleanup function
cleanup() {
    echo "🧹 Cleaning up..."
    # Remove any temporary files
    echo "✅ Cleanup completed"
}

# Main execution
main() {
    echo "🎯 Starting k6 test suite..."
    
    check_dependencies
    prepare_environment
    
    case "$TEST_TYPE" in
        "smoke")
            run_smoke_test
            ;;
        "load")
            run_load_test
            ;;
        "stress")
            run_stress_test
            ;;
        "spike")
            run_spike_test
            ;;
        "all")
            run_smoke_test
            sleep 30
            run_load_test
            sleep 60
            run_stress_test
            sleep 60
            run_spike_test
            ;;
        *)
            echo "❌ Unknown test type: $TEST_TYPE"
            echo "Available types: smoke, load, stress, spike, all"
            exit 1
            ;;
    esac
    
    generate_report
    cleanup
    
    echo ""
    echo "🎉 k6 testing completed successfully!"
    echo "📊 Results directory: $RESULTS_DIR"
    echo "📋 Report: $RESULTS_DIR/k6_report_${TIMESTAMP}.md"
    echo ""
    echo "💡 Usage examples:"
    echo "   ./run-k6.sh http://localhost:8080 smoke    # Quick validation"
    echo "   ./run-k6.sh http://localhost:8080 load     # Standard load test"
    echo "   ./run-k6.sh http://localhost:8080 stress   # High load test"
    echo "   ./run-k6.sh http://localhost:8080 spike    # Spike load test"
    echo "   ./run-k6.sh http://localhost:8080 all      # Run all tests"
}

# Handle script interruption
trap cleanup EXIT

# Show usage if no arguments
if [[ $# -eq 0 ]]; then
    echo "Usage: $0 [TARGET_URL] [TEST_TYPE]"
    echo ""
    echo "Examples:"
    echo "  $0                                    # Default: localhost load test"
    echo "  $0 http://localhost:8080              # Load test on localhost"
    echo "  $0 http://localhost:8080 smoke        # Smoke test"
    echo "  $0 http://production.example.com all  # All tests on production"
    echo ""
    echo "Test types: smoke, load, stress, spike, all"
    exit 0
fi

# Run main function
main "$@"