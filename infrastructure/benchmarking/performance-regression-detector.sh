#!/bin/bash

# Performance Regression Detection and Reporting
# Compares current performance test results against baselines

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASELINE_DIR="${BASELINE_DIR:-$SCRIPT_DIR/baselines}"
RESULTS_DIR="${RESULTS_DIR:-$SCRIPT_DIR/orchestrated_results}"
REGRESSION_THRESHOLD=${REGRESSION_THRESHOLD:-10}  # 10% regression threshold
REPORT_DIR="$SCRIPT_DIR/regression_reports"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

echo "📊 Performance Regression Detection"
echo "=================================="
echo "Baseline Directory: $BASELINE_DIR"
echo "Results Directory: $RESULTS_DIR"
echo "Regression Threshold: ${REGRESSION_THRESHOLD}%"
echo ""

# Create directories
mkdir -p "$BASELINE_DIR" "$REPORT_DIR"

# Function to extract key metrics from different tool outputs
extract_vegeta_metrics() {
    local results_file=$1
    local output_file=$2
    
    if [[ -f "$results_file" ]]; then
        # Parse Vegeta results (look for summary files)
        local summary_file=$(find "$(dirname "$results_file")" -name "*summary*" -o -name "*report*" | head -1)
        if [[ -f "$summary_file" ]]; then
            echo "tool=vegeta" > "$output_file"
            grep -E "(Success|Mean|95th|99th|Max)" "$summary_file" | sed 's/^/vegeta_/' >> "$output_file" 2>/dev/null || true
        fi
    fi
}

extract_goose_metrics() {
    local results_file=$1
    local output_file=$2
    
    # Look for goose report files
    local report_file=$(find "$(dirname "$results_file")" -name "goose-report*" | head -1)
    if [[ -f "$report_file" ]]; then
        echo "tool=goose" > "$output_file"
        # Extract metrics from goose report
        grep -E "(requests|users|response_time|error)" "$report_file" | head -10 | sed 's/^/goose_/' >> "$output_file" 2>/dev/null || true
    fi
}

extract_criterion_metrics() {
    local results_file=$1
    local output_file=$2
    
    # Look for criterion benchmark results
    local criterion_dir=$(find "$(dirname "$results_file")" -name "criterion" -type d | head -1)
    if [[ -d "$criterion_dir" ]]; then
        echo "tool=criterion" > "$output_file"
        # Find latest benchmark results
        find "$criterion_dir" -name "base" -type d | while read -r base_dir; do
            if [[ -f "$base_dir/estimates.json" && command -v jq >/dev/null 2>&1 ]]; then
                local benchmark_name=$(basename "$(dirname "$base_dir")")
                local mean=$(jq -r '.mean.point_estimate' "$base_dir/estimates.json" 2>/dev/null)
                echo "criterion_${benchmark_name}_mean=${mean}" >> "$output_file"
            fi
        done
    fi
}

extract_k6_metrics() {
    local results_file=$1
    local output_file=$2
    
    # Look for k6 summary JSON files
    local summary_file=$(find "$(dirname "$results_file")" -name "*summary*.json" | head -1)
    if [[ -f "$summary_file" && command -v jq >/dev/null 2>&1 ]]; then
        echo "tool=k6" > "$output_file"
        echo "k6_http_req_duration_avg=$(jq -r '.metrics.http_req_duration.avg // "N/A"' "$summary_file")" >> "$output_file"
        echo "k6_http_req_duration_p95=$(jq -r '.metrics.http_req_duration.p95 // "N/A"' "$summary_file")" >> "$output_file"
        echo "k6_http_req_duration_p99=$(jq -r '.metrics.http_req_duration.p99 // "N/A"' "$summary_file")" >> "$output_file"
        echo "k6_http_req_failed_rate=$(jq -r '.metrics.http_req_failed.rate // "N/A"' "$summary_file")" >> "$output_file"
        echo "k6_http_reqs_rate=$(jq -r '.metrics.http_reqs.rate // "N/A"' "$summary_file")" >> "$output_file"
    fi
}

extract_yandex_tank_metrics() {
    local results_file=$1
    local output_file=$2
    
    # Look for Yandex Tank report files
    local report_file=$(find "$(dirname "$results_file")" -name "*report*.md" -o -name "tank_*.log" | head -1)
    if [[ -f "$report_file" ]]; then
        echo "tool=yandex-tank" > "$output_file"
        # Extract basic metrics from tank logs/reports
        grep -E "(RPS|latency|errors)" "$report_file" | head -10 | sed 's/^/tank_/' >> "$output_file" 2>/dev/null || true
    fi
}

