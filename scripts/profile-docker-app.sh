#!/bin/bash

# Profile dockerized app with flamegraph while running benchmarks
set -e

CONTAINER_NAME="collider_app_dev"
RESULTS_DIR="profiling_results/$(date +%Y%m%d_%H%M%S)"

echo "🔥 Profiling Docker App + Benchmarks"
echo "====================================="

# Create results directory
mkdir -p "$RESULTS_DIR"

# Check if container is running
if ! docker ps | grep -q "$CONTAINER_NAME"; then
    echo "❌ Container $CONTAINER_NAME is not running"
    echo "Run 'just dev-up' first"
    exit 1
fi

echo "✅ Container $CONTAINER_NAME is running"

# Install profiling tools in container if needed
echo "📦 Installing profiling tools in container..."
docker exec "$CONTAINER_NAME" sh -c "
    if ! command -v perf &> /dev/null; then
        apt-get update -qq
        apt-get install -y linux-perf-$(uname -r) || apt-get install -y linux-tools-generic || echo 'Perf not available'
    fi
" 2>/dev/null || echo "⚠️  Perf installation failed (may not be available in container)"

# Get container PID for the Rust process
echo "🔍 Finding Rust server process in container..."
RUST_PID=$(docker exec "$CONTAINER_NAME" pgrep -f "server" | head -1)
if [[ -z "$RUST_PID" ]]; then
    echo "❌ Could not find Rust server process in container"
    exit 1
fi
echo "✅ Found Rust server process: PID $RUST_PID"

# Start container monitoring
echo "📊 Starting container monitoring..."
{
    echo "Timestamp,CPU%,Memory Usage,Memory %,Net I/O,Block I/O"
    while true; do
        docker stats "$CONTAINER_NAME" --no-stream --format "$(date '+%Y-%m-%d %H:%M:%S'),{{.CPUPerc}},{{.MemUsage}},{{.MemPerc}},{{.NetIO}},{{.BlockIO}}"
        sleep 1
    done
} > "$RESULTS_DIR/container_stats.csv" &
STATS_PID=$!

# Start perf profiling in container
echo "🔥 Starting perf profiling in container..."
docker exec -d "$CONTAINER_NAME" sh -c "
    perf record -g -F 99 -p $RUST_PID -o /tmp/perf.data &
    echo \$! > /tmp/perf.pid
" 2>/dev/null || echo "⚠️  Perf profiling may not be available"

# Give profiling a moment to start
sleep 2

# Run benchmarks
echo "🚀 Running benchmarks while profiling..."
{
    echo "=== Benchmark Start Time: $(date) ==="
    echo ""
    
    echo "=== Criterion Benchmarks ==="
    cargo bench --package collider-benchmarks 2>&1 || echo "Criterion benchmarks failed"
    echo ""
    
    echo "=== Quick K6 Load Test ==="
    docker run --rm --network collider \
        -v "$(pwd)/infrastructure/benchmarking/k6:/scripts" \
        grafana/k6:latest run --duration 30s --vus 50 /scripts/load-test.js 2>&1 || echo "K6 load test failed"
    echo ""
    
    echo "=== Benchmark End Time: $(date) ==="
} | tee "$RESULTS_DIR/benchmark_output.log"

# Stop profiling
echo "🛑 Stopping profiling..."

# Stop perf in container
docker exec "$CONTAINER_NAME" sh -c "
    if [[ -f /tmp/perf.pid ]]; then
        kill \$(cat /tmp/perf.pid) 2>/dev/null || true
        rm -f /tmp/perf.pid
    fi
" 2>/dev/null || true

# Copy perf data from container
echo "📋 Extracting profiling data..."
docker cp "$CONTAINER_NAME:/tmp/perf.data" "$RESULTS_DIR/" 2>/dev/null || echo "⚠️  Could not extract perf data"

# Generate flamegraph if possible
if [[ -f "$RESULTS_DIR/perf.data" ]]; then
    echo "🔥 Generating flamegraph..."
    cd "$RESULTS_DIR"
    
    # Try to generate flamegraph
    if command -v flamegraph >/dev/null 2>&1; then
        perf script -i perf.data | flamegraph > flamegraph.svg 2>/dev/null || echo "⚠️  Flamegraph generation failed"
    elif command -v perf >/dev/null 2>&1; then
        perf report -i perf.data --stdio > perf_report.txt 2>/dev/null || echo "⚠️  Perf report generation failed"
    fi
    cd - >/dev/null
fi

# Stop container monitoring
kill $STATS_PID 2>/dev/null || true

# Generate summary report
echo "📋 Generating profiling summary..."
cat > "$RESULTS_DIR/profiling_summary.md" <<EOF
# Profiling Summary

**Date:** $(date)
**Container:** $CONTAINER_NAME
**Rust Process PID:** $RUST_PID

## Files Generated

- \`container_stats.csv\` - Container resource usage over time
- \`benchmark_output.log\` - Benchmark execution logs
- \`perf.data\` - Raw perf profiling data (if available)
- \`flamegraph.svg\` - Flamegraph visualization (if generated)
- \`perf_report.txt\` - Perf text report (if generated)

## Container Stats Summary

EOF

# Add stats summary if available
if [[ -f "$RESULTS_DIR/container_stats.csv" ]]; then
    echo "### Resource Usage During Benchmarks" >> "$RESULTS_DIR/profiling_summary.md"
    echo "\`\`\`" >> "$RESULTS_DIR/profiling_summary.md"
    echo "First 5 lines:" >> "$RESULTS_DIR/profiling_summary.md"
    head -6 "$RESULTS_DIR/container_stats.csv" >> "$RESULTS_DIR/profiling_summary.md"
    echo "..." >> "$RESULTS_DIR/profiling_summary.md"
    echo "Last 5 lines:" >> "$RESULTS_DIR/profiling_summary.md"
    tail -5 "$RESULTS_DIR/container_stats.csv" >> "$RESULTS_DIR/profiling_summary.md"
    echo "\`\`\`" >> "$RESULTS_DIR/profiling_summary.md"
fi

echo ""
echo "✅ Profiling complete!"
echo "📁 Results saved to: $RESULTS_DIR"
echo ""
echo "📊 View results:"
echo "   cat $RESULTS_DIR/profiling_summary.md"
echo "   open $RESULTS_DIR/flamegraph.svg  # (if generated)"
echo "   cat $RESULTS_DIR/benchmark_output.log"

# Show quick summary
if [[ -f "$RESULTS_DIR/container_stats.csv" ]]; then
    echo ""
    echo "📈 Quick Stats Summary:"
    echo "   Lines in stats file: $(wc -l < "$RESULTS_DIR/container_stats.csv")"
    echo "   Profiling duration: ~$(($(wc -l < "$RESULTS_DIR/container_stats.csv") - 1)) seconds"
fi

echo ""
echo "💡 Next steps:"
echo "   - Analyze flamegraph.svg for hotspots"
echo "   - Review container_stats.csv for resource usage patterns"
echo "   - Compare benchmark results with baseline"