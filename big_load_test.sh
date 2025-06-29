#!/bin/bash

set -e

# Configuration
HOST="$1"
TEST_SCENARIO="${2:-mixed}"  # mixed, burst, sustained, ramp-up, realistic

# Load test parameters
BASE_REQUESTS="${LT_REQUESTS:-1000}"
BASE_CONCURRENCY="${LT_CONCURRENCY:-50}"
DURATION="${LT_DURATION:-60}"  # seconds for sustained tests
TIMEOUT="${LT_TIMEOUT:-30}"

# Data settings
EXPORT_DIR="${LT_EXPORT_DIR:-./load_testing/data}"
USE_LIVE_DATA="${LT_USE_LIVE_DATA:-true}"
PARALLEL_WORKERS="${LT_WORKERS:-4}"

# Tracking
TEMP_DIR="./collider_load_test_$$"
RESULTS_DIR="./load_test_results_$(date +%Y%m%d_%H%M%S)"
TOTAL_REQUESTS_SENT=0
TOTAL_ERRORS=0

usage() {
    cat << EOF
Usage: $0 <host> [scenario]

Advanced load testing for Collider with realistic patterns

Arguments:
  host      Server URL (required) - e.g., http://127.0.0.1:8880
  scenario  Test scenario (default: mixed)
            - mixed: Mix of all patterns
            - burst: Sudden traffic spikes
            - sustained: Constant load over time
            - ramp-up: Gradually increasing load
            - realistic: Simulates real user behavior
            - ultrakill: Extreme rapid-fire bullethell requests
            - bfg: Maximum server-killing load (use with caution!)

Environment Variables:
  LT_REQUESTS      Base requests count (default: 1000)
  LT_CONCURRENCY   Base concurrent connections (default: 50)
  LT_DURATION      Duration for sustained tests in seconds (default: 60)
  LT_TIMEOUT       Request timeout in seconds (default: 30)
  LT_WORKERS       Parallel worker processes (default: 4)
  LT_USE_LIVE_DATA Use csv-exporter for real data (default: true)
  LT_EXPORT_DIR    Directory for exported data (default: ./load_testing/data)

Examples:
  $0 http://127.0.0.1:8880
  $0 http://127.0.0.1:8880 burst
  LT_DURATION=300 $0 http://127.0.0.1:8880 sustained
  LT_WORKERS=8 $0 http://127.0.0.1:8880 realistic

EOF
}

validate_parameters() {
    if [ -z "$HOST" ]; then
        echo "❌ Error: Host parameter is required"
        usage
        exit 1
    fi

    if [[ ! "$HOST" =~ ^https?:// ]]; then
        echo "❌ Error: Invalid host format. Use http://hostname:port"
        exit 1
    fi

    case "$TEST_SCENARIO" in
        mixed|burst|sustained|ramp-up|realistic|ultrakill|bfg)
            ;;
        *)
            echo "❌ Error: Invalid scenario: $TEST_SCENARIO"
            usage
            exit 1
            ;;
    esac
}

