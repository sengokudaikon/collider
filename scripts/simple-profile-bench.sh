#!/bin/bash

# Simple profiling + benchmarking using cargo-flamegraph
set -e

echo "🔥 Simple Flamegraph Profiling + Benchmarks"
echo "============================================"

# Check if cargo-flamegraph is installed
if ! command -v cargo-flamegraph &> /dev/null; then
    echo "📦 Installing cargo-flamegraph..."
    cargo install flamegraph
fi

# Ensure dev environment is running (for database/cache)
echo "🐳 Ensuring dev environment is running..."
just dev-up >/dev/null 2>&1

# Create results directory
RESULTS_DIR="profiling_results/simple_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RESULTS_DIR"

echo "📊 Starting app with flamegraph profiling..."

# Start the app with flamegraph - it will run, get profiled, then exit
cd server

# Use cargo-flamegraph with a timeout to capture the benchmark period
timeout 60s cargo flamegraph --bin server -- &
APP_PID=$!

# Give the app time to start
echo "⏳ Waiting for app to start..."
sleep 10

# Check if app is ready
until curl -f http://localhost:8080/health &>/dev/null; do
    echo "Waiting for app to be ready..."
    sleep 2
done
echo "✅ App is ready!"

# Run benchmarks while app is being profiled
echo "🚀 Running benchmarks..."
cd ..
{
    echo "=== Benchmark Start: $(date) ==="
    cargo bench --package collider-benchmarks 2>&1
    echo "=== Benchmark End: $(date) ==="
} | tee "$RESULTS_DIR/benchmark_output.log"

# The flamegraph process should complete automatically
wait $APP_PID 2>/dev/null || true

# Move flamegraph results
if [[ -f server/flamegraph.svg ]]; then
    mv server/flamegraph.svg "$RESULTS_DIR/"
    echo "🔥 Flamegraph saved to: $RESULTS_DIR/flamegraph.svg"
else
    echo "⚠️  Flamegraph not generated"
fi

# Create summary
cat > "$RESULTS_DIR/README.md" <<EOF
# Simple Profiling Results

**Date:** $(date)
**Method:** cargo-flamegraph

## Files

- \`flamegraph.svg\` - Interactive flamegraph (open in browser)
- \`benchmark_output.log\` - Benchmark execution logs

## How to View

1. **Flamegraph:** Open \`flamegraph.svg\` in a web browser
2. **Benchmarks:** View \`benchmark_output.log\` for performance metrics

## Flamegraph Usage

- **Hover** over sections to see function names and percentages
- **Click** on sections to zoom in
- **Use browser back** to zoom out
- **Red/Orange areas** indicate CPU hotspots

EOF

echo ""
echo "✅ Simple profiling complete!"
echo "📁 Results: $RESULTS_DIR"
echo "🔥 Open flamegraph: open $RESULTS_DIR/flamegraph.svg"
echo "📋 View benchmarks: cat $RESULTS_DIR/benchmark_output.log"