#!/bin/bash

# Unified Performance Testing Orchestration Script
# Runs all performance testing tools in sequence with comprehensive reporting

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_URL=${1:-"http://localhost:8080"}
TEST_SUITE=${2:-"full"}
RESULTS_DIR="orchestrated_results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
MONITORING_ENABLED=${MONITORING_ENABLED:-true}

echo "🎯 Collider Performance Testing Orchestration"
echo "=============================================="
echo "Target: $TARGET_URL"
echo "Test Suite: $TEST_SUITE"
echo "Results: $RESULTS_DIR"
echo "Monitoring: $MONITORING_ENABLED"
echo ""

# Configuration
TOOLS_CONFIG=(
    "vegeta:run_load_test.sh:5:Vegeta HTTP load testing"
    "goose:goose_load_test.rs:3:Goose Rust-based load testing"
    "criterion:criterion_bench.rs:2:Criterion micro-benchmarking"
    "yandex-tank:yandex-tank/run_tank.sh:8:Yandex Tank comprehensive load testing"  
    "k6:k6/run-k6.sh:6:Grafana k6 JavaScript-based testing"
    "critical:critical-performance-test.sh:15:CRITICAL extreme scale testing (100k RPS)"
)

# Create results directory
mkdir -p "$RESULTS_DIR"

# Function to check tool availability
check_tool_availability() {
    local tool_name=$1
    local tool_script=$2
    
    if [[ -f "$SCRIPT_DIR/$tool_script" ]]; then
        echo "✅ $tool_name script found"
        return 0
    else
        echo "❌ $tool_name script not found: $tool_script"
        return 1
    fi
}

# Function to start monitoring
start_monitoring() {
    if [[ "$MONITORING_ENABLED" == "true" ]]; then
        echo "📊 Starting monitoring stack..."
        
        # Check if monitoring is already running
        if ! docker ps | grep -q "prometheus\|grafana"; then
            # Start monitoring using existing config
            cd "$SCRIPT_DIR/../config/monitoring" || {
                echo "⚠️  Monitoring config not found, continuing without monitoring"
                return 1
            }
            
            # Start basic monitoring services
            echo "Starting Prometheus and Grafana..."
            docker run -d \
                --name perf-prometheus \
                -p 9090:9090 \
                -v "$(pwd)/prometheus/prometheus.yml:/etc/prometheus/prometheus.yml:ro" \
                prom/prometheus:latest >/dev/null 2>&1 || true
                
            docker run -d \
                --name perf-grafana \
                -p 3000:3000 \
                -e GF_SECURITY_ADMIN_PASSWORD=admin \
                grafana/grafana:latest >/dev/null 2>&1 || true
            
            echo "✅ Monitoring started (Prometheus: 9090, Grafana: 3000)"
            cd "$SCRIPT_DIR"
        else
            echo "✅ Monitoring already running"
        fi
    fi
}

# Function to stop monitoring
stop_monitoring() {
    if [[ "$MONITORING_ENABLED" == "true" ]]; then
        echo "🛑 Stopping monitoring stack..."
        docker stop perf-prometheus perf-grafana >/dev/null 2>&1 || true
        docker rm perf-prometheus perf-grafana >/dev/null 2>&1 || true
        echo "✅ Monitoring stopped"
    fi
}

# Function to verify server health
verify_server_health() {
    echo "🔍 Verifying server health..."
    
    local max_attempts=5
    local attempt=1
    
    while [[ $attempt -le $max_attempts ]]; do
        if curl -f "$TARGET_URL/health" >/dev/null 2>&1; then
            echo "✅ Server is healthy"
            return 0
        fi
        
        echo "⏳ Attempt $attempt/$max_attempts failed, retrying..."
        sleep 5
        ((attempt++))
    done
    
    echo "❌ Server health check failed after $max_attempts attempts"
    return 1
}

