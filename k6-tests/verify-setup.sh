#!/bin/bash
# Verify K6 Performance Testing Setup
# This script checks that all components are ready for performance testing

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔍 K6 Performance Testing Setup Verification${NC}"
echo -e "${BLUE}=============================================${NC}"
echo ""

# Function to check if command exists
check_command() {
    if command -v "$1" &> /dev/null; then
        echo -e "${GREEN}✅ $1 is installed${NC}"
        return 0
    else
        echo -e "${RED}❌ $1 is not installed${NC}"
        return 1
    fi
}

# Function to check file exists
check_file() {
    if [ -f "$1" ]; then
        echo -e "${GREEN}✅ $1 exists${NC}"
        return 0
    else
        echo -e "${RED}❌ $1 does not exist${NC}"
        return 1
    fi
}

# Function to check directory exists
check_directory() {
    if [ -d "$1" ]; then
        echo -e "${GREEN}✅ $1 directory exists${NC}"
        return 0
    else
        echo -e "${RED}❌ $1 directory does not exist${NC}"
        return 1
    fi
}

# Check prerequisites
echo -e "${YELLOW}📋 Checking Prerequisites...${NC}"
check_command k6 || echo "  Install with: brew install k6"
check_command docker || echo "  Install Docker from https://docker.com"
check_command just || echo "  Install with: brew install just"
check_command curl || echo "  Install with: brew install curl"
echo ""

# Check K6 test files
echo -e "${YELLOW}📁 Checking K6 Test Files...${NC}"
check_file "setup.js"
check_file "mass-post-events.js"
check_file "mass-get-events.js"
check_file "analytics-stress.js"
check_file "mass-delete-events.js"
check_file "seed-10million.js"
check_file "full-system-stress.js"
check_file "run-tests.sh"
check_file "README.md"
echo ""

# Check test runner is executable
echo -e "${YELLOW}🔧 Checking Test Runner...${NC}"
if [ -x "run-tests.sh" ]; then
    echo -e "${GREEN}✅ run-tests.sh is executable${NC}"
else
    echo -e "${RED}❌ run-tests.sh is not executable${NC}"
    echo "  Fix with: chmod +x run-tests.sh"
fi
echo ""

# Check results directory setup
echo -e "${YELLOW}📊 Checking Results Directory...${NC}"
if [ -d "results" ]; then
    echo -e "${GREEN}✅ results directory exists${NC}"
else
    echo -e "${YELLOW}⚠️ results directory will be created on first run${NC}"
    mkdir -p results
    echo -e "${GREEN}✅ results directory created${NC}"
fi
echo ""

# Check K6 version
echo -e "${YELLOW}📋 K6 Version Information...${NC}"
if command -v k6 &> /dev/null; then
    k6 version
else
    echo -e "${RED}❌ K6 not available${NC}"
fi
echo ""

# Test K6 basic functionality
echo -e "${YELLOW}🧪 Testing K6 Basic Functionality...${NC}"
cat > test_basic.js << 'EOF'
import { check } from 'k6';
import http from 'k6/http';

export const options = {
    vus: 1,
    duration: '5s',
    thresholds: {
        http_req_duration: ['p(95)<1000'],
    },
};

export default function() {
    const res = http.get('https://httpbin.org/status/200');
    check(res, {
        'status is 200': (r) => r.status === 200,
    });
}
EOF

if k6 run test_basic.js --quiet; then
    echo -e "${GREEN}✅ K6 basic functionality test passed${NC}"
    rm test_basic.js
else
    echo -e "${RED}❌ K6 basic functionality test failed${NC}"
    rm -f test_basic.js
fi
echo ""

# Check if service is running (optional)
echo -e "${YELLOW}🏥 Checking Local Service Health (optional)...${NC}"
if curl -s -f http://localhost:8880/health > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Local service is running and healthy${NC}"
    echo "  Ready for performance testing!"
else
    echo -e "${YELLOW}⚠️ Local service is not running${NC}"
    echo "  Start with: just dev-up && just dev-setup"
    echo "  Or use production setup: just prod-up"
fi
echo ""

# Check Docker setup
echo -e "${YELLOW}🐳 Checking Docker Environment...${NC}"
if docker ps &> /dev/null; then
    echo -e "${GREEN}✅ Docker is running${NC}"
    
    # Check if development containers are running
    if docker ps --filter "name=collider" --format "table {{.Names}}\t{{.Status}}" | grep -q "collider"; then
        echo -e "${GREEN}✅ Collider containers are running${NC}"
        docker ps --filter "name=collider" --format "table {{.Names}}\t{{.Status}}"
    else
        echo -e "${YELLOW}⚠️ No Collider containers running${NC}"
        echo "  Start with: just dev-up or just prod-up"
    fi
else
    echo -e "${RED}❌ Docker is not running${NC}"
    echo "  Start Docker Desktop or Docker daemon"
fi
echo ""

# Check justfile integration
echo -e "${YELLOW}⚙️ Checking Justfile Integration...${NC}"
cd ..
if just --list | grep -q "perf-"; then
    echo -e "${GREEN}✅ Performance testing commands available in justfile${NC}"
    echo "Available commands:"
    just --list | grep "perf-" | head -5
else
    echo -e "${RED}❌ Performance testing commands not found in justfile${NC}"
fi
cd k6-tests
echo ""

# Summary and recommendations
echo -e "${BLUE}📋 Summary and Recommendations${NC}"
echo -e "${BLUE}=============================${NC}"
echo ""

echo -e "${GREEN}✅ Setup Complete!${NC}"
echo ""
echo "🚀 Quick Start Commands:"
echo "  ./run-tests.sh smoke                    # Quick validation"
echo "  just perf-smoke                         # Same as above"
echo "  ./run-tests.sh load                     # Load testing"
echo "  ./run-tests.sh 10k-rps stress          # Full benchmark"
echo ""

echo "🔧 If service is not running:"
echo "  just dev-up && just dev-setup           # Development setup"
echo "  just prod-up                            # Production setup"
echo ""

echo "📊 Performance Testing Workflow:"
echo "  1. Start service: just dev-up && just dev-setup"
echo "  2. Quick test: just perf-smoke"
echo "  3. Load test: just perf-load"
echo "  4. Full benchmark: just perf-10k"
echo ""

echo "🎯 Production Readiness Test:"
echo "  just perf-10k http://your-production-url.com"
echo ""

echo -e "${YELLOW}⚠️ Notes:${NC}"
echo "  • 10k+ RPS tests are intensive and take 60+ minutes"
echo "  • 10M seeding test takes 3+ hours"
echo "  • Monitor system resources during stress tests"
echo "  • Results are saved in timestamped directories"
echo ""

echo -e "${GREEN}🎉 K6 Performance Testing setup verification complete!${NC}"