check_dependencies() {
    local missing_deps=()
    
    for cmd in ab curl jq bc; do
        if ! command -v "$cmd" &> /dev/null; then
            missing_deps+=("$cmd")
        fi
    done
    
    if [ ${#missing_deps[@]} -gt 0 ]; then
        echo "❌ Error: Missing required dependencies: ${missing_deps[*]}"
        echo "   Install with:"
        echo "   - Ubuntu/Debian: apt-get install apache2-utils curl jq bc"
        echo "   - macOS: brew install httpd curl jq bc"
        exit 1
    fi

    if [ "$USE_LIVE_DATA" = "true" ] && ! command -v cargo &> /dev/null; then
        echo "⚠  Warning: cargo not found. Cannot use live data"
        USE_LIVE_DATA="false"
    fi
}

setup_directories() {
    mkdir -p "$TEMP_DIR"
    mkdir -p "$RESULTS_DIR"
    mkdir -p "$EXPORT_DIR"
}

cleanup() {
    echo "🧹 Cleaning up..."
    [ -d "$TEMP_DIR" ] && rm -rf "$TEMP_DIR"
}

export_live_data() {
    if [ "$USE_LIVE_DATA" != "true" ]; then
        return 0
    fi

    echo "📦 Checking live data from database..."
    
    local csv_exporter_path="./binaries/csv-exporter"
    
    if [ ! -d "$csv_exporter_path" ] && [ -d "../binaries/csv-exporter" ]; then
        csv_exporter_path="../binaries/csv-exporter"
    fi
    
    if [ ! -d "$csv_exporter_path" ]; then
        echo "⚠  Warning: csv-exporter not found. Using synthetic data"
        USE_LIVE_DATA="false"
        return 0
    fi
    
    # Check if exported data already exists and is fresh
    local data_age_minutes="${LT_DATA_FRESHNESS:-30}"  # Default: 30 minutes
    local data_is_fresh=false
    
    if [ -f "$EXPORT_DIR/event_types.csv" ] && [ -f "$EXPORT_DIR/users.csv" ]; then
        # Check if files are newer than the specified age
        local newest_file=$(find "$EXPORT_DIR" -name "*.csv" -type f -exec stat -f "%m %N" {} \; 2>/dev/null | sort -nr | head -1 | cut -d' ' -f2)
        
        if [ -n "$newest_file" ]; then
            # Get file age in minutes (works on both macOS and Linux)
            local file_age_minutes
            if [[ "$OSTYPE" == "darwin"* ]]; then
                # macOS
                file_age_minutes=$(( ($(date +%s) - $(stat -f "%m" "$newest_file")) / 60 ))
            else
                # Linux
                file_age_minutes=$(( ($(date +%s) - $(stat -c "%Y" "$newest_file")) / 60 ))
            fi
            
            if [ "$file_age_minutes" -le "$data_age_minutes" ]; then
                data_is_fresh=true
                echo "✅ Using existing data (${file_age_minutes}min old, fresh within ${data_age_minutes}min)"
            else
                echo "🔄 Data is ${file_age_minutes}min old, refreshing (freshness limit: ${data_age_minutes}min)"
            fi
        fi
    else
        echo "📂 No existing data files found, will export fresh data"
    fi
    
    # Skip export if data is fresh
    if [ "$data_is_fresh" = "true" ]; then
        return 0
    fi
    
    # Create export directory if it doesn't exist
    mkdir -p "$EXPORT_DIR"
    
    # Build csv-exporter if binary doesn't exist or is older than source
    local exporter_bin="$csv_exporter_path/target/release/csv-exporter"
    local needs_build=false
    
    if [ ! -f "$exporter_bin" ]; then
        needs_build=true
        echo "🔨 Building csv-exporter (binary not found)..."
    else
        # Check if source is newer than binary
        local src_file="$csv_exporter_path/src/main.rs"
        if [ -f "$src_file" ] && [ "$src_file" -nt "$exporter_bin" ]; then
            needs_build=true
            echo "🔨 Rebuilding csv-exporter (source updated)..."
        fi
    fi
    
    if [ "$needs_build" = "true" ]; then
        if ! (cd "$csv_exporter_path" && cargo build --release --quiet); then
            echo "⚠  Warning: Failed to build csv-exporter. Using synthetic data"
            USE_LIVE_DATA="false"
            return 0
        fi
    else
        echo "✅ Using existing csv-exporter binary"
    fi
    
    # Export data
    echo "📊 Exporting fresh data from database..."
    if ! "$exporter_bin" all --output-dir "$EXPORT_DIR" > /dev/null 2>&1; then
        echo "⚠  Warning: Failed to export data. Using synthetic data"
        USE_LIVE_DATA="false"
        return 0
    fi
    
    echo "✅ Successfully exported live data"
}

load_test_data() {
    echo "📊 Loading test data..."
    
    # Load event types
    if [ "$USE_LIVE_DATA" = "true" ] && [ -f "$EXPORT_DIR/event_types.csv" ]; then
        EVENT_TYPES=$(tail -n +2 "$EXPORT_DIR/event_types.csv" | cut -d',' -f2 | tr -d '"' | grep -v '^$')
        echo "✅ Loaded $(echo "$EVENT_TYPES" | wc -l) event types from database"
    else
        EVENT_TYPES="api.request
api.response
api.error
api.rate_limited
admin.login
admin.logout
admin.settings_updated
admin.updated_user
admin.deleted_user
admin.generated_report
user.login
user.logout
user.profile_updated
user.password_changed
system.startup
system.shutdown
system.health_check
payment.initiated
payment.completed
payment.failed"
        echo "ℹ️  Using synthetic event types"
    fi
    
    # Load users
    if [ "$USE_LIVE_DATA" = "true" ] && [ -f "$EXPORT_DIR/users.csv" ]; then
        USER_IDS=$(tail -n +2 "$EXPORT_DIR/users.csv" | cut -d',' -f1 | tr -d '"' | grep -v '^$' | head -100)
        echo "✅ Loaded $(echo "$USER_IDS" | wc -l) users from database"
    else
        # Generate synthetic user IDs
        USER_IDS=$(seq 1 50)
        echo "ℹ️  Using synthetic user IDs (1-50)"
    fi
    
    # Convert to arrays
    EVENT_TYPE_ARRAY=($(echo "$EVENT_TYPES"))
    USER_ID_ARRAY=($(echo "$USER_IDS"))
    
    # Ensure arrays are not empty
    if [ ${#EVENT_TYPE_ARRAY[@]} -eq 0 ]; then
        echo "❌ Error: No event types loaded"
        exit 1
    fi
    
    if [ ${#USER_ID_ARRAY[@]} -eq 0 ]; then
        echo "❌ Error: No user IDs loaded"
        exit 1
    fi
    
    echo "📊 Loaded ${#EVENT_TYPE_ARRAY[@]} event types and ${#USER_ID_ARRAY[@]} user IDs"
}

# Safe function to get random array element
get_random_user() {
    if [ ${#USER_ID_ARRAY[@]} -gt 0 ]; then
        echo "${USER_ID_ARRAY[$((RANDOM % ${#USER_ID_ARRAY[@]}))]}"
    else
        echo "1"  # Default user ID
    fi
}

get_random_event_type() {
    if [ ${#EVENT_TYPE_ARRAY[@]} -gt 0 ]; then
        echo "${EVENT_TYPE_ARRAY[$((RANDOM % ${#EVENT_TYPE_ARRAY[@]}))]}"
    else
        echo "api.request"  # Default event type
    fi
}

generate_event_payload() {
    local user_id=$1
    local event_type=$2
    local output_file=$3
    
    # Realistic metadata based on event type
    local metadata=""
    case "$event_type" in
        api.*)
            local endpoints=("/api/v1/users" "/api/v1/events" "/api/v1/stats" "/api/v1/health")
            local endpoint=${endpoints[$RANDOM % ${#endpoints[@]}]}
            local methods=("GET" "POST" "PUT" "DELETE")
            local method=${methods[$RANDOM % ${#methods[@]}]}
            local status_codes=(200 201 400 401 403 404 500)
            local status=${status_codes[$RANDOM % ${#status_codes[@]}]}
            metadata='"endpoint": "'$endpoint'", "method": "'$method'", "status": '$status', "duration_ms": '$((RANDOM % 1000 + 50))
            ;;
        admin.*)
            local pages=("/admin/dashboard" "/admin/users" "/admin/settings" "/admin/reports")
            local page=${pages[$RANDOM % ${#pages[@]}]}
            metadata='"page": "'$page'", "ip": "192.168.1.'$((RANDOM % 255 + 1))'", "user_agent": "Mozilla/5.0"'
            ;;
        user.*)
            local actions=("view_profile" "edit_profile" "change_password" "update_settings")
            local action=${actions[$RANDOM % ${#actions[@]}]}
            metadata='"action": "'$action'", "device": "web", "browser": "Chrome"'
            ;;
        payment.*)
            local amounts=("9.99" "29.99" "49.99" "99.99" "149.99")
            local amount="${amounts[$RANDOM % ${#amounts[@]}]}"
            local currencies=("USD" "EUR" "GBP")
            local currency=${currencies[$RANDOM % ${#currencies[@]}]}
            metadata='"amount": '$amount', "currency": "'$currency'", "payment_method": "card"'
            ;;
        system.*)
            metadata='"node": "server-'$((RANDOM % 4 + 1))'", "version": "1.2.3"'
            ;;
        *)
            metadata='"source": "load_test", "test_id": "'$$'"'
            ;;
    esac
    
    cat > "$output_file" << EOF
{
  "user_id": $user_id,
  "event_type": "$event_type",
  "metadata": {
    $metadata,
    "session_id": "session-$user_id-$(date +%s)",
    "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "test_run": true,
    "scenario": "$TEST_SCENARIO"
  }
}
EOF
}

generate_test_payloads() {
    echo "📝 Generating test payloads..."
    
    local num_payloads=100
    for i in $(seq 1 $num_payloads); do
        local user_id=$(get_random_user)
        local event_type=$(get_random_event_type)
        generate_event_payload "$user_id" "$event_type" "$TEMP_DIR/event_$i.json"
    done
    
    echo "✅ Generated $num_payloads test payloads with varied data"
}

run_ab_test() {
    local test_name="$1"
    local url="$2"
    local requests="$3"
    local concurrency="$4"
    local extra_args="$5"
    
    # Generate unique identifier for this test run to avoid race conditions
    local test_id="${RANDOM}_$(date +%s%N)"
    local output_file="$TEMP_DIR/ab_output_${test_id}.txt"
    local result_file="$RESULTS_DIR/${test_name// /_}.json"
    local log_file="$RESULTS_DIR/${test_name// /_}.log"
    
    echo "  Running: $test_name (${requests}r/${concurrency}c)"
    
    if ab -n "$requests" -c "$concurrency" -s "$TIMEOUT" -g "$TEMP_DIR/gnuplot_${test_id}.dat" \
        $extra_args "$url" > "$output_file" 2>&1; then
        
        # Save full output for debugging
        cp "$output_file" "$log_file"
        
        # Parse results
        local rps=$(grep "Requests per second" "$output_file" | awk '{print $4}')
        local avg_time=$(grep "Time per request.*mean" "$output_file" | head -1 | awk '{print $4}')
        local failed=$(grep "Failed requests" "$output_file" | awk '{print $3}')
        local non_2xx=$(grep "Non-2xx responses" "$output_file" | awk '{print $3}' 2>/dev/null || true)
        non_2xx=${non_2xx:-0}
        
        failed=${failed:-0}
        non_2xx=${non_2xx:-0}
        
        # Debug: Check for unexpected values
        if [ "$non_2xx" != "0" ] && [ -n "$non_2xx" ]; then
            echo "    ⚠️  Non-2xx responses detected: $non_2xx (likely server errors under load)"
            # Apache Bench doesn't show individual status codes in summary
            # Based on our testing, these are usually 500 errors (database timeouts)
            if [ "$non_2xx" -eq "$requests" ]; then
                echo "       - All requests failed (100% error rate)"
                echo "       - Likely cause: Database connection pool exhausted"
            elif [ "$non_2xx" -gt 0 ]; then
                local error_percentage=$((non_2xx * 100 / requests))
                echo "       - Error rate: ${error_percentage}%"
            fi
            
            # If this was a POST request, save the payload for debugging
            if [[ "$extra_args" == *"-p"* ]]; then
                local payload_path=$(echo "$extra_args" | sed -n 's/.*-p \([^ ]*\).*/\1/p')
                if [ -f "$payload_path" ]; then
                    echo "    📋 Payload that caused errors:"
                    cat "$payload_path" | jq -c . 2>/dev/null || cat "$payload_path"
                    cp "$payload_path" "$RESULTS_DIR/${test_name// /_}_failed_payload.json"
                fi
            fi
        fi
        
        # Save results
        cat > "$result_file" << EOF
{
  "test_name": "$test_name",
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "requests": $requests,
  "concurrency": $concurrency,
  "rps": ${rps:-0},
  "avg_response_time_ms": ${avg_time:-0},
  "failed_requests": ${failed},
  "non_2xx_responses": ${non_2xx},
  "success_rate": $(if [ $requests -gt 0 ]; then echo "scale=2; (($requests - $failed - $non_2xx) * 100) / $requests" | bc; else echo "0"; fi)
}
EOF
        
        TOTAL_REQUESTS_SENT=$((TOTAL_REQUESTS_SENT + requests))
        TOTAL_ERRORS=$((TOTAL_ERRORS + failed + non_2xx))
        
        echo "    ✅ RPS: $rps, Avg: ${avg_time}ms, Success: $(if [ $requests -gt 0 ]; then echo "scale=2; (($requests - $failed - $non_2xx) * 100) / $requests" | bc; else echo "0"; fi)%"
    else
        echo "    ❌ Test failed"
        TOTAL_ERRORS=$((TOTAL_ERRORS + requests))
    fi
    
    rm -f "$output_file" "$TEMP_DIR/gnuplot_${test_id}.dat"
}

run_multi_endpoint_test() {
    local test_name="$1"
    local requests_per_endpoint="$2"
    local concurrency="$3"
    
    echo "🔄 $test_name"
    
    # Run tests on different endpoints in parallel
    (
        run_ab_test "$test_name - Events" "$HOST/events?limit=100" "$requests_per_endpoint" "$concurrency" ""
    ) &
    
    (
        local user_id=$(get_random_user)
        run_ab_test "$test_name - User Events" "$HOST/user/$user_id/events?limit=50" "$requests_per_endpoint" "$concurrency" ""
    ) &
    
    (
        local event_type=$(get_random_event_type)
        run_ab_test "$test_name - Stats" "$HOST/stats?type=$event_type" "$requests_per_endpoint" "$concurrency" ""
    ) &
    
    # Wait for all parallel tests to complete
    wait
}

scenario_burst() {
    echo "💥 Running BURST scenario..."
    echo "  Simulating sudden traffic spikes"
    
    # Normal load
    run_ab_test "Burst - Baseline" "$HOST/events" 100 10 ""
    
    # Sudden spike
    sleep 2
    run_ab_test "Burst - Spike 1" "$HOST/events" 500 100 ""
    
    # Back to normal
    sleep 2
    run_ab_test "Burst - Recovery" "$HOST/events" 100 10 ""
    
    # Another spike with POST requests
    sleep 2
    local payload_file="$TEMP_DIR/event_burst.json"
    for i in {1..10}; do
        generate_event_payload "$(get_random_user)" "$(get_random_event_type)" "$payload_file"
        run_ab_test "Burst - POST Spike $i" "$HOST/event" 200 50 \
                   "-p $payload_file -T application/json"
    done
}

scenario_sustained() {
    echo "⏱️  Running SUSTAINED scenario..."
    echo "  Maintaining constant load for $DURATION seconds"
    
    local start_time=$(date +%s)
    local end_time=$((start_time + DURATION))
    local iteration=1
    
    while [ $(date +%s) -lt $end_time ]; do
        echo "  Iteration $iteration ($(( end_time - $(date +%s) ))s remaining)"
        
        # Mix of operations
        run_ab_test "Sustained - GET Events $iteration" "$HOST/events?limit=100" 100 20 "" &
        
        local user_id=$(get_random_user)
        run_ab_test "Sustained - User Events $iteration" "$HOST/user/$user_id/events" 100 20 "" &
        
        # POST events
        local payload_file="$TEMP_DIR/sustained_$iteration.json"
        generate_event_payload "$user_id" "$(get_random_event_type)" "$payload_file"
        run_ab_test "Sustained - POST $iteration" "$HOST/event" 50 10 "-p $payload_file -T application/json" &
        
        # Wait for parallel tests to complete
        wait
        
        iteration=$((iteration + 1))
        sleep 1
    done
}

scenario_ramp_up() {
    echo "📈 Running RAMP-UP scenario..."
    echo "  Gradually increasing load"
    
    local max_concurrency=100
    local steps=10
    
    for step in $(seq 1 $steps); do
        local concurrency=$((step * max_concurrency / steps))
        local requests=$((step * 100))
        
        echo "  Step $step/$steps: ${requests}r/${concurrency}c"
        
        # Increasing GET load
        run_ab_test "Ramp-up - Step $step GET" "$HOST/events" "$requests" "$concurrency" ""
        
        # Increasing POST load
        local payload_file="$TEMP_DIR/rampup_$step.json"
        generate_event_payload "$(get_random_user)" \
                             "$(get_random_event_type)" \
                             "$payload_file"
        run_ab_test "Ramp-up - Step $step POST" "$HOST/event" "$((requests / 2))" "$((concurrency / 2))" \
                   "-p $payload_file -T application/json"
        
        sleep 2
    done
}

scenario_realistic() {
    echo "🌍 Running REALISTIC scenario..."
    echo "  Simulating real-world usage patterns"
    
    # Simulate different user behaviors
    echo "  Phase 1: Morning traffic (gradual increase)"
    for i in {1..5}; do
        local concurrency=$((i * 10))
        run_multi_endpoint_test "Morning Traffic $i" 50 "$concurrency"
        sleep 1
    done
    
    echo "  Phase 2: Peak hours (high sustained load)"
    for i in {1..10}; do
        # Parallel user sessions
        for worker in $(seq 1 $PARALLEL_WORKERS); do
            (
                # User session simulation
                local user_id=$(get_random_user)
                
                # User views their events
                curl -s "$HOST/user/$user_id/events?limit=20" > /dev/null
                
                # User creates new events
                for _ in {1..3}; do
                    local event_type=$(get_random_event_type)
                    local payload_file="$TEMP_DIR/realistic_${worker}_${i}.json"
                    generate_event_payload "$user_id" "$event_type" "$payload_file"
                    curl -s -X POST -H "Content-Type: application/json" -d "@$payload_file" "$HOST/event" > /dev/null
                done
                
                # User checks stats
                curl -s "$HOST/stats?type=$(get_random_event_type)" > /dev/null
            ) &
        done
        wait
        echo "    Completed peak hour batch $i"
    done
    
    echo "  Phase 3: Evening decline"
    for i in {5..1}; do
        local concurrency=$((i * 10))
        run_multi_endpoint_test "Evening Traffic $i" 30 "$concurrency"
        sleep 2
    done
    
    echo "  Phase 4: Night maintenance (health checks)"
    for i in {1..20}; do
        curl -s "$HOST/" > /dev/null
        sleep 0.5
    done
}

scenario_mixed() {
    echo "🎲 Running MIXED scenario..."
    echo "  Combining all test patterns"
    
    # Run different scenarios in sequence
    scenario_burst
    echo ""
    
    # Short sustained test
    local original_duration=$DURATION
    DURATION=30
    scenario_sustained
    DURATION=$original_duration
    echo ""
    
    scenario_ramp_up
    echo ""
    
    scenario_realistic
}

scenario_ultrakill() {
    echo "💀🔫 Running ULTRAKILL scenario..."
    echo "  BULLETHELL MODE: Extreme rapid-fire request assault!"
    echo "  ⚠️  WARNING: This will stress your server significantly!"
    
    # Parallel assault configuration
    local assault_waves=20
    local bullets_per_wave=10
    local base_concurrency=200
    
    echo "  Launching $assault_waves waves of $bullets_per_wave parallel attacks each"
    
    for wave in $(seq 1 $assault_waves); do
        echo "  🌊 WAVE $wave/$assault_waves - INCOMING!"
        
        # Launch multiple parallel "bullets" 
        for bullet in $(seq 1 $bullets_per_wave); do
            (
                # Randomize attack parameters for chaos
                local concurrency=$((base_concurrency + RANDOM % 300))  # 200-500 concurrent
                local requests=$((500 + RANDOM % 1000))  # 500-1500 requests
                local endpoint_choice=$((RANDOM % 4))
                
                case $endpoint_choice in
                    0)
                        # Rapid GET assault
                        run_ab_test "ULTRAKILL-W${wave}B${bullet}-GET" \
                            "$HOST/events?limit=1000" \
                            "$requests" "$concurrency" "" &
                        ;;
                    1)
                        # User endpoint barrage
                        local user_id=$(get_random_user)
                        run_ab_test "ULTRAKILL-W${wave}B${bullet}-USER" \
                            "$HOST/user/$user_id/events?limit=500" \
                            "$requests" "$concurrency" "" &
                        ;;
                    2)
                        # Stats endpoint spam
                        local event_type=$(get_random_event_type)
                        run_ab_test "ULTRAKILL-W${wave}B${bullet}-STATS" \
                            "$HOST/stats?type=$event_type" \
                            "$requests" "$concurrency" "" &
                        ;;
                    3)
                        # POST bullet spray
                        local payload_file="$TEMP_DIR/ultrakill_${wave}_${bullet}.json"
                        generate_event_payload "$(get_random_user)" \
                                             "$(get_random_event_type)" \
                                             "$payload_file"
                        run_ab_test "ULTRAKILL-W${wave}B${bullet}-POST" \
                            "$HOST/event" \
                            "$((requests / 2))" "$((concurrency / 2))" \
                            "-p $payload_file -T application/json" &
                        ;;
                esac
            ) &
            
            # Minimal delay between bullets (50ms)
            sleep 0.05
        done
        
        # Wait for wave to complete before next one
        wait
        
        # Minimal break between waves (100ms)
        echo "    💥 Wave $wave complete! Next wave in 100ms..."
        sleep 0.1
    done
    
    echo "  ☠️  ULTRAKILL assault complete! Check server vitals!"
}

scenario_bfg() {
    echo "🔥💣 Running BFG scenario..."
    echo "  BIG F***ING GUN MODE: Maximum server annihilation!"
    echo "  ⚠️  EXTREME WARNING: This WILL attempt to break your server!"
    echo "  ⚠️  Only use this in test environments!"
    
    # Maximum assault parameters
    local max_concurrency=1000
    local max_requests=10000
    local parallel_cannons=20  # Number of parallel BFG shots
    local sustained_duration=120  # 2 minutes of hell
    
    echo "  🎯 Target acquired: $HOST"
    echo "  🔋 Charging BFG capacitors..."
    sleep 2
    
    echo "  💥 FIRING BFG! BRACE FOR IMPACT!"
    
    local start_time=$(date +%s)
    local end_time=$((start_time + sustained_duration))
    local shot_number=1
    
    while [ $(date +%s) -lt $end_time ]; do
        echo "  🔥 BFG SHOT #$shot_number ($(( end_time - $(date +%s) ))s of destruction remaining)"
        
        # Launch parallel cannon blasts
        for cannon in $(seq 1 $parallel_cannons); do
            (
                # Randomize destruction patterns
                local attack_type=$((RANDOM % 5))
                
                case $attack_type in
                    0)
                        # Maximum GET devastation
                        run_ab_test "BFG-S${shot_number}C${cannon}-OBLITERATE-GET" \
                            "$HOST/events?limit=10000" \
                            "$max_requests" "$max_concurrency" \
                            "-k" &  # Keep-alive for sustained pressure
                        ;;
                    1)
                        # User endpoint annihilation
                        local user_id=$(get_random_user)
                        run_ab_test "BFG-S${shot_number}C${cannon}-DESTROY-USER" \
                            "$HOST/user/$user_id/events?limit=5000" \
                            "$max_requests" "$max_concurrency" \
                            "-k" &
                        ;;
                    2)
                        # Stats endpoint decimation
                        local event_type=$(get_random_event_type)
                        run_ab_test "BFG-S${shot_number}C${cannon}-WRECK-STATS" \
                            "$HOST/stats?type=$event_type&detailed=true" \
                            "$max_requests" "$max_concurrency" \
                            "-k" &
                        ;;
                    3)
                        # POST payload bombardment
                        local payload_file="$TEMP_DIR/bfg_${shot_number}_${cannon}.json"
                        generate_event_payload "$(get_random_user)" \
                                             "$(get_random_event_type)" \
                                             "$payload_file"
                        run_ab_test "BFG-S${shot_number}C${cannon}-BOMBARD-POST" \
                            "$HOST/event" \
                            "$((max_requests / 2))" "$((max_concurrency / 2))" \
                            "-p $payload_file -T application/json -k" &
                        ;;
                    4)
                        # Mixed endpoint chaos
                        local endpoints=("/events" "/stats" "/" "/user/1/events")
                        local endpoint=${endpoints[$RANDOM % ${#endpoints[@]}]}
                        run_ab_test "BFG-S${shot_number}C${cannon}-CHAOS" \
                            "$HOST$endpoint" \
                            "$max_requests" "$max_concurrency" \
                            "-k" &
                        ;;
                esac
            ) &
        done
        
        # Don't wait between shots - continuous assault
        shot_number=$((shot_number + 1))
        
        # Check if we should wait (every 5 shots)
        if [ $((shot_number % 5)) -eq 0 ]; then
            echo "    ⏳ Letting parallel attacks complete before next barrage..."
            wait
        fi
    done
    
    # Final cleanup wait
    echo "  ⏳ Waiting for final destruction to complete..."
    wait
    
    echo "  ☠️💀☠️ BFG assault complete!"
    echo "  📊 Damage assessment:"
    echo "     - Total shots fired: $shot_number"
    echo "     - Estimated requests: $((shot_number * parallel_cannons * max_requests / 2))"
    echo "     - Duration: $sustained_duration seconds"
    echo "  🏥 Recommend checking server health immediately!"
}

generate_report() {
    echo ""
    echo "📊 Generating performance report..."
    
    local report_file="$RESULTS_DIR/report.md"
    
    cat > "$report_file" << EOF
# Load Test Report

**Date:** $(date)
**Host:** $HOST
**Scenario:** $TEST_SCENARIO
**Total Requests:** $TOTAL_REQUESTS_SENT
**Total Errors:** $TOTAL_ERRORS
**Overall Success Rate:** $(if [ $TOTAL_REQUESTS_SENT -gt 0 ]; then echo "scale=2; (($TOTAL_REQUESTS_SENT - $TOTAL_ERRORS) * 100) / $TOTAL_REQUESTS_SENT" | bc; else echo "0"; fi)%

## Configuration
- Base Requests: $BASE_REQUESTS
- Base Concurrency: $BASE_CONCURRENCY
- Duration (sustained): $DURATION seconds
- Parallel Workers: $PARALLEL_WORKERS
- Live Data: $USE_LIVE_DATA

## Test Results
EOF
    
    # Aggregate results
    if [ -n "$(ls -A $RESULTS_DIR/*.json 2>/dev/null)" ]; then
        echo -e "\n### Individual Test Results\n" >> "$report_file"
        
        for result in "$RESULTS_DIR"/*.json; do
            if [ -f "$result" ]; then
                local test_name=$(jq -r '.test_name' "$result")
                local rps=$(jq -r '.rps' "$result")
                local avg_time=$(jq -r '.avg_response_time_ms' "$result")
                local success_rate=$(jq -r '.success_rate' "$result")
                
                echo "- **$test_name**: ${rps} req/s, ${avg_time}ms avg, ${success_rate}% success" >> "$report_file"
            fi
        done
        
        # Calculate aggregates
        echo -e "\n### Aggregate Statistics\n" >> "$report_file"
        
        local avg_rps=$(jq -s 'map(.rps) | add/length' "$RESULTS_DIR"/*.json 2>/dev/null || echo "0")
        local avg_response=$(jq -s 'map(.avg_response_time_ms) | add/length' "$RESULTS_DIR"/*.json 2>/dev/null || echo "0")
        local min_rps=$(jq -s 'map(.rps) | min' "$RESULTS_DIR"/*.json 2>/dev/null || echo "0")
        local max_rps=$(jq -s 'map(.rps) | max' "$RESULTS_DIR"/*.json 2>/dev/null || echo "0")
        
        echo "- Average RPS: $avg_rps" >> "$report_file"
        echo "- Average Response Time: ${avg_response}ms" >> "$report_file"
        echo "- Min RPS: $min_rps" >> "$report_file"
        echo "- Max RPS: $max_rps" >> "$report_file"
    fi
    
    echo -e "\n## Recommendations\n" >> "$report_file"
    
    if [ "$TOTAL_REQUESTS_SENT" -gt 0 ] && [ "$TOTAL_ERRORS" -gt $((TOTAL_REQUESTS_SENT / 100)) ]; then
        echo "⚠️  High error rate detected (>1%). Consider:" >> "$report_file"
        echo "- Database timeouts under load (500 errors)" >> "$report_file"
        echo "- Connection pool exhaustion" >> "$report_file"
        echo "- Reducing concurrent connections" >> "$report_file"
        echo "- Increasing database connection pool size" >> "$report_file"
        echo "- Optimizing slow queries" >> "$report_file"
        echo "" >> "$report_file"
        echo "Check server logs and failed payload files in: $RESULTS_DIR" >> "$report_file"
    else
        echo "✅ Error rate is within acceptable limits (<1%)" >> "$report_file"
    fi
    
    echo ""
    echo "📄 Full report saved to: $report_file"
    echo "📁 Results directory: $RESULTS_DIR"
}

main() {
    trap cleanup EXIT
    
    validate_parameters
    check_dependencies
    setup_directories
    
    echo "🚀 Advanced Load Testing for Collider"
    echo "====================================="
    echo "Host: $HOST"
    echo "Scenario: $TEST_SCENARIO"
    echo "Workers: $PARALLEL_WORKERS"
    echo ""
    
    # Prepare test data
    export_live_data
    load_test_data
    generate_test_payloads
    
    # Run selected scenario
    case "$TEST_SCENARIO" in
        burst)
            scenario_burst
            ;;
        sustained)
            scenario_sustained
            ;;
        ramp-up)
            scenario_ramp_up
            ;;
        realistic)
            scenario_realistic
            ;;
        mixed)
            scenario_mixed
            ;;
        ultrakill)
            scenario_ultrakill
            ;;
        bfg)
            scenario_bfg
            ;;
    esac
    
    # Generate report
    generate_report
    
    echo ""
    echo "✅ Load testing completed!"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi