#!/bin/bash

# Goose Load Testing Script for Collider
# Rust-based load testing with customizable scenarios

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_URL=${1:-"http://localhost:8080"}
USERS=${2:-1000}
HATCH_RATE=${3:-"100/1s"}
RUN_TIME=${4:-300}
RESULTS_DIR="goose_results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

echo "🦆 Goose Load Testing Suite"
echo "=========================="
echo "Target: $TARGET_URL"
echo "Users: $USERS"
echo "Hatch Rate: $HATCH_RATE"
echo "Run Time: ${RUN_TIME}s"
echo "Results: $RESULTS_DIR"
echo ""

# Create results directory
mkdir -p "$RESULTS_DIR"

# Check if server is available
check_server_health() {
    echo "🔍 Checking server health..."
    if ! curl -f "$TARGET_URL/health" &>/dev/null; then
        echo "❌ Server health check failed: $TARGET_URL"
        exit 1
    fi
    echo "✅ Server is healthy"
}

# Create dynamic Goose configuration
create_goose_config() {
    cat > goose_dynamic.rs <<EOF
use goose::prelude::*;
use serde_json::json;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), GooseError> {
    GooseAttack::initialize()?
        .register_scenario(
            scenario!("HealthCheck")
                .register_transaction(transaction!(health_check).set_weight(1)?),
        )
        .register_scenario(
            scenario!("EventCreation")
                .register_transaction(transaction!(create_event).set_weight(8)?),
        )
        .register_scenario(
            scenario!("EventRetrieval")
                .register_transaction(transaction!(get_events).set_weight(6)?),
        )
        .register_scenario(
            scenario!("EventUpdate")
                .register_transaction(transaction!(update_event).set_weight(2)?),
        )
        .set_default(GooseDefault::Host, "$TARGET_URL")?
        .set_default(GooseDefault::Users, $USERS)?
        .set_default(GooseDefault::HatchRate, "$HATCH_RATE")?
        .set_default(GooseDefault::RunTime, $RUN_TIME)?
        .set_default(GooseDefault::LogLevel, 1)?
        .set_default(GooseDefault::ReportFile, "goose-report-$TIMESTAMP.html")?
        .execute()
        .await?;

    Ok(())
}

async fn health_check(user: &mut GooseUser) -> TransactionResult {
    let _response = user.get("/health").await?;
    Ok(())
}

async fn create_event(user: &mut GooseUser) -> TransactionResult {
    let event_payload = json!({
        "data": {
            "event_type": "user_action",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "user_id": format!("goose_user_{}", user.weighted_users_index),
            "session_id": format!("goose_session_{}", fastrand::u64(..)),
            "action": ["click", "view", "scroll", "submit"][fastrand::usize(0..4)],
            "element": format!("element_{}", fastrand::u32(0..100)),
            "page": ["/dashboard", "/profile", "/settings", "/analytics"][fastrand::usize(0..4)],
            "metadata": {
                "browser": "Chrome",
                "version": "120.0.0.0",
                "platform": "Linux",
                "screen_resolution": "1920x1080",
                "test_tool": "goose",
                "user_index": user.weighted_users_index
            }
        }
    });

    let _response = user
        .post("/api/events")
        .json(&event_payload)
        .await?;

    Ok(())
}

async fn get_events(user: &mut GooseUser) -> TransactionResult {
    // Get events with random pagination
    let limit = fastrand::u32(10..50);
    let offset = fastrand::u32(0..100);
    
    let _response = user
        .get(&format!("/api/events?limit={}&offset={}", limit, offset))
        .await?;
    
    Ok(())
}

async fn update_event(user: &mut GooseUser) -> TransactionResult {
    // Try to update a random event (may get 404, that's ok)
    let event_id = format!("event_{}", fastrand::u32(1..1000));
    
    let update_payload = json!({
        "metadata": {
            "updated_at": chrono::Utc::now().to_rfc3339(),
            "updated_by": format!("goose_user_{}", user.weighted_users_index),
            "update_reason": "goose_load_test"
        }
    });
    
    let _response = user
        .put(&format!("/api/events/{}", event_id))
        .json(&update_payload)
        .await?;
    
    Ok(())
}
EOF
}

# Run Goose load test
run_goose_test() {
    echo "🦆 Starting Goose load test..."
    
    # Create dynamic configuration
    create_goose_config
    
    # Compile and run the dynamic Goose test
    echo "🔨 Compiling Goose test..."
    if rustc --version >/dev/null 2>&1; then
        # Add to Cargo.toml temporarily or use existing binary
        cargo run --bin goose_load_test > "$RESULTS_DIR/goose_output_$TIMESTAMP.log" 2>&1
    else
        echo "❌ Rust not found. Using pre-compiled Goose binary..."
        # Try to use existing binary
        if [[ -f "target/debug/goose_load_test" ]]; then
            ./target/debug/goose_load_test > "$RESULTS_DIR/goose_output_$TIMESTAMP.log" 2>&1
        else
            echo "❌ No Goose binary found. Please compile first with: cargo build"
            exit 1
        fi
    fi
    
    echo "✅ Goose test completed"
}

