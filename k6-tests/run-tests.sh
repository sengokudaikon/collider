#!/bin/bash
# K6 Performance Test Runner
# Usage: ./run-tests.sh [test_type] [profile] [base_url]

set -e

# Default values
TEST_TYPE=${1:-"smoke"}
PROFILE=${2:-"smoke"}
BASE_URL=${3:-"http://localhost:8880"}
RESULTS_DIR="./results/$(date +%Y%m%d_%H%M%S)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Create results directory
mkdir -p "$RESULTS_DIR"

echo -e "${BLUE}🚀 K6 Performance Test Runner${NC}"
echo -e "${BLUE}================================${NC}"
echo "Test Type: $TEST_TYPE"
echo "Profile: $PROFILE"
echo "Base URL: $BASE_URL"
echo "Results: $RESULTS_DIR"
echo ""

# Function to run a test
run_test() {
    local test_file=$1
    local test_name=$2
    local options_file=$3
    
    echo -e "${YELLOW}📊 Running $test_name...${NC}"
    
    # Set K6 options
    export K6_WEB_DASHBOARD=true
    export K6_WEB_DASHBOARD_EXPORT="$RESULTS_DIR/${test_name}_dashboard.html"
    
    # Run the test
    k6 run \
        --env BASE_URL="$BASE_URL" \
        --out json="$RESULTS_DIR/${test_name}_results.json" \
        --out csv="$RESULTS_DIR/${test_name}_results.csv" \
        ${options_file:+--config "$options_file"} \
        "$test_file" \
        2>&1 | tee "$RESULTS_DIR/${test_name}_output.log"
    
    local exit_code=$?
    
    if [ $exit_code -eq 0 ]; then
        echo -e "${GREEN}✅ $test_name completed successfully${NC}"
    else
        echo -e "${RED}❌ $test_name failed with exit code $exit_code${NC}"
    fi
    
    echo ""
    return $exit_code
}

# Function to check service health
check_service() {
    echo -e "${YELLOW}🏥 Checking service health...${NC}"
    
    local max_attempts=30
    local attempt=1
    
    while [ $attempt -le $max_attempts ]; do
        if curl -s -f "$BASE_URL/health" > /dev/null; then
            echo -e "${GREEN}✅ Service is healthy${NC}"
            return 0
        fi
        
        echo "Attempt $attempt/$max_attempts: Service not ready, waiting..."
        sleep 2
        attempt=$((attempt + 1))
    done
    
    echo -e "${RED}❌ Service health check failed after $max_attempts attempts${NC}"
    return 1
}

