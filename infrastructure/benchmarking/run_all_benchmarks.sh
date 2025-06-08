#!/bin/bash

# Comprehensive benchmark runner for docker-compose environment
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_URL=${TARGET_URL:-"http://app:8080"}
RESULTS_DIR="/app/results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

echo "🚀 Collider Benchmarking Suite (Docker Compose)"
echo "==============================================="
echo "Target: $TARGET_URL"
echo "Results: $RESULTS_DIR"
echo "Timestamp: $TIMESTAMP"
echo ""

# Create results directory
mkdir -p "$RESULTS_DIR"

# Wait for server to be ready
wait_for_server() {
    echo "⏳ Waiting for server to be ready..."
    local max_attempts=30
    local attempt=1
    
    while [[ $attempt -le $max_attempts ]]; do
        if curl -f "$TARGET_URL/health" &>/dev/null; then
            echo "✅ Server is ready"
            return 0
        fi
        
        echo "Attempt $attempt/$max_attempts - waiting for server..."
        sleep 5
        ((attempt++))
    done
    
    echo "❌ Server failed to become ready after $max_attempts attempts"
    exit 1
}

# Run Criterion benchmarks
run_criterion() {
    echo "📊 Running Criterion benchmarks..."
    
    if command -v criterion_bench >/dev/null 2>&1; then
        criterion_bench 2>&1 | tee "$RESULTS_DIR/criterion_output_$TIMESTAMP.log"
        echo "✅ Criterion benchmarks completed"
    else
        echo "⚠️ Criterion benchmark binary not found"
    fi
}

# Run Goose load tests
run_goose() {
    echo "🦆 Running Goose load tests..."
    
    if command -v goose_load_test >/dev/null 2>&1; then
        goose_load_test --host "$TARGET_URL" \
            --users 100 \
            --hatch-rate 10 \
            --run-time 60s \
            --report-file "$RESULTS_DIR/goose_report_$TIMESTAMP.html" \
            2>&1 | tee "$RESULTS_DIR/goose_output_$TIMESTAMP.log"
        echo "✅ Goose load tests completed"
    else
        echo "⚠️ Goose load test binary not found"
    fi
}

# Run workspace benchmarks
run_workspace_benchmarks() {
    echo "🔧 Running workspace benchmarks..."
    
    cd /app
    if [[ -f "Cargo.toml" ]]; then
        # Set environment for benchmarks
        export CRITERION_TARGET_URL="$TARGET_URL"
        
        # Run cargo benchmarks
        cargo bench --all 2>&1 | tee "$RESULTS_DIR/workspace_bench_output_$TIMESTAMP.log"
        echo "✅ Workspace benchmarks completed"
    else
        echo "⚠️ Cargo.toml not found for workspace benchmarks"
    fi
}

# Generate summary report
generate_summary() {
    echo "📋 Generating benchmark summary..."
    
    local summary_file="$RESULTS_DIR/benchmark_summary_$TIMESTAMP.md"
    
    cat > "$summary_file" <<EOF
# Collider Benchmark Summary

**Test Date:** $(date)
**Target:** $TARGET_URL
**Environment:** Docker Compose
**Test ID:** $TIMESTAMP

## Test Overview

This report contains results from multiple benchmarking tools:

### 1. Criterion (Statistical Micro-benchmarks)
- **Purpose:** Precise latency measurements with statistical analysis
- **Focus:** Individual endpoint performance and regression detection
- **Output:** \`criterion_output_$TIMESTAMP.log\`

### 2. Goose (Load Testing)
- **Purpose:** Realistic load simulation with concurrent users
- **Focus:** System behavior under load and scalability assessment
- **Output:** \`goose_output_$TIMESTAMP.log\`, \`goose_report_$TIMESTAMP.html\`

### 3. Workspace Benchmarks
- **Purpose:** Domain-specific and backend component benchmarks
- **Focus:** Internal performance of business logic and data access
- **Output:** \`workspace_bench_output_$TIMESTAMP.log\`

## Quick Results

EOF

    # Add quick metrics if available
    if [[ -f "$RESULTS_DIR/criterion_output_$TIMESTAMP.log" ]]; then
        echo "### Criterion Results" >> "$summary_file"
        echo "\`\`\`" >> "$summary_file"
        grep -E "(time:|Performance has)" "$RESULTS_DIR/criterion_output_$TIMESTAMP.log" | head -10 >> "$summary_file" 2>/dev/null || echo "See detailed log for metrics" >> "$summary_file"
        echo "\`\`\`" >> "$summary_file"
        echo "" >> "$summary_file"
    fi
    
    if [[ -f "$RESULTS_DIR/goose_output_$TIMESTAMP.log" ]]; then
        echo "### Goose Results" >> "$summary_file"
        echo "\`\`\`" >> "$summary_file"
        grep -E "(users|requests|response time)" "$RESULTS_DIR/goose_output_$TIMESTAMP.log" | tail -10 >> "$summary_file" 2>/dev/null || echo "See detailed log for metrics" >> "$summary_file"
        echo "\`\`\`" >> "$summary_file"
        echo "" >> "$summary_file"
    fi
    
    cat >> "$summary_file" <<EOF

## File Locations

All benchmark results are stored in: \`$RESULTS_DIR\`

- **Criterion:** Statistical micro-benchmarks with HTML reports
- **Goose:** Load testing with detailed HTML dashboard
- **Workspace:** Backend component benchmarks

## Next Steps

1. **Review HTML Reports:** Open detailed visualizations
2. **Compare Baselines:** Track performance over time
3. **Investigate Issues:** Focus on any regressions or errors
4. **Optimize:** Use insights to improve performance

## Running Benchmarks

To run benchmarks in docker-compose environment:

\`\`\`bash
# Run all benchmarks
docker-compose -f docker-compose.yml -f infrastructure/benchmarking/docker-compose-bench.yml --profile bench up bench-runner

# Run K6 load tests
docker-compose -f docker-compose.yml -f infrastructure/benchmarking/docker-compose-bench.yml --profile k6 run k6 run /scripts/load-test.js

# Run individual tools
docker-compose exec app cargo bench
\`\`\`

EOF

    echo "✅ Summary generated: $summary_file"
}

# Main execution
main() {
    wait_for_server
    
    echo "🎯 Starting comprehensive benchmark suite..."
    echo ""
    
    # Run all benchmark types
    run_criterion
    echo ""
    
    run_goose
    echo ""
    
    run_workspace_benchmarks
    echo ""
    
    generate_summary
    
    echo ""
    echo "🎉 All benchmarks completed!"
    echo "📊 Results directory: $RESULTS_DIR"
    echo "📋 Summary: $RESULTS_DIR/benchmark_summary_$TIMESTAMP.md"
    echo ""
    echo "📁 Available reports:"
    ls -la "$RESULTS_DIR"/*"$TIMESTAMP"* 2>/dev/null || echo "  No reports generated"
}

# Show usage
if [[ "$1" == "--help" || "$1" == "-h" ]]; then
    echo "Usage: $0"
    echo ""
    echo "Environment Variables:"
    echo "  TARGET_URL    Target URL (default: http://app:8080)"
    echo ""
    echo "This script runs comprehensive benchmarks in docker-compose environment:"
    echo "  - Criterion statistical micro-benchmarks"
    echo "  - Goose load testing"
    echo "  - Workspace component benchmarks"
    echo ""
    echo "Results are saved to: $RESULTS_DIR"
    exit 0
fi

# Run main function
main "$@"