# Function to normalize metric values for comparison
normalize_metric_value() {
    local value=$1
    local unit=$2
    
    # Remove common suffixes and convert to base units
    case "$unit" in
        "ms"|"milliseconds")
            echo "$value" | sed 's/ms$//' | sed 's/[^0-9.]//g'
            ;;
        "s"|"seconds")
            # Convert seconds to milliseconds
            echo "$value" | sed 's/s$//' | sed 's/[^0-9.]//g' | awk '{print $1 * 1000}'
            ;;
        "rps"|"req/s")
            echo "$value" | sed 's/rps$//' | sed 's/req\/s$//' | sed 's/[^0-9.]//g'
            ;;
        "percent"|"%")
            echo "$value" | sed 's/%$//' | sed 's/[^0-9.]//g'
            ;;
        *)
            echo "$value" | sed 's/[^0-9.]//g'
            ;;
    esac
}

# Function to calculate percentage change
calculate_percentage_change() {
    local baseline=$1
    local current=$2
    
    if [[ -n "$baseline" && -n "$current" && "$baseline" != "0" && "$baseline" != "N/A" && "$current" != "N/A" ]]; then
        echo "scale=2; (($current - $baseline) / $baseline) * 100" | bc -l 2>/dev/null || echo "N/A"
    else
        echo "N/A"
    fi
}

# Function to detect regressions in extracted metrics
detect_regressions() {
    local baseline_metrics=$1
    local current_metrics=$2
    local tool_name=$3
    local output_file=$4
    
    echo "## $tool_name Regression Analysis" >> "$output_file"
    echo "" >> "$output_file"
    echo "| Metric | Baseline | Current | Change (%) | Status |" >> "$output_file"
    echo "|--------|----------|---------|------------|--------|" >> "$output_file"
    
    local regressions_found=false
    
    # Compare common metrics
    while IFS='=' read -r key value; do
        if [[ -n "$key" && "$key" != "tool" ]]; then
            local baseline_value=$(grep "^$key=" "$baseline_metrics" 2>/dev/null | cut -d'=' -f2)
            local current_value="$value"
            
            if [[ -n "$baseline_value" && "$baseline_value" != "N/A" && "$current_value" != "N/A" ]]; then
                # Normalize values for comparison
                local norm_baseline=$(normalize_metric_value "$baseline_value" "")
                local norm_current=$(normalize_metric_value "$current_value" "")
                
                local change=$(calculate_percentage_change "$norm_baseline" "$norm_current")
                
                if [[ "$change" != "N/A" ]]; then
                    local status="✅ OK"
                    local change_float=$(echo "$change" | sed 's/[^0-9.-]//g')
                    
                    # Determine if this is a regression based on metric type
                    case "$key" in
                        *"duration"*|*"latency"*|*"time"*)
                            # For latency metrics, increase is bad
                            if (( $(echo "$change_float > $REGRESSION_THRESHOLD" | bc -l) )); then
                                status="🔴 REGRESSION"
                                regressions_found=true
                            elif (( $(echo "$change_float > 5" | bc -l) )); then
                                status="⚠️ WARNING"
                            fi
                            ;;
                        *"rate"*|*"rps"*|*"throughput"*)
                            # For throughput metrics, decrease is bad
                            if (( $(echo "$change_float < -$REGRESSION_THRESHOLD" | bc -l) )); then
                                status="🔴 REGRESSION"
                                regressions_found=true
                            elif (( $(echo "$change_float < -5" | bc -l) )); then
                                status="⚠️ WARNING"
                            fi
                            ;;
                        *"error"*|*"failed"*)
                            # For error metrics, increase is bad
                            if (( $(echo "$change_float > 5" | bc -l) )); then
                                status="🔴 REGRESSION"
                                regressions_found=true
                            elif (( $(echo "$change_float > 2" | bc -l) )); then
                                status="⚠️ WARNING"
                            fi
                            ;;
                    esac
                    
                    echo "| $key | $baseline_value | $current_value | ${change}% | $status |" >> "$output_file"
                fi
            else
                echo "| $key | $baseline_value | $current_value | N/A | ➖ No comparison |" >> "$output_file"
            fi
        fi
    done < "$current_metrics"
    
    echo "" >> "$output_file"
    
    if [[ "$regressions_found" == "true" ]]; then
        echo "🔴 **REGRESSIONS DETECTED** in $tool_name" >> "$output_file"
        return 1
    else
        echo "✅ No significant regressions detected in $tool_name" >> "$output_file"
        return 0
    fi
}

