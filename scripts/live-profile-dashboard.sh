#!/bin/bash

# Live profiling dashboard with real-time metrics
set -e

echo "📊 Live Profiling Dashboard"
echo "=========================="

# Check dependencies
if ! command -v cargo-flamegraph &> /dev/null; then
    echo "📦 Installing cargo-flamegraph..."
    cargo install flamegraph
fi

# Start dev environment
echo "🐳 Starting development environment..."
just dev-up >/dev/null 2>&1

# Create dashboard directory
DASHBOARD_DIR="live_dashboard_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$DASHBOARD_DIR"

echo "🚀 Starting live profiling dashboard..."
echo "📁 Results will be saved to: $DASHBOARD_DIR"

# Start Prometheus and Grafana if not running
echo "📈 Ensuring monitoring stack is running..."
docker-compose up -d prometheus grafana >/dev/null 2>&1

# Function to generate live metrics
generate_live_metrics() {
    while true; do
        {
            echo "=== $(date) ==="
            echo "Container Stats:"
            docker stats collider_app_dev --no-stream --format "  CPU: {{.CPUPerc}} | Memory: {{.MemUsage}} | Net: {{.NetIO}}"
            echo ""
            
            echo "Application Health:"
            if curl -s http://localhost:8080/health >/dev/null; then
                echo "  ✅ App responding"
                
                # Get response time
                RESPONSE_TIME=$(curl -o /dev/null -s -w '%{time_total}' http://localhost:8080/health)
                echo "  ⏱️  Health check: ${RESPONSE_TIME}s"
            else
                echo "  ❌ App not responding"
            fi
            echo ""
            
            echo "System Load:"
            uptime | sed 's/.*load averages: /  Load: /'
            echo ""
            
            echo "=================================="
            echo ""
        } | tee -a "$DASHBOARD_DIR/live_metrics.log"
        
        sleep 5
    done
}

# Start live metrics in background
generate_live_metrics &
METRICS_PID=$!

# Function to run continuous benchmarks
run_continuous_benchmarks() {
    local iteration=1
    while true; do
        echo "🔄 Benchmark iteration $iteration - $(date)"
        {
            echo "=== Benchmark Iteration $iteration - $(date) ==="
            
            # Quick criterion benchmark
            timeout 30s cargo bench --package collider-benchmarks -- --sample-size 5 --measurement-time 3 2>&1 || echo "Benchmark timeout/error"
            
            echo "=== End Iteration $iteration ==="
            echo ""
        } | tee -a "$DASHBOARD_DIR/continuous_benchmarks.log"
        
        iteration=$((iteration + 1))
        sleep 10
    done
}

# Start continuous benchmarks in background
run_continuous_benchmarks &
BENCH_PID=$!

# Create live HTML dashboard
create_dashboard() {
    cat > "$DASHBOARD_DIR/dashboard.html" <<'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>Collider Live Profiling Dashboard</title>
    <meta charset="UTF-8">
    <meta http-equiv="refresh" content="5">
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; background: #f5f5f5; }
        .container { max-width: 1200px; margin: 0 auto; }
        .card { background: white; padding: 20px; margin: 10px 0; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
        .metrics { display: grid; grid-template-columns: 1fr 1fr; gap: 20px; }
        .status { padding: 10px; border-radius: 4px; margin: 5px 0; }
        .status.ok { background: #d4edda; color: #155724; }
        .status.warn { background: #fff3cd; color: #856404; }
        .status.error { background: #f8d7da; color: #721c24; }
        pre { background: #f8f9fa; padding: 15px; border-radius: 4px; overflow-x: auto; }
        .timestamp { color: #666; font-size: 0.9em; }
        h1, h2 { color: #333; }
        .links a { display: inline-block; margin: 5px 10px 5px 0; padding: 8px 15px; background: #007bff; color: white; text-decoration: none; border-radius: 4px; }
        .links a:hover { background: #0056b3; }
    </style>
</head>
<body>
    <div class="container">
        <h1>🔥 Collider Live Profiling Dashboard</h1>
        <div class="timestamp">Last updated: <span id="timestamp"></span></div>
        
        <div class="card">
            <h2>📊 Quick Links</h2>
            <div class="links">
                <a href="http://localhost:8080/health" target="_blank">App Health</a>
                <a href="http://localhost:9090" target="_blank">Prometheus</a>
                <a href="http://localhost:3000" target="_blank">Grafana</a>
                <a href="live_metrics.log" target="_blank">Live Metrics Log</a>
                <a href="continuous_benchmarks.log" target="_blank">Benchmark Log</a>
            </div>
        </div>
        
        <div class="metrics">
            <div class="card">
                <h2>📈 Latest Metrics</h2>
                <div id="latest-metrics">
                    <div class="status ok">✅ Dashboard Active</div>
                    <div class="status warn">⏳ Loading metrics...</div>
                </div>
            </div>
            
            <div class="card">
                <h2>🚀 Benchmark Status</h2>
                <div id="benchmark-status">
                    <div class="status warn">⏳ Loading benchmark data...</div>
                </div>
            </div>
        </div>
        
        <div class="card">
            <h2>📋 Instructions</h2>
            <p><strong>This dashboard auto-refreshes every 5 seconds.</strong></p>
            <ul>
                <li><strong>Live Metrics:</strong> Container stats and app health updated every 5 seconds</li>
                <li><strong>Continuous Benchmarks:</strong> Quick performance tests running every 10 seconds</li>
                <li><strong>Monitoring:</strong> Prometheus metrics at <a href="http://localhost:9090">:9090</a></li>
                <li><strong>Grafana:</strong> Visual dashboards at <a href="http://localhost:3000">:3000</a> (admin/admin)</li>
            </ul>
            
            <h3>🔧 Controls</h3>
            <p>Stop this dashboard: <code>Ctrl+C</code> in the terminal</p>
            <p>View detailed logs: Click the log links above</p>
        </div>
    </div>
    
    <script>
        document.getElementById('timestamp').textContent = new Date().toLocaleString();
        
        // Auto-refresh timestamp
        setInterval(() => {
            document.getElementById('timestamp').textContent = new Date().toLocaleString();
        }, 1000);
    </script>
</body>
</html>
EOF

    echo "📊 Dashboard created: $DASHBOARD_DIR/dashboard.html"
}

# Create the dashboard
create_dashboard

# Open dashboard in browser
if command -v open >/dev/null 2>&1; then
    echo "🌐 Opening dashboard in browser..."
    open "$DASHBOARD_DIR/dashboard.html"
elif command -v xdg-open >/dev/null 2>&1; then
    echo "🌐 Opening dashboard in browser..."
    xdg-open "$DASHBOARD_DIR/dashboard.html"
fi

echo ""
echo "📊 Live Dashboard Running!"
echo "========================="
echo "📁 Directory: $DASHBOARD_DIR"
echo "🌐 Dashboard: file://$(pwd)/$DASHBOARD_DIR/dashboard.html"
echo "📈 Prometheus: http://localhost:9090"
echo "📊 Grafana: http://localhost:3000 (admin/admin)"
echo ""
echo "📋 Real-time logs:"
echo "   tail -f $DASHBOARD_DIR/live_metrics.log"
echo "   tail -f $DASHBOARD_DIR/continuous_benchmarks.log"
echo ""
echo "🛑 Press Ctrl+C to stop all profiling..."

# Wait for user interrupt
trap "echo; echo '🛑 Stopping live dashboard...'; kill $METRICS_PID $BENCH_PID 2>/dev/null || true; exit 0" INT

# Keep the script running
while true; do
    sleep 1
done