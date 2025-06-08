#!/bin/bash

# Yandex Tank Load Testing Script for Collider
# High-performance load testing with system monitoring

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_HOST=${1:-"localhost:8080"}
CONFIG_FILE=${2:-"load.yaml"}
RESULTS_DIR="tank_results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

echo "🔥 Yandex Tank Load Testing Suite"
echo "=================================="
echo "Target: $TARGET_HOST"
echo "Config: $CONFIG_FILE"
echo "Results: $RESULTS_DIR"
echo ""

# Check dependencies
check_dependencies() {
    echo "🔍 Checking dependencies..."
    
    # Check if yandex-tank is installed
    if ! command -v yandex-tank &> /dev/null; then
        echo "❌ Yandex Tank not found. Installing..."
        install_yandex_tank
    else
        echo "✅ Yandex Tank found: $(yandex-tank --version)"
    fi
    
    # Check if target server is running
    if ! curl -f "http://$TARGET_HOST/health" &>/dev/null; then
        echo "❌ Target server health check failed: $TARGET_HOST"
        echo "💡 Please ensure the server is running and accessible"
        exit 1
    fi
    echo "✅ Target server is healthy: $TARGET_HOST"
}

install_yandex_tank() {
    echo "📦 Installing Yandex Tank..."
    
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        # Install on Linux
        sudo apt-get update
        sudo apt-get install -y python3-pip python3-dev
        pip3 install yandextank[phantom]
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        # Install on macOS
        if command -v brew &> /dev/null; then
            brew install python3
            pip3 install yandextank[phantom]
        else
            echo "❌ Homebrew not found. Please install manually:"
            echo "   pip3 install yandextank[phantom]"
            exit 1
        fi
    else
        echo "❌ Unsupported OS. Please install Yandex Tank manually:"
        echo "   pip3 install yandextank[phantom]"
        exit 1
    fi
    
    echo "✅ Yandex Tank installed successfully"
}

# Prepare test environment
prepare_environment() {
    echo "🛠️  Preparing test environment..."
    
    # Create results directory
    mkdir -p "$RESULTS_DIR"
    
    # Update config with target host
    sed "s/localhost:8080/$TARGET_HOST/g" "$CONFIG_FILE" > "$RESULTS_DIR/load_${TIMESTAMP}.yaml"
    
    # Update ammo file with target host
    sed "s/localhost:8080/$TARGET_HOST/g" "ammo.txt" > "$RESULTS_DIR/ammo_${TIMESTAMP}.txt"
    
    # Update config to use the new ammo file
    sed -i.bak "s/ammo.txt/ammo_${TIMESTAMP}.txt/g" "$RESULTS_DIR/load_${TIMESTAMP}.yaml"
    
    echo "✅ Environment prepared"
}

# Run the load test
run_load_test() {
    echo "🚀 Starting Yandex Tank load test..."
    echo "⚠️  This will generate significant load on $TARGET_HOST"
    echo "📊 Monitor system resources during the test"
    echo ""
    
    cd "$RESULTS_DIR"
    
    # Run yandex-tank with config
    yandex-tank "load_${TIMESTAMP}.yaml" \
        --log-file "tank_${TIMESTAMP}.log" \
        --option "phantom.address=$TARGET_HOST" \
        --option "phantom.ammo_file=ammo_${TIMESTAMP}.txt" \
        || true  # Don't fail if tank stops due to autostop conditions
    
    cd ..
    
    echo "✅ Load test completed"
}

# Generate comprehensive report
generate_report() {
    echo "📋 Generating test report..."
    
    local report_file="$RESULTS_DIR/tank_report_${TIMESTAMP}.md"
    
    cat > "$report_file" <<EOF
# Yandex Tank Load Test Report

**Test Date:** $(date)
**Target:** $TARGET_HOST
**Duration:** $(date)
**Test ID:** $TIMESTAMP

## Test Configuration

- **Load Profile:** Progressive ramp from 100 to 10,000 RPS
- **Duration:** ~8 minutes (480s ramp + 300s sustained)
- **Autostop Conditions:**
  - Max 5,000 instances for 10s
  - 99th percentile latency > 100ms for 30s
  - HTTP 5xx errors > 10% for 30s

## Test Scenarios

1. **Health Check** - GET /health
2. **Event Creation** - POST /api/events
3. **Event Count** - GET /api/events/count
4. **User Analytics** - GET /api/users/{id}/analytics
5. **User Creation** - POST /api/users

## Results Summary

EOF

    # Parse log file for key metrics (if available)
    if [[ -f "$RESULTS_DIR/tank_${TIMESTAMP}.log" ]]; then
        echo "### Key Metrics" >> "$report_file"
        echo "" >> "$report_file"
        echo "\`\`\`" >> "$report_file"
        grep -E "(RPS|latency|errors)" "$RESULTS_DIR/tank_${TIMESTAMP}.log" | tail -20 >> "$report_file" || true
        echo "\`\`\`" >> "$report_file"
        echo "" >> "$report_file"
    fi
    
    cat >> "$report_file" <<EOF

## Files Generated

- Configuration: \`load_${TIMESTAMP}.yaml\`
- Ammo file: \`ammo_${TIMESTAMP}.txt\`
- Log file: \`tank_${TIMESTAMP}.log\`
- Detailed results: Check yandex-tank output files

## Analysis

Review the generated files for:
- Response time percentiles
- Error rates by scenario
- System resource utilization
- Throughput over time
- Autostop trigger analysis

## Recommendations

1. **If test stopped due to autostop conditions:** Check server logs and system metrics
2. **If all scenarios passed:** Consider increasing load for stress testing
3. **If errors occurred:** Investigate specific failure patterns
4. **For production tuning:** Use these results to optimize server configuration

EOF

    echo "✅ Report generated: $report_file"
}

# Cleanup function
cleanup() {
    echo "🧹 Cleaning up..."
    # Remove temporary files if needed
    rm -f "$RESULTS_DIR"/*.bak 2>/dev/null || true
    echo "✅ Cleanup completed"
}

# Main execution
main() {
    echo "🎯 Starting Yandex Tank test suite..."
    
    check_dependencies
    prepare_environment
    run_load_test
    generate_report
    cleanup
    
    echo ""
    echo "🎉 Yandex Tank testing completed successfully!"
    echo "📊 Results directory: $RESULTS_DIR"
    echo "📋 Report: $RESULTS_DIR/tank_report_${TIMESTAMP}.md"
    echo ""
    echo "💡 Next steps:"
    echo "   1. Review the generated report"
    echo "   2. Analyze system metrics during peak load"
    echo "   3. Check application logs for any errors"
    echo "   4. Compare results with other load testing tools"
}

# Handle script interruption
trap cleanup EXIT

# Run main function
main "$@"