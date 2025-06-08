#!/bin/bash

# Criterion Benchmarking Script for Collider
# Micro-benchmarking with statistical analysis

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_URL=${1:-"http://app:8080"}
BENCHMARK_TYPE=${2:-"all"}
RESULTS_DIR="criterion_results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

echo "📊 Criterion Benchmarking Suite"
echo "==============================="
echo "Target: $TARGET_URL"
echo "Benchmark Type: $BENCHMARK_TYPE"
echo "Results: $RESULTS_DIR"
echo ""

# Create results directory
mkdir -p "$RESULTS_DIR"

# Check server health
check_server_health() {
    echo "🔍 Checking server health..."
    if ! curl -f "$TARGET_URL/health" &>/dev/null; then
        echo "❌ Server health check failed: $TARGET_URL"
        exit 1
    fi
    echo "✅ Server is healthy"
}

# Run Criterion benchmarks
run_criterion_benchmarks() {
    echo "📊 Running Criterion benchmarks..."
    
    local benchmark_args=""
    
    case "$BENCHMARK_TYPE" in
        "http")
            benchmark_args="--bench http_bench -- benchmark_http_requests"
            ;;
        "concurrent")
            benchmark_args="--bench http_bench -- benchmark_concurrent_requests"
            ;;
        "all")
            benchmark_args="--bench http_bench"
            ;;
        "quick")
            benchmark_args="--bench http_bench -- --sample-size 10 --measurement-time 5"
            ;;
        *)
            echo "❌ Unknown benchmark type: $BENCHMARK_TYPE"
            echo "Available types: http, concurrent, all, quick"
            exit 1
            ;;
    esac
    
    echo "🔨 Running: cargo bench $benchmark_args"
    
    # Set the target URL for the benchmark
    export CRITERION_TARGET_URL="$TARGET_URL"
    
    # Run the benchmarks
    cargo bench $benchmark_args 2>&1 | tee "$RESULTS_DIR/criterion_output_$TIMESTAMP.log"
    
    # Copy criterion results
    if [[ -d "target/criterion" ]]; then
        echo "📁 Copying Criterion results..."
        cp -r target/criterion "$RESULTS_DIR/criterion_$TIMESTAMP"
        
        # Create symlink to latest results
        rm -f "$RESULTS_DIR/latest"
        ln -s "criterion_$TIMESTAMP" "$RESULTS_DIR/latest"
    fi
    
    echo "✅ Criterion benchmarks completed"
}