# Function to run a performance test tool
run_performance_tool() {
    local tool_name=$1
    local tool_script=$2
    local estimated_minutes=$3
    local description=$4
    local test_type=${5:-"load"}
    
    echo ""
    echo "🚀 Running $tool_name"
    echo "Description: $description"
    echo "Estimated time: $estimated_minutes minutes"
    echo "----------------------------------------"
    
    local start_time=$(date +%s)
    local tool_results_dir="$RESULTS_DIR/${tool_name}_${TIMESTAMP}"
    
    # Create tool-specific results directory
    mkdir -p "$tool_results_dir"
    
    # Change to tool directory and run
    local tool_dir="$(dirname "$SCRIPT_DIR/$tool_script")"
    local script_name="$(basename "$tool_script")"
    
    cd "$tool_dir" || {
        echo "❌ Failed to change to $tool_dir"
        return 1
    }
    
    # Run the tool with appropriate parameters
    case "$tool_name" in
        "vegeta")
            bash "$script_name" "${TARGET_URL#http://}" > "$tool_results_dir/output.log" 2>&1
            ;;
        "goose")
            echo "🦆 Running Goose load test..." >> "$tool_results_dir/output.log"
            cargo run --bin goose_load_test >> "$tool_results_dir/output.log" 2>&1
            ;;
        "criterion")
            echo "📊 Running Criterion benchmarks..." >> "$tool_results_dir/output.log"
            cargo bench --bench http_bench >> "$tool_results_dir/output.log" 2>&1
            # Copy criterion results
            if [[ -d "target/criterion" ]]; then
                cp -r target/criterion "$tool_results_dir/" 2>/dev/null || true
            fi
            ;;
        "yandex-tank")
            bash "$script_name" "$TARGET_URL" > "$tool_results_dir/output.log" 2>&1
            ;;
        "k6")
            bash "$script_name" "$TARGET_URL" "$test_type" > "$tool_results_dir/output.log" 2>&1
            ;;
        "critical")
            echo "⚠️  CRITICAL TESTING - This will generate extreme load!" >> "$tool_results_dir/output.log"
            bash "$script_name" "$TARGET_URL" 100000 600 5000000 > "$tool_results_dir/output.log" 2>&1
            ;;
        *)
            bash "$script_name" "$TARGET_URL" > "$tool_results_dir/output.log" 2>&1
            ;;
    esac
    
    local exit_code=$?
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    # Copy results to orchestrated results directory
    if [[ -d "results" ]]; then
        cp -r results/* "$tool_results_dir/" 2>/dev/null || true
    fi
    
    if [[ -d "${tool_name}_results" ]]; then
        cp -r "${tool_name}_results"/* "$tool_results_dir/" 2>/dev/null || true
    fi
    
    # Copy tool-specific result patterns
    case "$tool_name" in
        "vegeta")
            cp -r results/* "$tool_results_dir/" 2>/dev/null || true
            ;;
        "goose")
            # Goose creates report files in current directory
            mv goose-report* "$tool_results_dir/" 2>/dev/null || true
            ;;
        "criterion")
            # Criterion results already copied above
            ;;
        "yandex-tank")
            cp -r tank_results/* "$tool_results_dir/" 2>/dev/null || true
            ;;
        "k6")
            cp -r k6_results/* "$tool_results_dir/" 2>/dev/null || true
            ;;
        "critical")
            cp -r critical_performance_results/* "$tool_results_dir/" 2>/dev/null || true
            ;;
    esac
    
    cd "$SCRIPT_DIR"
    
    if [[ $exit_code -eq 0 ]]; then
        echo "✅ $tool_name completed successfully in ${duration}s"
    else
        echo "⚠️  $tool_name completed with warnings/errors in ${duration}s"
    fi
    
    # Store tool summary
    cat > "$tool_results_dir/summary.txt" <<EOF
Tool: $tool_name
Description: $description
Target: $TARGET_URL
Start Time: $(date -d "@$start_time" 2>/dev/null || date -r $start_time)
End Time: $(date -d "@$end_time" 2>/dev/null || date -r $end_time)
Duration: ${duration}s
Exit Code: $exit_code
Results Directory: $tool_results_dir
EOF
    
    return $exit_code
}

# Function to generate comprehensive report
generate_orchestration_report() {
    echo "📋 Generating comprehensive performance test report..."
    
    local report_file="$RESULTS_DIR/orchestration_report_${TIMESTAMP}.md"
    
    cat > "$report_file" <<EOF
# Collider Performance Testing Orchestration Report

**Test Date:** $(date)
**Target URL:** $TARGET_URL
**Test Suite:** $TEST_SUITE
**Test ID:** $TIMESTAMP

## Executive Summary

This report consolidates results from multiple performance testing tools run against the Collider application.

## Test Configuration

- **Target URL:** $TARGET_URL
- **Test Suite:** $TEST_SUITE
- **Monitoring Enabled:** $MONITORING_ENABLED
- **Total Tools Run:** ${#COMPLETED_TOOLS[@]}

## Tools Executed

EOF
    
    # Add tool summaries
    for tool_dir in "$RESULTS_DIR"/*_"$TIMESTAMP"; do
        if [[ -d "$tool_dir" && -f "$tool_dir/summary.txt" ]]; then
            echo "### $(basename "$tool_dir" | sed "s/_${TIMESTAMP}//")" >> "$report_file"
            echo "" >> "$report_file"
            echo "\`\`\`" >> "$report_file"
            cat "$tool_dir/summary.txt" >> "$report_file"
            echo "\`\`\`" >> "$report_file"
            echo "" >> "$report_file"
        fi
    done
    
    cat >> "$report_file" <<EOF

## Performance Comparison

| Tool | Duration | Exit Code | Results Available |
|------|----------|-----------|-------------------|
EOF
    
    # Add comparison table
    for tool_dir in "$RESULTS_DIR"/*_"$TIMESTAMP"; do
        if [[ -d "$tool_dir" && -f "$tool_dir/summary.txt" ]]; then
            local tool_name=$(basename "$tool_dir" | sed "s/_${TIMESTAMP}//")
            local duration=$(grep "Duration:" "$tool_dir/summary.txt" | cut -d: -f2 | tr -d ' ')
            local exit_code=$(grep "Exit Code:" "$tool_dir/summary.txt" | cut -d: -f2 | tr -d ' ')
            local has_results="Yes"
            
            echo "| $tool_name | $duration | $exit_code | $has_results |" >> "$report_file"
        fi
    done
    
    cat >> "$report_file" <<EOF

## Key Findings

### Performance Metrics
- Review individual tool reports for detailed performance metrics
- Compare response times across different testing methodologies
- Analyze throughput capabilities under various load patterns

### Recommendations

1. **Compare Results**: Each tool provides different perspectives on performance
2. **Identify Bottlenecks**: Look for consistent patterns across all tools
3. **Validate Findings**: Use multiple tools to confirm performance characteristics
4. **Monitor Trends**: Establish baseline performance for regression testing

## Detailed Results

Detailed results for each tool can be found in their respective directories:

EOF
    
    for tool_dir in "$RESULTS_DIR"/*_"$TIMESTAMP"; do
        if [[ -d "$tool_dir" ]]; then
            echo "- **$(basename "$tool_dir" | sed "s/_${TIMESTAMP}//")**: \`$tool_dir/\`" >> "$report_file"
        fi
    done
    
    cat >> "$report_file" <<EOF

## Next Steps

1. Analyze detailed results from each tool
2. Identify performance bottlenecks and optimization opportunities
3. Set up automated performance regression testing
4. Establish performance benchmarks for future releases

EOF
    
    echo "✅ Orchestration report generated: $report_file"
}

# Cleanup function
cleanup() {
    echo ""
    echo "🧹 Cleaning up..."
    
    # Stop monitoring if we started it
    if [[ "$MONITORING_ENABLED" == "true" ]]; then
        stop_monitoring
    fi
    
    echo "✅ Cleanup completed"
}

# Main execution function
main() {
    local failed_tools=()
    local completed_tools=()
    
    echo "🎯 Starting orchestrated performance testing..."
    
    # Verify server health
    if ! verify_server_health; then
        echo "❌ Server health check failed. Aborting tests."
        exit 1
    fi
    
    # Start monitoring
    start_monitoring
    
    # Check tool availability
    echo "🔍 Checking tool availability..."
    for tool_config in "${TOOLS_CONFIG[@]}"; do
        IFS=':' read -r tool_name tool_script estimated_minutes description <<< "$tool_config"
        
        if check_tool_availability "$tool_name" "$tool_script"; then
            completed_tools+=("$tool_name")
        else
            failed_tools+=("$tool_name")
        fi
    done
    
    if [[ ${#completed_tools[@]} -eq 0 ]]; then
        echo "❌ No performance testing tools available"
        exit 1
    fi
    
    echo "✅ Available tools: ${completed_tools[*]}"
    if [[ ${#failed_tools[@]} -gt 0 ]]; then
        echo "⚠️  Unavailable tools: ${failed_tools[*]}"
    fi
    echo ""
    
    # Run performance tests based on suite selection
    case "$TEST_SUITE" in
        "quick")
            echo "🏃 Running quick test suite (k6 smoke test only)"
            run_performance_tool "k6" "k6/run-k6.sh" 2 "Quick smoke test" "smoke"
            ;;
        "load")
            echo "📈 Running load test suite (all tools, load testing)"
            for tool_config in "${TOOLS_CONFIG[@]}"; do
                IFS=':' read -r tool_name tool_script estimated_minutes description <<< "$tool_config"
                # Skip critical testing in regular load suite
                if [[ "$tool_name" == "critical" ]]; then
                    continue
                fi
                if [[ " ${completed_tools[*]} " =~ " $tool_name " ]]; then
                    run_performance_tool "$tool_name" "$tool_script" "$estimated_minutes" "$description" "load"
                    sleep 30  # Cool-down between tools
                fi
            done
            ;;
        "stress")
            echo "💪 Running stress test suite"
            run_performance_tool "k6" "k6/run-k6.sh" 8 "k6 stress testing" "stress"
            sleep 60
            run_performance_tool "yandex-tank" "yandex-tank/run_tank.sh" 10 "Yandex Tank stress testing"
            ;;
        "critical")
            echo "🔥 Running CRITICAL performance testing (EXTREME LOAD WARNING!)"
            echo "⚠️  This test will generate 100k+ RPS and test with millions of events"
            echo "⚠️  Ensure adequate system resources and monitoring"
            sleep 5
            run_performance_tool "critical" "critical-performance-test.sh" 15 "CRITICAL extreme scale testing" "critical"
            ;;
        "full")
            echo "🎯 Running full test suite (all tools, all test types)"
            # Quick validation first
            run_performance_tool "k6" "k6/run-k6.sh" 1 "Initial smoke test" "smoke"
            sleep 30
            
            # Main load tests (excluding critical)
            for tool_config in "${TOOLS_CONFIG[@]}"; do
                IFS=':' read -r tool_name tool_script estimated_minutes description <<< "$tool_config"
                # Skip critical testing in full suite (run separately)
                if [[ "$tool_name" == "critical" ]]; then
                    continue
                fi
                if [[ " ${completed_tools[*]} " =~ " $tool_name " ]]; then
                    run_performance_tool "$tool_name" "$tool_script" "$estimated_minutes" "$description" "load"
                    sleep 60  # Longer cool-down for full suite
                fi
            done
            
            # Stress testing
            echo "🔥 Starting stress testing phase..."
            sleep 120  # Cool-down before stress tests
            run_performance_tool "k6" "k6/run-k6.sh" 8 "k6 stress testing" "stress"
            ;;
        *)
            echo "❌ Unknown test suite: $TEST_SUITE"
            echo "Available suites: quick, load, stress, critical, full"
            exit 1
            ;;
    esac
    
    # Generate comprehensive report
    generate_orchestration_report
    
    echo ""
    echo "🎉 Performance testing orchestration completed!"
    echo "📊 Results directory: $RESULTS_DIR"
    echo "📋 Orchestration report: $RESULTS_DIR/orchestration_report_${TIMESTAMP}.md"
    
    if [[ "$MONITORING_ENABLED" == "true" ]]; then
        echo "📈 Grafana dashboard: http://localhost:3000 (admin/admin)"
        echo "📊 Prometheus: http://localhost:9090"
    fi
    
    echo ""
    echo "💡 Next steps:"
    echo "   1. Review the orchestration report"
    echo "   2. Analyze individual tool results"
    echo "   3. Check monitoring dashboards if enabled"
    echo "   4. Compare performance across different tools"
}

# Handle script interruption
trap cleanup EXIT

# Show usage if help requested
if [[ "$1" == "--help" || "$1" == "-h" ]]; then
    echo "Usage: $0 [TARGET_URL] [TEST_SUITE]"
    echo ""
    echo "Arguments:"
    echo "  TARGET_URL    Target URL for testing (default: http://localhost:8080)"
    echo "  TEST_SUITE    Test suite to run (default: full)"
    echo ""
    echo "Test Suites:"
    echo "  quick         Quick validation (k6 smoke test only) - ~2 minutes"
    echo "  load          Load testing with all tools - ~20 minutes"  
    echo "  stress        Stress testing focused suite - ~20 minutes"
    echo "  full          Complete test suite - ~45 minutes"
    echo ""
    echo "Environment Variables:"
    echo "  MONITORING_ENABLED    Enable monitoring stack (default: true)"
    echo ""
    echo "Examples:"
    echo "  $0                                    # Full suite on localhost"
    echo "  $0 http://localhost:8080 quick        # Quick test"
    echo "  $0 http://localhost:8080 critical     # CRITICAL 100k RPS testing"
    echo "  MONITORING_ENABLED=false $0 load      # Load test without monitoring"
    echo ""
    exit 0
fi

# Store completed tools for reporting
declare -a COMPLETED_TOOLS

# Run main function
main "$@"