# Function to generate summary report
generate_summary() {
    echo -e "${BLUE}📋 Generating test summary...${NC}"
    
    local summary_file="$RESULTS_DIR/test_summary.md"
    
    cat > "$summary_file" << EOF
# Performance Test Summary

**Date:** $(date)
**Test Type:** $TEST_TYPE
**Profile:** $PROFILE
**Base URL:** $BASE_URL

## Test Results

EOF

    # Add results for each test that was run
    for log_file in "$RESULTS_DIR"/*_output.log; do
        if [ -f "$log_file" ]; then
            local test_name=$(basename "$log_file" "_output.log")
            echo "### $test_name" >> "$summary_file"
            echo '```' >> "$summary_file"
            tail -20 "$log_file" >> "$summary_file"
            echo '```' >> "$summary_file"
            echo "" >> "$summary_file"
        fi
    done
    
    echo -e "${GREEN}✅ Summary generated: $summary_file${NC}"
}

# Main test execution logic
main() {
    # Check service health first
    if ! check_service; then
        echo -e "${RED}❌ Cannot proceed without healthy service${NC}"
        exit 1
    fi
    
    case $TEST_TYPE in
        "smoke")
            echo -e "${BLUE}🔍 Running smoke tests (quick validation)${NC}"
            run_test "mass-post-events.js" "smoke_post_events"
            run_test "mass-get-events.js" "smoke_get_events"
            run_test "analytics-stress.js" "smoke_analytics"
            ;;
            
        "load")
            echo -e "${BLUE}📈 Running load tests${NC}"
            run_test "mass-post-events.js" "load_post_events"
            run_test "mass-get-events.js" "load_get_events"
            run_test "analytics-stress.js" "load_analytics"
            run_test "mass-delete-events.js" "load_delete_events"
            ;;
            
        "stress")
            echo -e "${BLUE}🔥 Running stress tests${NC}"
            run_test "mass-post-events.js" "stress_post_events"
            run_test "mass-get-events.js" "stress_get_events"
            run_test "analytics-stress.js" "stress_analytics"
            run_test "mass-delete-events.js" "stress_delete_events"
            ;;
            
        "full-system")
            echo -e "${BLUE}🌟 Running full system stress test${NC}"
            run_test "full-system-stress.js" "full_system_stress"
            ;;
            
        "seeding")
            echo -e "${BLUE}🌱 Running seeding performance test${NC}"
            if [ "$PROFILE" = "10million" ]; then
                echo -e "${YELLOW}⚠️ 10 million event seeding test - this will take 3+ hours${NC}"
                read -p "Are you sure you want to continue? (y/N): " -n 1 -r
                echo
                if [[ ! $REPLY =~ ^[Yy]$ ]]; then
                    echo "Test cancelled"
                    exit 0
                fi
            fi
            run_test "seed-10million.js" "seed_performance"
            ;;
            
        "post")
            echo -e "${BLUE}📝 Running POST events test only${NC}"
            run_test "mass-post-events.js" "post_events_$PROFILE"
            ;;
            
        "get")
            echo -e "${BLUE}📖 Running GET events test only${NC}"
            run_test "mass-get-events.js" "get_events_$PROFILE"
            ;;
            
        "analytics")
            echo -e "${BLUE}📊 Running analytics test only${NC}"
            run_test "analytics-stress.js" "analytics_$PROFILE"
            ;;
            
        "delete")
            echo -e "${BLUE}🗑️ Running delete events test only${NC}"
            run_test "mass-delete-events.js" "delete_events_$PROFILE"
            ;;
            
        "10k-rps")
            echo -e "${BLUE}🎯 Running 10k+ RPS benchmark suite${NC}"
            echo -e "${YELLOW}This is the full production readiness test${NC}"
            
            # Run individual tests first
            run_test "mass-post-events.js" "10k_post_events"
            run_test "mass-get-events.js" "10k_get_events"
            run_test "analytics-stress.js" "10k_analytics"
            run_test "mass-delete-events.js" "10k_delete_events"
            
            # Then run the combined system test
            run_test "full-system-stress.js" "10k_full_system"
            ;;
            
        *)
            echo -e "${RED}❌ Unknown test type: $TEST_TYPE${NC}"
            echo ""
            echo "Available test types:"
            echo "  smoke       - Quick validation tests"
            echo "  load        - Standard load tests"
            echo "  stress      - High load stress tests"
            echo "  full-system - Combined system stress test"
            echo "  seeding     - Database seeding performance"
            echo "  post        - POST events only"
            echo "  get         - GET events only"
            echo "  analytics   - Analytics endpoints only"
            echo "  delete      - Delete operations only"
            echo "  10k-rps     - Full 10k+ RPS benchmark suite"
            echo ""
            echo "Available profiles:"
            echo "  smoke       - Quick validation"
            echo "  load        - Normal load testing"
            echo "  stress      - Maximum stress testing"
            echo "  10million   - 10M event seeding (seeding test only)"
            exit 1
            ;;
    esac
    
    # Generate summary report
    generate_summary
    
    echo -e "${GREEN}🎉 All tests completed!${NC}"
    echo -e "${BLUE}📁 Results available in: $RESULTS_DIR${NC}"
    
    # Show quick stats
    if command -v jq &> /dev/null; then
        echo -e "${BLUE}📊 Quick Stats:${NC}"
        for json_file in "$RESULTS_DIR"/*_results.json; do
            if [ -f "$json_file" ]; then
                local test_name=$(basename "$json_file" "_results.json")
                echo -n "  $test_name: "
                jq -r '.metrics.http_reqs.count // "N/A"' "$json_file" | tr -d '\n'
                echo " requests"
            fi
        done
    fi
}

# Show usage if help requested
if [[ "$1" == "-h" || "$1" == "--help" ]]; then
    echo "Usage: $0 [test_type] [profile] [base_url]"
    echo ""
    echo "Examples:"
    echo "  $0 smoke                          # Quick smoke tests"
    echo "  $0 load stress                    # Load tests with stress profile"
    echo "  $0 10k-rps stress http://prod.com # Full production benchmark"
    echo "  $0 seeding 10million              # 10M event seeding test"
    echo "  $0 post stress                    # POST events stress test only"
    echo ""
    echo "Test Types: smoke, load, stress, full-system, seeding, post, get, analytics, delete, 10k-rps"
    echo "Profiles: smoke, load, stress, 10million"
    exit 0
fi

# Run main function
main