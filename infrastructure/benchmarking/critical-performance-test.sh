#!/bin/bash

# Critical Performance Testing for Collider
# Tests server performance under extreme conditions:
# - Database with millions of events
# - 100k+ RPS sustained load
# - Resource exhaustion scenarios

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_URL=${1:-"http://localhost:8080"}
MAX_RPS=${2:-100000}
TEST_DURATION=${3:-600}  # 10 minutes
DB_SCALE=${4:-5000000}   # 5 million events
RESULTS_DIR="critical_performance_results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

echo "🔥 CRITICAL PERFORMANCE TESTING"
echo "==============================="
echo "Target: $TARGET_URL"
echo "Maximum RPS: $MAX_RPS"
echo "Test Duration: ${TEST_DURATION}s"
echo "Database Scale: $DB_SCALE events"
echo "Results: $RESULTS_DIR"
echo ""
echo "⚠️  WARNING: This test will generate EXTREME load!"
echo "   - Ensure adequate system resources"
echo "   - Monitor system health during test"
echo "   - Have recovery procedures ready"
echo ""

# Create results directory
mkdir -p "$RESULTS_DIR"

# Configuration for critical testing
CRITICAL_TEST_CONFIG=(
    "warmup:1000:60:Database warmup phase"
    "ramp_light:10000:120:Light load ramp-up"
    "ramp_medium:25000:180:Medium load ramp-up" 
    "ramp_heavy:50000:240:Heavy load ramp-up"
    "ramp_extreme:75000:300:Extreme load ramp-up"
    "critical_sustained:${MAX_RPS}:${TEST_DURATION}:Critical sustained load"
    "spike_test:$((MAX_RPS * 2)):30:Extreme spike test"
    "recovery:5000:120:Recovery verification"
)

# Pre-flight checks
preflight_checks() {
    echo "🔍 Running pre-flight checks..."
    
    # Check server availability
    if ! curl -f "$TARGET_URL/health" &>/dev/null; then
        echo "❌ Server health check failed: $TARGET_URL"
        exit 1
    fi
    
    # Check system resources
    echo "📊 System Resources:"
    if command -v free >/dev/null 2>&1; then
        free -h | head -2
    fi
    
    if command -v nproc >/dev/null 2>&1; then
        echo "CPU Cores: $(nproc)"
    fi
    
    # Check database state
    echo "🔍 Checking database state..."
    local event_count_response=$(curl -s "$TARGET_URL/api/events/count" || echo "0")
    local current_events=$(echo "$event_count_response" | grep -o '[0-9]*' | head -1 || echo "0")
    
    echo "Current events in database: $current_events"
    
    if [[ $current_events -lt $((DB_SCALE / 2)) ]]; then
        echo "⚠️  Database has insufficient data for critical testing"
        echo "   Current: $current_events events"
        echo "   Recommended: $DB_SCALE+ events"
        echo ""
        echo "Run database seeding first:"
        echo "   just test-setup-db  # For test environment"
        echo "   # Or seed production-scale data"
        echo ""
        read -p "Continue anyway? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi
    
    # Check available tools
    local missing_tools=()
    command -v vegeta >/dev/null 2>&1 || missing_tools+=("vegeta")
    command -v k6 >/dev/null 2>&1 || missing_tools+=("k6")
    
    if [[ ${#missing_tools[@]} -gt 0 ]]; then
        echo "❌ Missing required tools: ${missing_tools[*]}"
        echo "Install with: brew install ${missing_tools[*]}"
        exit 1
    fi
    
    echo "✅ Pre-flight checks passed"
}

# Monitor system resources during test
start_system_monitoring() {
    echo "📊 Starting system monitoring..."
    
    # Create monitoring script
    cat > "$RESULTS_DIR/monitor_system.sh" <<'EOF'
#!/bin/bash
RESULTS_DIR="$1"
INTERVAL=5

echo "timestamp,cpu_usage,memory_usage,load_avg,disk_io,network_rx,network_tx" > "$RESULTS_DIR/system_metrics.csv"

while true; do
    timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    
    # CPU usage
    cpu_usage=$(top -bn1 | grep "Cpu(s)" | awk '{print $2}' | sed 's/%us,//' || echo "0")
    
    # Memory usage
    if command -v free >/dev/null 2>&1; then
        memory_usage=$(free | grep Mem | awk '{printf "%.1f", $3/$2 * 100.0}')
    else
        memory_usage="0"
    fi
    
    # Load average
    load_avg=$(uptime | awk '{print $(NF-2)}' | sed 's/,//')
    
    # Disk I/O (simplified)
    disk_io=$(iostat -d 1 1 2>/dev/null | tail -1 | awk '{print $4}' || echo "0")
    
    # Network (simplified - requires monitoring interface)
    network_rx="0"
    network_tx="0"
    
    echo "$timestamp,$cpu_usage,$memory_usage,$load_avg,$disk_io,$network_rx,$network_tx" >> "$RESULTS_DIR/system_metrics.csv"
    
    sleep $INTERVAL
done
EOF
    
    chmod +x "$RESULTS_DIR/monitor_system.sh"
    "$RESULTS_DIR/monitor_system.sh" "$RESULTS_DIR" &
    MONITOR_PID=$!
    
    echo "✅ System monitoring started (PID: $MONITOR_PID)"
}

# Stop system monitoring
stop_system_monitoring() {
    if [[ -n "$MONITOR_PID" ]]; then
        kill $MONITOR_PID 2>/dev/null || true
        echo "✅ System monitoring stopped"
    fi
}

# Run critical load test phase
run_critical_phase() {
    local phase_name=$1
    local target_rps=$2
    local duration=$3
    local description=$4
    
    echo ""
    echo "🚀 Phase: $phase_name"
    echo "Target RPS: $target_rps"
    echo "Duration: ${duration}s"
    echo "Description: $description"
    echo "----------------------------------------"
    
    local phase_start=$(date +%s)
    local phase_results_dir="$RESULTS_DIR/${phase_name}_${TIMESTAMP}"
    mkdir -p "$phase_results_dir"
    
    # Create realistic payload for this phase
    cat > "$phase_results_dir/critical_payload.json" <<EOF
{
  "user_id": "550e8400-e29b-41d4-a716-$(date +%s | tail -c 13)",
  "event_type": "critical_load_test",
  "timestamp": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
  "metadata": {
    "session_id": "critical_session_$(date +%s)",
    "action": "critical_action",
    "element": "critical_element",
    "page": "/critical/test",
    "test_phase": "$phase_name",
    "target_rps": $target_rps,
    "browser": "CriticalTestBot",
    "version": "1.0.0",
    "platform": "LoadTest",
    "screen_resolution": "1920x1080",
    "critical_test": true,
    "phase_start": $phase_start,
    "expected_duration": $duration,
    "performance_data": {
      "load_time": 0,
      "dom_ready": 0,
      "first_paint": 0,
      "phase_identifier": "$phase_name"
    }
  }
}
EOF

    # Create Vegeta targets file
    cat > "$phase_results_dir/targets.txt" <<EOF
POST $TARGET_URL/api/events
Content-Type: application/json
@$phase_results_dir/critical_payload.json

GET $TARGET_URL/health

GET $TARGET_URL/api/events?limit=10

EOF

    # Run load test with error handling
    echo "⚡ Starting $phase_name load test..."
    
    local vegeta_output="$phase_results_dir/vegeta_${phase_name}.bin"
    local vegeta_report="$phase_results_dir/vegeta_${phase_name}_report.txt"
    local vegeta_json="$phase_results_dir/vegeta_${phase_name}_report.json"
    
    # Run Vegeta attack
    if timeout $((duration + 60)) vegeta attack \
        -targets="$phase_results_dir/targets.txt" \
        -rate="$target_rps" \
        -duration="${duration}s" \
        -workers=100 \
        -max-workers=500 \
        -connections=1000 \
        -timeout=30s \
        -keepalive=true > "$vegeta_output" 2>"$phase_results_dir/vegeta_errors.log"; then
        
        echo "✅ Vegeta attack completed"
        
        # Generate reports
        vegeta report < "$vegeta_output" > "$vegeta_report"
        vegeta report -type=json < "$vegeta_output" > "$vegeta_json"
        
        # Show quick summary
        echo "📊 Quick Results:"
        head -n 15 "$vegeta_report" | tail -n 10
        
    else
        echo "⚠️  Vegeta attack encountered issues (timeout or error)"
        echo "Check error log: $phase_results_dir/vegeta_errors.log"
    fi
    
    # Run concurrent k6 test for additional metrics
    echo "🔄 Running concurrent k6 metrics collection..."
    
    cat > "$phase_results_dir/k6_critical.js" <<EOF
import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';

const errorRate = new Rate('critical_errors');
const responseTime = new Trend('critical_response_time', true);
const throughput = new Counter('critical_requests');

export let options = {
  stages: [
    { duration: '${duration}s', target: 50 },  // Light monitoring load
  ],
  thresholds: {
    critical_errors: ['rate<0.1'],
    critical_response_time: ['p(95)<2000'],
  },
};

export default function () {
  const response = http.get('$TARGET_URL/health');
  
  const success = check(response, {
    'status is 200': (r) => r.status === 200,
    'response time < 5s': (r) => r.timings.duration < 5000,
  });
  
  errorRate.add(!success);
  responseTime.add(response.timings.duration);
  throughput.add(1);
  
  sleep(1);
}
EOF

    k6 run "$phase_results_dir/k6_critical.js" \
        --out json="$phase_results_dir/k6_critical_metrics.json" \
        > "$phase_results_dir/k6_critical_output.log" 2>&1 &
    
    local k6_pid=$!
    
    # Wait for phase to complete
    local phase_end=$(date +%s)
    local actual_duration=$((phase_end - phase_start))
    
    # Stop k6 monitoring
    kill $k6_pid 2>/dev/null || true
    wait $k6_pid 2>/dev/null || true
    
    # Health check after phase
    echo "🔍 Post-phase health check..."
    if curl -f "$TARGET_URL/health" &>/dev/null; then
        echo "✅ Server still healthy after $phase_name"
    else
        echo "❌ Server health check failed after $phase_name"
        echo "⚠️  CRITICAL: Server may be in degraded state"
    fi
    
    # Store phase summary
    cat > "$phase_results_dir/phase_summary.txt" <<EOF
Phase: $phase_name
Target RPS: $target_rps
Planned Duration: ${duration}s
Actual Duration: ${actual_duration}s
Start Time: $(date -d "@$phase_start" 2>/dev/null || date -r $phase_start)
End Time: $(date -d "@$phase_end" 2>/dev/null || date -r $phase_end)
Description: $description
Status: $(if [[ $actual_duration -ge $((duration - 10)) ]]; then echo "COMPLETED"; else echo "TERMINATED_EARLY"; fi)
EOF

    echo "✅ Phase $phase_name completed in ${actual_duration}s"
    
    # Brief cooldown between phases
    if [[ "$phase_name" != "recovery" ]]; then
        echo "😴 Cooldown period (30s)..."
        sleep 30
    fi
}

# Generate comprehensive critical performance report
generate_critical_report() {
    echo "📋 Generating critical performance report..."
    
    local report_file="$RESULTS_DIR/critical_performance_report_$TIMESTAMP.md"
    
    cat > "$report_file" <<EOF
# CRITICAL PERFORMANCE TEST REPORT

**Test Date:** $(date)
**Target URL:** $TARGET_URL
**Maximum RPS Target:** $MAX_RPS
**Test Duration:** ${TEST_DURATION}s
**Database Scale:** $DB_SCALE events
**Test ID:** $TIMESTAMP

## ⚠️ CRITICAL TEST WARNING

This test pushed the system to its absolute limits:
- Sustained load up to $MAX_RPS RPS
- Database with millions of events
- Extended duration testing
- Resource exhaustion scenarios

## Executive Summary

EOF

    # Analyze results and add summary
    local total_phases=0
    local successful_phases=0
    local max_achieved_rps=0
    
    for phase_dir in "$RESULTS_DIR"/*_"$TIMESTAMP"; do
        if [[ -d "$phase_dir" && -f "$phase_dir/phase_summary.txt" ]]; then
            ((total_phases++))
            
            local status=$(grep "Status:" "$phase_dir/phase_summary.txt" | cut -d: -f2 | tr -d ' ')
            if [[ "$status" == "COMPLETED" ]]; then
                ((successful_phases++))
            fi
            
            # Extract achieved RPS from Vegeta report
            local vegeta_report="$phase_dir/vegeta_"*"_report.txt"
            if [[ -f $vegeta_report ]]; then
                local achieved_rps=$(grep "Rate" "$vegeta_report" | awk '{print $2}' | head -1 || echo "0")
                achieved_rps=${achieved_rps%.*}  # Remove decimal
                if (( achieved_rps > max_achieved_rps )); then
                    max_achieved_rps=$achieved_rps
                fi
            fi
        fi
    done
    
    cat >> "$report_file" <<EOF
- **Total Test Phases:** $total_phases
- **Successful Phases:** $successful_phases
- **Success Rate:** $(( successful_phases * 100 / total_phases ))%
- **Maximum Achieved RPS:** $max_achieved_rps
- **Target Achievement:** $(( max_achieved_rps * 100 / MAX_RPS ))%

## Test Phases

| Phase | Target RPS | Duration | Status | Max Latency | Error Rate |
|-------|------------|----------|---------|-------------|------------|
EOF

    # Add phase details
    for phase_dir in "$RESULTS_DIR"/*_"$TIMESTAMP"; do
        if [[ -d "$phase_dir" && -f "$phase_dir/phase_summary.txt" ]]; then
            local phase_name=$(basename "$phase_dir" | sed "s/_${TIMESTAMP}//")
            local target_rps=$(grep "Target RPS:" "$phase_dir/phase_summary.txt" | cut -d: -f2 | tr -d ' ')
            local duration=$(grep "Actual Duration:" "$phase_dir/phase_summary.txt" | cut -d: -f2 | tr -d ' ')
            local status=$(grep "Status:" "$phase_dir/phase_summary.txt" | cut -d: -f2 | tr -d ' ')
            
            # Extract metrics from Vegeta report
            local vegeta_report="$phase_dir/vegeta_"*"_report.txt"
            local max_latency="N/A"
            local error_rate="N/A"
            
            if [[ -f $vegeta_report ]]; then
                max_latency=$(grep "Max" "$vegeta_report" | awk '{print $2}' || echo "N/A")
                error_rate=$(grep "Success" "$vegeta_report" | awk '{print (100-$2)"%"}' || echo "N/A")
            fi
            
            local status_icon="✅"
            if [[ "$status" != "COMPLETED" ]]; then
                status_icon="❌"
            fi
            
            echo "| $phase_name | $target_rps | $duration | $status_icon $status | $max_latency | $error_rate |" >> "$report_file"
        fi
    done
    
    cat >> "$report_file" <<EOF

## System Performance Analysis

### Resource Utilization
EOF

    # Add system metrics analysis if available
    if [[ -f "$RESULTS_DIR/system_metrics.csv" ]]; then
        echo "System metrics collected during test. Key observations:" >> "$report_file"
        echo "" >> "$report_file"
        echo "\`\`\`" >> "$report_file"
        echo "Peak CPU Usage: $(tail -n +2 "$RESULTS_DIR/system_metrics.csv" | cut -d, -f2 | sort -n | tail -1)%" >> "$report_file"
        echo "Peak Memory Usage: $(tail -n +2 "$RESULTS_DIR/system_metrics.csv" | cut -d, -f3 | sort -n | tail -1)%" >> "$report_file"
        echo "Peak Load Average: $(tail -n +2 "$RESULTS_DIR/system_metrics.csv" | cut -d, -f4 | sort -n | tail -1)" >> "$report_file"
        echo "\`\`\`" >> "$report_file"
    else
        echo "System metrics not available." >> "$report_file"
    fi
    
    cat >> "$report_file" <<EOF

## Critical Findings

### Performance Limits
- **Maximum Sustained RPS:** $max_achieved_rps
- **Breaking Point:** $(if (( max_achieved_rps >= MAX_RPS )); then echo "Not reached - system handled target load"; else echo "Reached at ~$max_achieved_rps RPS"; fi)
- **Recovery Capability:** $(if (( successful_phases == total_phases )); then echo "Excellent - all phases completed"; else echo "Needs investigation - some phases failed"; fi)

### Bottleneck Analysis
EOF

    # Analyze bottlenecks based on failure patterns
    if (( max_achieved_rps < MAX_RPS )); then
        cat >> "$report_file" <<EOF
1. **Throughput Limitation**: System could not sustain $MAX_RPS RPS
2. **Possible Causes**:
   - Database connection limits
   - CPU/Memory exhaustion
   - Network bandwidth limits
   - Application-level bottlenecks

EOF
    else
        cat >> "$report_file" <<EOF
1. **Excellent Performance**: System successfully sustained target load
2. **Potential for Higher Load**: Consider testing even higher RPS
3. **Stable Under Pressure**: No significant degradation observed

EOF
    fi
    
    cat >> "$report_file" <<EOF

## Recommendations

### Immediate Actions
EOF

    if (( successful_phases < total_phases )); then
        cat >> "$report_file" <<EOF
- 🔴 **Critical**: Investigate failed test phases
- 🔍 **Debug**: Review error logs for failure patterns
- 🛠️ **Fix**: Address bottlenecks before production deployment
EOF
    else
        cat >> "$report_file" <<EOF
- ✅ **Performance Validated**: System meets critical performance requirements
- 📊 **Establish Baselines**: Use these results as performance baselines
- 🔄 **Regular Testing**: Schedule regular critical performance validation
EOF
    fi
    
    cat >> "$report_file" <<EOF

### Long-term Optimizations
1. **Database Optimization**:
   - Review query performance with millions of records
   - Consider database sharding or partitioning
   - Optimize indexes for high-volume operations

2. **Application Scaling**:
   - Implement horizontal scaling
   - Review connection pooling configuration
   - Consider caching strategies for read operations

3. **Infrastructure Scaling**:
   - Monitor resource utilization patterns
   - Plan capacity for peak load scenarios
   - Implement auto-scaling policies

## Detailed Results

### Files Generated
EOF

    # List all generated files
    find "$RESULTS_DIR" -name "*$TIMESTAMP*" -type f | while read -r file; do
        echo "- \`$(basename "$file")\`: $(basename "$(dirname "$file")")" >> "$report_file"
    done
    
    cat >> "$report_file" <<EOF

### Analysis Commands
\`\`\`bash
# View system metrics
cat $RESULTS_DIR/system_metrics.csv

# Review individual phase reports
ls $RESULTS_DIR/*_$TIMESTAMP/

# Check error patterns
grep -r "error\|Error\|ERROR" $RESULTS_DIR/*_$TIMESTAMP/
\`\`\`

## Next Steps

1. **Review Detailed Logs**: Examine each phase for specific insights
2. **Correlate with Application Logs**: Check server logs during test periods
3. **Database Analysis**: Review database performance during high load
4. **Infrastructure Review**: Assess system resource utilization
5. **Optimization Planning**: Create action plan based on bottlenecks found

---

**Test completed at:** $(date)
**Total test duration:** $(( $(date +%s) - $(date -d "@$phase_start" +%s 2>/dev/null || echo "0") )) seconds
**Results location:** $RESULTS_DIR

EOF

    echo "✅ Critical performance report generated: $report_file"
}

# Cleanup function
cleanup() {
    echo ""
    echo "🧹 Cleaning up critical performance test..."
    
    # Stop system monitoring
    stop_system_monitoring
    
    # Remove temporary files
    rm -f "$RESULTS_DIR"/*/critical_payload.json 2>/dev/null || true
    rm -f "$RESULTS_DIR"/*/targets.txt 2>/dev/null || true
    rm -f "$RESULTS_DIR"/*/k6_critical.js 2>/dev/null || true
    
    echo "✅ Cleanup completed"
}

# Main execution
main() {
    echo "🎯 Starting CRITICAL performance testing..."
    echo ""
    
    # Confirm before starting
    echo "⚠️  FINAL WARNING: This test will generate extreme load!"
    echo "   Target: $TARGET_URL"
    echo "   Max RPS: $MAX_RPS"
    echo "   Duration: ${TEST_DURATION}s per phase"
    echo ""
    read -p "Are you sure you want to proceed? (yes/NO): " -r
    if [[ ! "$REPLY" == "yes" ]]; then
        echo "❌ Critical performance test cancelled"
        exit 0
    fi
    
    local test_start=$(date +%s)
    
    preflight_checks
    start_system_monitoring
    
    echo ""
    echo "🚀 Starting critical performance test phases..."
    
    # Run all critical test phases
    for config in "${CRITICAL_TEST_CONFIG[@]}"; do
        IFS=':' read -r phase_name target_rps duration description <<< "$config"
        run_critical_phase "$phase_name" "$target_rps" "$duration" "$description"
    done
    
    local test_end=$(date +%s)
    local total_duration=$((test_end - test_start))
    
    generate_critical_report
    
    echo ""
    echo "🎉 CRITICAL PERFORMANCE TESTING COMPLETED!"
    echo "================================================="
    echo "📊 Total Duration: ${total_duration}s"
    echo "📁 Results: $RESULTS_DIR"
    echo "📋 Report: $RESULTS_DIR/critical_performance_report_$TIMESTAMP.md"
    echo ""
    echo "⚠️  IMPORTANT:"
    echo "   1. Review the comprehensive report immediately"
    echo "   2. Check server health and application logs"
    echo "   3. Monitor system recovery"
    echo "   4. Address any performance issues found"
    echo ""
    echo "💡 Next steps:"
    echo "   - Analyze bottlenecks identified"
    echo "   - Plan performance optimizations"
    echo "   - Establish performance SLAs based on results"
    echo "   - Schedule regular critical performance validation"
}

# Handle script interruption
trap cleanup EXIT

# Show usage
if [[ "$1" == "--help" || "$1" == "-h" ]]; then
    echo "Usage: $0 [TARGET_URL] [MAX_RPS] [DURATION] [DB_SCALE]"
    echo ""
    echo "Arguments:"
    echo "  TARGET_URL    Target URL (default: http://localhost:8080)"
    echo "  MAX_RPS       Maximum RPS target (default: 100000)"
    echo "  DURATION      Sustained test duration in seconds (default: 600)"
    echo "  DB_SCALE      Expected database scale in events (default: 5000000)"
    echo ""
    echo "Examples:"
    echo "  $0                                          # Default critical test"
    echo "  $0 http://localhost:8080 50000             # 50k RPS target"
    echo "  $0 http://staging.example.com 75000 300    # 75k RPS for 5 minutes"
    echo ""
    echo "⚠️  WARNING: This test generates EXTREME load!"
    echo "   - Ensure adequate system resources"
    echo "   - Monitor system health during execution"
    echo "   - Have recovery procedures ready"
    exit 0
fi

# Run main function
main "$@"