# Function to create baseline from current results
create_baseline() {
    local results_dir=$1
    local baseline_dir=$2
    
    echo "📝 Creating performance baseline..."
    
    # Find the most recent orchestrated results
    local latest_results=$(find "$results_dir" -maxdepth 1 -type d -name "*_20*" | sort | tail -1)
    
    if [[ -z "$latest_results" ]]; then
        echo "❌ No recent results found in $results_dir"
        return 1
    fi
    
    echo "Using results from: $latest_results"
    
    # Create baseline directory with timestamp
    local baseline_timestamp=$(basename "$latest_results")
    local baseline_path="$baseline_dir/baseline_$baseline_timestamp"
    mkdir -p "$baseline_path"
    
    # Extract metrics from each tool's results
    for tool_dir in "$latest_results"/*; do
        if [[ -d "$tool_dir" ]]; then
            local tool_name=$(basename "$tool_dir" | sed 's/_[0-9]*$//')
            local metrics_file="$baseline_path/${tool_name}_metrics.txt"
            
            case "$tool_name" in
                "vegeta")
                    extract_vegeta_metrics "$tool_dir" "$metrics_file"
                    ;;
                "goose")
                    extract_goose_metrics "$tool_dir" "$metrics_file"
                    ;;
                "criterion")
                    extract_criterion_metrics "$tool_dir" "$metrics_file"
                    ;;
                "k6")
                    extract_k6_metrics "$tool_dir" "$metrics_file"
                    ;;
                "yandex-tank")
                    extract_yandex_tank_metrics "$tool_dir" "$metrics_file"
                    ;;
            esac
            
            if [[ -f "$metrics_file" ]]; then
                echo "✅ Extracted $tool_name baseline metrics"
            fi
        fi
    done
    
    # Create baseline metadata
    cat > "$baseline_path/baseline_info.txt" <<EOF
Baseline Created: $(date)
Source Results: $latest_results
Git Commit: $(git rev-parse HEAD 2>/dev/null || echo "N/A")
Git Branch: $(git branch --show-current 2>/dev/null || echo "N/A")
Threshold: ${REGRESSION_THRESHOLD}%
EOF
    
    # Update symlink to latest baseline
    ln -sfn "$baseline_path" "$baseline_dir/latest"
    
    echo "✅ Baseline created: $baseline_path"
    echo "📍 Latest baseline symlink updated"
}

# Function to run regression detection
run_regression_detection() {
    local current_results_dir=$1
    local baseline_dir=$2
    
    echo "🔍 Running regression detection..."
    
    # Check if baseline exists
    local latest_baseline="$baseline_dir/latest"
    if [[ ! -d "$latest_baseline" ]]; then
        echo "⚠️  No baseline found. Creating baseline from current results..."
        create_baseline "$current_results_dir" "$baseline_dir"
        echo "✅ Baseline created. Run regression detection again with new results."
        return 0
    fi
    
    echo "📊 Using baseline: $(readlink "$latest_baseline")"
    
    # Find the most recent results to compare
    local latest_results=$(find "$current_results_dir" -maxdepth 1 -type d -name "*_20*" | sort | tail -1)
    
    if [[ -z "$latest_results" ]]; then
        echo "❌ No recent results found for comparison"
        return 1
    fi
    
    echo "📈 Comparing results: $latest_results"
    
    # Create regression report
    local report_file="$REPORT_DIR/regression_report_$TIMESTAMP.md"
    
    cat > "$report_file" <<EOF
# Performance Regression Detection Report

**Generated:** $(date)
**Baseline:** $(readlink "$latest_baseline")
**Current Results:** $latest_results
**Regression Threshold:** ${REGRESSION_THRESHOLD}%

## Summary

This report compares current performance test results against the established baseline to detect performance regressions.

EOF
    
    local overall_regressions=false
    local tools_analyzed=0
    
    # Analyze each tool's results
    for current_tool_dir in "$latest_results"/*; do
        if [[ -d "$current_tool_dir" ]]; then
            local tool_name=$(basename "$current_tool_dir" | sed 's/_[0-9]*$//')
            local baseline_metrics="$latest_baseline/${tool_name}_metrics.txt"
            local current_metrics_file="/tmp/${tool_name}_current_metrics.txt"
            
            # Extract current metrics
            case "$tool_name" in
                "vegeta")
                    extract_vegeta_metrics "$current_tool_dir" "$current_metrics_file"
                    ;;
                "goose")
                    extract_goose_metrics "$current_tool_dir" "$current_metrics_file"
                    ;;
                "criterion")
                    extract_criterion_metrics "$current_tool_dir" "$current_metrics_file"
                    ;;
                "k6")
                    extract_k6_metrics "$current_tool_dir" "$current_metrics_file"
                    ;;
                "yandex-tank")
                    extract_yandex_tank_metrics "$current_tool_dir" "$current_metrics_file"
                    ;;
            esac
            
            # Compare with baseline if both exist
            if [[ -f "$baseline_metrics" && -f "$current_metrics_file" ]]; then
                if ! detect_regressions "$baseline_metrics" "$current_metrics_file" "$tool_name" "$report_file"; then
                    overall_regressions=true
                fi
                ((tools_analyzed++))
            else
                echo "## $tool_name" >> "$report_file"
                echo "⚠️ No baseline or current metrics available for comparison" >> "$report_file"
                echo "" >> "$report_file"
            fi
            
            # Cleanup temporary file
            rm -f "$current_metrics_file"
        fi
    done
    
    # Add summary to report
    cat >> "$report_file" <<EOF

## Overall Assessment

- **Tools Analyzed:** $tools_analyzed
- **Regression Threshold:** ${REGRESSION_THRESHOLD}%
- **Overall Status:** $(if [[ "$overall_regressions" == "true" ]]; then echo "🔴 REGRESSIONS DETECTED"; else echo "✅ NO SIGNIFICANT REGRESSIONS"; fi)

## Recommendations

EOF
    
    if [[ "$overall_regressions" == "true" ]]; then
        cat >> "$report_file" <<EOF
### Action Required

1. **Investigate Root Cause:** Review the regressed metrics and identify potential causes
2. **Check Recent Changes:** Review commits since the baseline was established
3. **Validate Results:** Re-run tests to confirm the regression is consistent
4. **Optimize Performance:** Address identified bottlenecks before deployment

EOF
    else
        cat >> "$report_file" <<EOF
### Performance Status Good

1. **No Action Required:** Performance is within acceptable thresholds
2. **Consider New Baseline:** If this represents expected performance after optimizations
3. **Continue Monitoring:** Keep running regression detection on new changes

EOF
    fi
    
    cat >> "$report_file" <<EOF

## Next Steps

1. Review detailed tool reports for additional insights
2. Monitor trends over multiple test runs
3. Update baseline after confirmed performance improvements
4. Set up automated regression detection in CI/CD pipeline

EOF
    
    echo "✅ Regression report generated: $report_file"
    
    if [[ "$overall_regressions" == "true" ]]; then
        echo "🔴 PERFORMANCE REGRESSIONS DETECTED!"
        echo "📋 Review report: $report_file"
        return 1
    else
        echo "✅ No significant performance regressions detected"
        return 0
    fi
}

# Main execution
main() {
    case "${1:-detect}" in
        "baseline"|"create-baseline")
            create_baseline "$RESULTS_DIR" "$BASELINE_DIR"
            ;;
        "detect"|"regression")
            run_regression_detection "$RESULTS_DIR" "$BASELINE_DIR"
            ;;
        "help"|"--help")
            echo "Usage: $0 [COMMAND]"
            echo ""
            echo "Commands:"
            echo "  baseline    Create performance baseline from latest results"
            echo "  detect      Run regression detection (default)"
            echo "  help        Show this help message"
            echo ""
            echo "Environment Variables:"
            echo "  BASELINE_DIR           Directory for baseline storage (default: ./baselines)"
            echo "  RESULTS_DIR            Directory with test results (default: ./orchestrated_results)"
            echo "  REGRESSION_THRESHOLD   Percentage threshold for regressions (default: 10)"
            echo ""
            echo "Examples:"
            echo "  $0 baseline                          # Create new baseline"
            echo "  $0 detect                            # Run regression detection"
            echo "  REGRESSION_THRESHOLD=5 $0 detect     # Stricter threshold"
            ;;
        *)
            echo "❌ Unknown command: $1"
            echo "Use '$0 help' for usage information"
            exit 1
            ;;
    esac
}

# Check dependencies
if ! command -v bc >/dev/null 2>&1; then
    echo "⚠️  Warning: 'bc' calculator not found. Install for better calculations."
fi

# Run main function
main "$@"