# Generate report
generate_report() {
    echo "📋 Generating Goose report..."
    
    local report_file="$RESULTS_DIR/goose_report_$TIMESTAMP.md"
    
    cat > "$report_file" <<EOF
# Goose Load Test Report

**Test Date:** $(date)
**Target:** $TARGET_URL
**Users:** $USERS
**Hatch Rate:** $HATCH_RATE
**Run Time:** ${RUN_TIME}s
**Test ID:** $TIMESTAMP

## Test Configuration

- **Tool:** Goose (Rust-based load testing)
- **Scenarios:** Health Check (10%), Event Creation (50%), Event Retrieval (30%), Event Update (10%)
- **Target URL:** $TARGET_URL
- **Virtual Users:** $USERS
- **Hatch Rate:** $HATCH_RATE
- **Duration:** ${RUN_TIME} seconds

## Results Summary

EOF

    # Parse Goose output for key metrics
    if [[ -f "$RESULTS_DIR/goose_output_$TIMESTAMP.log" ]]; then
        echo "### Performance Metrics" >> "$report_file"
        echo "" >> "$report_file"
        echo "\`\`\`" >> "$report_file"
        grep -E "(requests|users|response|error)" "$RESULTS_DIR/goose_output_$TIMESTAMP.log" | head -20 >> "$report_file" 2>/dev/null || echo "See detailed log for metrics" >> "$report_file"
        echo "\`\`\`" >> "$report_file"
    fi
    
    cat >> "$report_file" <<EOF

## Files Generated

- Output log: \`goose_output_$TIMESTAMP.log\`
- HTML report: \`goose-report-$TIMESTAMP.html\` (if generated)
- This report: \`goose_report_$TIMESTAMP.md\`

## Analysis

### Goose Advantages
- Native Rust performance and safety
- Excellent async/await support
- Built-in metrics collection
- Realistic user simulation

### Key Metrics to Review
- Requests per second by scenario
- Response time percentiles
- Error rates by transaction type
- User ramp-up patterns

## Next Steps

1. Review HTML report for detailed visualizations
2. Compare with other load testing tools
3. Analyze any failed transactions
4. Use results for performance optimization

EOF

    echo "✅ Report generated: $report_file"
}

# Cleanup
cleanup() {
    rm -f goose_dynamic.rs 2>/dev/null || true
}

# Main execution
main() {
    echo "🎯 Starting Goose performance test..."
    
    check_server_health
    run_goose_test
    generate_report
    cleanup
    
    # Move any generated HTML reports
    mv goose-report-*.html "$RESULTS_DIR/" 2>/dev/null || true
    
    echo ""
    echo "🎉 Goose testing completed!"
    echo "📊 Results directory: $RESULTS_DIR"
    echo "📋 Report: $RESULTS_DIR/goose_report_$TIMESTAMP.md"
    
    if [[ -f "$RESULTS_DIR/goose-report-$TIMESTAMP.html" ]]; then
        echo "🌐 HTML Report: $RESULTS_DIR/goose-report-$TIMESTAMP.html"
    fi
    
    echo ""
    echo "💡 Usage examples:"
    echo "   ./run-goose.sh                                    # Default test"
    echo "   ./run-goose.sh http://localhost:8080 500         # 500 users"
    echo "   ./run-goose.sh http://localhost:8080 1000 50/1s  # Custom hatch rate"
    echo "   ./run-goose.sh http://localhost:8080 2000 100/1s 600  # 10 minute test"
}

# Handle interruption
trap cleanup EXIT

# Show usage
if [[ "$1" == "--help" || "$1" == "-h" ]]; then
    echo "Usage: $0 [TARGET_URL] [USERS] [HATCH_RATE] [RUN_TIME]"
    echo ""
    echo "Arguments:"
    echo "  TARGET_URL    Target URL (default: http://localhost:8080)"
    echo "  USERS         Number of virtual users (default: 1000)"
    echo "  HATCH_RATE    User spawn rate (default: 100/1s)"
    echo "  RUN_TIME      Test duration in seconds (default: 300)"
    echo ""
    echo "Examples:"
    echo "  $0                                    # Default test"
    echo "  $0 http://localhost:8080 500         # 500 users"
    echo "  $0 http://localhost:8080 1000 50/1s  # Custom spawn rate"
    echo "  $0 http://localhost:8080 2000 100/1s 600  # 10 minute test"
    exit 0
fi

# Run main function
main "$@"