# Generate comprehensive report
generate_report() {
    echo "📋 Generating Criterion report..."
    
    local report_file="$RESULTS_DIR/criterion_report_$TIMESTAMP.md"
    
    cat > "$report_file" <<EOF
# Criterion Benchmark Report

**Test Date:** $(date)
**Target:** $TARGET_URL
**Benchmark Type:** $BENCHMARK_TYPE
**Test ID:** $TIMESTAMP

## Overview

Criterion provides statistical analysis of micro-benchmarks with:
- Statistical significance testing
- Outlier detection and removal
- Performance regression detection
- Detailed timing analysis

## Test Configuration

- **Tool:** Criterion (Rust statistical benchmarking)
- **Target URL:** $TARGET_URL
- **Benchmark Type:** $BENCHMARK_TYPE
- **Sample Collection:** Automated based on stability

EOF

    # Parse criterion results for key metrics
    if [[ -f "$RESULTS_DIR/criterion_output_$TIMESTAMP.log" ]]; then
        echo "## Benchmark Results" >> "$report_file"
        echo "" >> "$report_file"
        
        # Extract benchmark summaries
        echo "### Performance Summary" >> "$report_file"
        echo "" >> "$report_file"
        echo "\`\`\`" >> "$report_file"
        grep -A 5 -B 1 "time:" "$RESULTS_DIR/criterion_output_$TIMESTAMP.log" >> "$report_file" 2>/dev/null || echo "See detailed log for metrics" >> "$report_file"
        echo "\`\`\`" >> "$report_file"
        echo "" >> "$report_file"
        
        # Look for performance regressions
        if grep -q "Performance has regressed" "$RESULTS_DIR/criterion_output_$TIMESTAMP.log"; then
            echo "### ⚠️ Performance Regressions Detected" >> "$report_file"
            echo "" >> "$report_file"
            echo "\`\`\`" >> "$report_file"
            grep -A 3 -B 1 "Performance has regressed" "$RESULTS_DIR/criterion_output_$TIMESTAMP.log" >> "$report_file"
            echo "\`\`\`" >> "$report_file"
            echo "" >> "$report_file"
        fi
        
        # Look for performance improvements
        if grep -q "Performance has improved" "$RESULTS_DIR/criterion_output_$TIMESTAMP.log"; then
            echo "### ✅ Performance Improvements Detected" >> "$report_file"
            echo "" >> "$report_file"
            echo "\`\`\`" >> "$report_file"
            grep -A 3 -B 1 "Performance has improved" "$RESULTS_DIR/criterion_output_$TIMESTAMP.log" >> "$report_file"
            echo "\`\`\`" >> "$report_file"
            echo "" >> "$report_file"
        fi
    fi
    
    # Add detailed analysis section
    cat >> "$report_file" <<EOF

## Detailed Results

### HTML Reports
EOF

    # List HTML reports if available
    if [[ -d "$RESULTS_DIR/criterion_$TIMESTAMP" ]]; then
        find "$RESULTS_DIR/criterion_$TIMESTAMP" -name "report.html" | while read -r html_file; do
            local bench_name=$(basename "$(dirname "$html_file")")
            echo "- **$bench_name**: [\`$(basename "$(dirname "$html_file")")/report.html\`]($html_file)" >> "$report_file"
        done
    fi
    
    cat >> "$report_file" <<EOF

### Result Files
- Benchmark output: \`criterion_output_$TIMESTAMP.log\`
- Criterion data: \`criterion_$TIMESTAMP/\`
- Latest results symlink: \`latest/\`

## Statistical Analysis

Criterion provides:

### Measurement Quality
- **Outlier Detection**: Automatic detection and handling of measurement outliers
- **Statistical Tests**: Student's t-test for performance regression detection
- **Confidence Intervals**: 95% confidence intervals for all measurements
- **Sample Size**: Adaptive sample collection until stable results

### Performance Insights
- **Mean Execution Time**: Average time per operation
- **Standard Deviation**: Measurement variability
- **Median**: 50th percentile execution time
- **MAD (Median Absolute Deviation)**: Robust measure of variability

## Benchmark Types

### HTTP Request Benchmarks
- Individual request latency measurement
- Statistical analysis of response times
- Comparison with baseline performance

### Concurrent Request Benchmarks
- Parallel request handling capability
- Scalability analysis across different concurrency levels
- Contention and resource utilization patterns

## Recommendations

### Performance Analysis
1. **Review Mean Times**: Focus on the mean execution times for typical performance
2. **Check Variability**: High standard deviation indicates inconsistent performance
3. **Baseline Comparison**: Use regression detection for performance monitoring
4. **HTML Reports**: Detailed visualizations available in HTML format

### Action Items
EOF

    # Add conditional recommendations based on results
    if [[ -f "$RESULTS_DIR/criterion_output_$TIMESTAMP.log" ]]; then
        if grep -q "regressed" "$RESULTS_DIR/criterion_output_$TIMESTAMP.log"; then
            cat >> "$report_file" <<EOF
- 🔴 **Regressions Found**: Investigate performance degradation causes
- 📊 Review detailed HTML reports for affected benchmarks
- 🔍 Check recent code changes that might impact performance
EOF
        elif grep -q "improved" "$RESULTS_DIR/criterion_output_$TIMESTAMP.log"; then
            cat >> "$report_file" <<EOF
- ✅ **Improvements Found**: Performance optimizations detected
- 📈 Document changes that led to improvements
- 🎯 Consider setting new baseline performance targets
EOF
        else
            cat >> "$report_file" <<EOF
- ✅ **Stable Performance**: No significant changes detected
- 📊 Review absolute performance values for optimization opportunities
- 🔄 Continue regular benchmark monitoring
EOF
        fi
    fi
    
    cat >> "$report_file" <<EOF

## Next Steps

1. **Review HTML Reports**: Open detailed reports for visual analysis
2. **Compare with Historical Data**: Track performance trends over time
3. **Integrate with CI/CD**: Run benchmarks automatically on code changes
4. **Set Performance Budgets**: Define acceptable performance thresholds

EOF

    echo "✅ Report generated: $report_file"
}

# Open HTML reports if requested
open_reports() {
    if [[ "$1" == "--open" && -d "$RESULTS_DIR/criterion_$TIMESTAMP" ]]; then
        echo "🌐 Opening HTML reports..."
        
        # Find and open HTML reports
        find "$RESULTS_DIR/criterion_$TIMESTAMP" -name "report.html" | head -3 | while read -r html_file; do
            if command -v open >/dev/null 2>&1; then
                open "$html_file"  # macOS
            elif command -v xdg-open >/dev/null 2>&1; then
                xdg-open "$html_file"  # Linux
            else
                echo "📄 Report available: $html_file"
            fi
        done
    fi
}

# Main execution
main() {
    echo "🎯 Starting Criterion benchmark suite..."
    
    check_server_health
    run_criterion_benchmarks
    generate_report
    open_reports "$@"
    
    echo ""
    echo "🎉 Criterion benchmarking completed!"
    echo "📊 Results directory: $RESULTS_DIR"
    echo "📋 Report: $RESULTS_DIR/criterion_report_$TIMESTAMP.md"
    echo "📁 Latest results: $RESULTS_DIR/latest"
    
    if [[ -d "$RESULTS_DIR/criterion_$TIMESTAMP" ]]; then
        echo "🌐 HTML reports: $RESULTS_DIR/criterion_$TIMESTAMP/*/report.html"
    fi
    
    echo ""
    echo "💡 Usage examples:"
    echo "   ./run-criterion.sh                                    # All benchmarks"
    echo "   ./run-criterion.sh http://localhost:8080 http         # HTTP only"
    echo "   ./run-criterion.sh http://localhost:8080 concurrent   # Concurrency only"
    echo "   ./run-criterion.sh http://localhost:8080 quick        # Quick test"
    echo "   ./run-criterion.sh http://localhost:8080 all --open   # Open HTML reports"
}

# Show usage
if [[ "$1" == "--help" || "$1" == "-h" ]]; then
    echo "Usage: $0 [TARGET_URL] [BENCHMARK_TYPE] [--open]"
    echo ""
    echo "Arguments:"
    echo "  TARGET_URL       Target URL (default: http://localhost:8080)"
    echo "  BENCHMARK_TYPE   Type of benchmarks to run (default: all)"
    echo "  --open          Open HTML reports after completion"
    echo ""
    echo "Benchmark Types:"
    echo "  http            HTTP request latency benchmarks"
    echo "  concurrent      Concurrent request benchmarks"
    echo "  all             All available benchmarks"
    echo "  quick           Quick test with reduced samples"
    echo ""
    echo "Examples:"
    echo "  $0                                    # All benchmarks"
    echo "  $0 http://localhost:8080 http         # HTTP benchmarks only"
    echo "  $0 http://localhost:8080 quick        # Quick validation"
    echo "  $0 http://localhost:8080 all --open   # Run all and open reports"
    exit 0
fi

# Run main function
main "$@"