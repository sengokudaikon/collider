#!/bin/bash
# Test script for local development environment

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Base URL
BASE_URL="${BASE_URL:-http://localhost:8880}"

# Test counter
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Test function
test_endpoint() {
    local name=$1
    local url=$2
    local expected_status=${3:-200}
    local method=${4:-GET}
    local data=${5:-}
    
    TESTS_RUN=$((TESTS_RUN + 1))
    
    echo -n "Testing $name... "
    
    if [ -n "$data" ]; then
        response=$(curl -s -w "\n%{http_code}" -X "$method" -H "Content-Type: application/json" -d "$data" "$url" 2>/dev/null)
    else
        response=$(curl -s -w "\n%{http_code}" -X "$method" "$url" 2>/dev/null)
    fi
    
    http_code=$(echo "$response" | tail -n1)
    body=$(echo "$response" | sed '$d')
    
    if [ "$http_code" = "$expected_status" ]; then
        echo -e "${GREEN}PASS${NC} (HTTP $http_code)"
        TESTS_PASSED=$((TESTS_PASSED + 1))
        if [ -n "$body" ] && [ "${VERBOSE:-false}" = true ]; then
            echo "  Response: $body"
        fi
    else
        echo -e "${RED}FAIL${NC} (Expected: $expected_status, Got: $http_code)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        if [ -n "$body" ]; then
            echo "  Response: $body"
        fi
    fi
}

# Check if services are ready
check_services() {
    echo -e "${BLUE}Checking service health...${NC}"
    
    # Check app health
    if curl -s -f "$BASE_URL/health" > /dev/null 2>&1; then
        echo -e "  App: ${GREEN}Healthy${NC}"
    else
        echo -e "  App: ${RED}Not ready${NC}"
        return 1
    fi
    
    # Check database (through Docker)
    if docker exec collider-postgres pg_isready -U postgres > /dev/null 2>&1; then
        echo -e "  PostgreSQL: ${GREEN}Ready${NC}"
    else
        echo -e "  PostgreSQL: ${RED}Not ready${NC}"
        return 1
    fi
    
    # Check Dragonfly
    if docker exec collider-dragonfly redis-cli -a "development" ping > /dev/null 2>&1; then
        echo -e "  Dragonfly: ${GREEN}Ready${NC}"
    else
        echo -e "  Dragonfly: ${RED}Not ready${NC}"
        return 1
    fi
    
    # Check Jaeger
    if curl -s -f "http://localhost:16686/api/services" > /dev/null 2>&1; then
        echo -e "  Jaeger: ${GREEN}Ready${NC}"
    else
        echo -e "  Jaeger: ${YELLOW}Not ready (optional)${NC}"
    fi
    
    # Check Prometheus
    if curl -s -f "http://localhost:9090/-/ready" > /dev/null 2>&1; then
        echo -e "  Prometheus: ${GREEN}Ready${NC}"
    else
        echo -e "  Prometheus: ${YELLOW}Not ready (optional)${NC}"
    fi
    
    # Check Grafana
    if curl -s -f "http://localhost:3000/api/health" > /dev/null 2>&1; then
        echo -e "  Grafana: ${GREEN}Ready${NC}"
    else
        echo -e "  Grafana: ${YELLOW}Not ready (optional)${NC}"
    fi
    
    echo ""
    return 0
}

# Verbose mode
VERBOSE=false
if [ "${1:-}" = "-v" ] || [ "${1:-}" = "--verbose" ]; then
    VERBOSE=true
fi

# Main tests
echo -e "${BLUE}=== Local Development Environment Tests ===${NC}"
echo ""

# Check services
if ! check_services; then
    echo -e "${RED}Core services are not ready. Please ensure the environment is running:${NC}"
    echo "  ./scripts/deploy.sh -e local"
    exit 1
fi

# Run tests
echo -e "${BLUE}Running API tests...${NC}"
echo ""

# Health endpoints
test_endpoint "App Health Check" "$BASE_URL/health" 200

# API endpoints
test_endpoint "API Root" "$BASE_URL/" 200

# Create test event (if events API exists)
EVENT_DATA='{"type":"test_event","data":{"message":"Local development test"}}'
test_endpoint "Create Event" "$BASE_URL/api/events" 201 "POST" "$EVENT_DATA"

# Test database connectivity
test_endpoint "Database Status" "$BASE_URL/api/health/db" 200

# Test cache connectivity
test_endpoint "Cache Status" "$BASE_URL/api/health/cache" 200

# Performance test (optional)
if [ "${VERBOSE}" = true ]; then
    echo ""
    echo -e "${BLUE}Running performance test...${NC}"
    
    start_time=$(date +%s%N)
    for i in {1..10}; do
        curl -s -o /dev/null "$BASE_URL/health"
    done
    end_time=$(date +%s%N)
    
    duration=$(( (end_time - start_time) / 1000000 ))
    avg_response=$(( duration / 10 ))
    
    echo "  10 requests completed in ${duration}ms (avg: ${avg_response}ms per request)"
fi

# Summary
echo ""
echo -e "${BLUE}=== Test Summary ===${NC}"
echo "Tests run: $TESTS_RUN"
echo -e "Passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Failed: ${RED}$TESTS_FAILED${NC}"

if [ "$TESTS_FAILED" -eq 0 ]; then
    echo ""
    echo -e "${GREEN}All tests passed! Your local environment is working correctly.${NC}"
    echo ""
    echo "Available services:"
    echo "  - Application: http://localhost:8880"
    echo "  - PostgreSQL: localhost:5432"
    echo "  - Dragonfly: localhost:6379"
    echo "  - Jaeger: http://localhost:16686"
    echo "  - Prometheus: http://localhost:9090"  
    echo "  - Grafana: http://localhost:3000 (admin/admin)"
    exit 0
else
    echo ""
    echo -e "${RED}Some tests failed${NC}"
    exit 1
fi