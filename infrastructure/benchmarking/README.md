# Collider Performance Testing Suite

A comprehensive performance testing infrastructure with multiple tools, monitoring, and regression detection.

## 🎯 Overview

This suite provides everything needed for thorough performance testing of the Collider application:

- **5 Different Testing Tools**: Vegeta, Goose, Criterion, k6, Yandex Tank
- **Unified Orchestration**: Single command to run all tools
- **Monitoring Integration**: Prometheus + Grafana dashboards
- **Regression Detection**: Automated baseline comparison
- **Comprehensive Reporting**: Detailed reports from all tools

## 🚀 Quick Start

```bash
# Quick validation (2 minutes)
just perf-quick

# Standard load testing (20 minutes)
just perf-load

# Full comprehensive suite (45 minutes)
just perf-full

# View results
just perf-results
```

## 🛠️ Available Tools

### 1. **Vegeta** - HTTP Load Testing
- **Purpose**: High-performance HTTP load testing
- **Language**: Go
- **Strengths**: Simple, fast, excellent for basic HTTP testing
- **Usage**: `just perf-vegeta`

### 2. **Goose** - Rust Load Testing  
- **Purpose**: Realistic user behavior simulation
- **Language**: Rust
- **Strengths**: Native performance, async/await support, detailed metrics
- **Usage**: `just perf-goose`

### 3. **Criterion** - Micro-benchmarking
- **Purpose**: Statistical benchmarking with regression detection
- **Language**: Rust
- **Strengths**: Statistical analysis, outlier detection, HTML reports
- **Usage**: `just perf-criterion`

### 4. **k6** - JavaScript Load Testing
- **Purpose**: Modern load testing with scriptable scenarios
- **Language**: JavaScript
- **Strengths**: Scripting flexibility, cloud integration, modern APIs
- **Usage**: `just perf-k6`

### 5. **Yandex Tank** - Comprehensive Testing
- **Purpose**: High-load testing with system monitoring
- **Language**: Python
- **Strengths**: System monitoring, autostop conditions, detailed analysis
- **Usage**: `just perf-tank`

## 📊 Test Scenarios

### API Endpoints Tested
- `GET /health` - Health check monitoring
- `POST /api/events` - Event creation (primary load)
- `GET /api/events` - Event listing with pagination
- `GET /api/events/{id}` - Individual event retrieval
- `PUT /api/events/{id}` - Event updates
- `DELETE /api/events/{id}` - Event deletion

### Workload Patterns
- **Realistic**: 40% create, 30% list, 15% get, 7% update, 3% delete, 5% health
- **Read-Heavy**: 50% list, 35% get, 10% create, 2% update, 1% delete, 2% health
- **Write-Heavy**: 70% create, 20% list, 5% get, 3% update, 1% delete, 1% health

## 🎛️ Test Suites

### Quick (`just perf-quick`)
- **Duration**: ~2 minutes
- **Tools**: k6 smoke test only
- **Purpose**: Fast validation

### Load (`just perf-load`)
- **Duration**: ~20 minutes  
- **Tools**: All tools with standard load patterns
- **Purpose**: Comprehensive load testing

### Stress (`just perf-stress`)
- **Duration**: ~20 minutes
- **Tools**: k6 stress + Yandex Tank stress
- **Purpose**: High-load stress testing

### Full (`just perf-full`)
- **Duration**: ~45 minutes
- **Tools**: All tools with all test types
- **Purpose**: Complete performance validation

## 📈 Monitoring & Observability

### Prometheus Metrics
- HTTP request rates and latencies
- Error rates by endpoint
- System resource utilization
- Custom performance metrics

### Grafana Dashboards
- Real-time performance visualization
- Historical trend analysis
- Alert configurations
- Multi-tool comparison views

### Start Monitoring
```bash
just perf-monitoring-start
# Grafana: http://localhost:3000 (admin/admin)
# Prometheus: http://localhost:9090
```

## 🔍 Regression Detection

### Create Baseline
```bash
# Run tests first
just perf-load

# Create baseline from results
just perf-baseline
```

### Check for Regressions
```bash
# Run new tests
just perf-load

# Compare against baseline
just perf-regression
```

### Thresholds
- **Latency**: >10% increase = regression
- **Throughput**: >10% decrease = regression  
- **Error Rate**: >5% increase = regression

## 📁 Directory Structure

```
infrastructure/benchmarking/
├── orchestrate-performance-tests.sh    # Main orchestration script
├── performance-regression-detector.sh  # Regression detection
├── run_load_test.sh                    # Existing Vegeta script
├── goose_load_test.rs                  # Existing Goose config
├── criterion_bench.rs                  # Existing Criterion benchmarks
├── run-goose.sh                        # Goose wrapper script
├── run-criterion.sh                    # Criterion wrapper script
├── yandex-tank/                        # Yandex Tank configuration
│   ├── load.yaml                       # Tank load profile
│   ├── ammo.txt                        # Request ammunition
│   ├── monitoring.xml                  # System monitoring
│   └── run_tank.sh                     # Tank execution script
├── k6/                                 # k6 test scripts
│   ├── load-test.js                    # Main k6 test
│   ├── run-k6.sh                       # k6 wrapper script
│   └── scenarios/                      # Additional test scenarios
├── scenarios/                          # Test scenario definitions
│   ├── comprehensive-scenarios.json    # All scenario configs
│   ├── event-api-scenarios.yaml       # API-specific tests
│   └── mixed-workload.js              # Mixed workload k6 script
├── orchestrated_results/               # Unified test results
├── baselines/                          # Performance baselines
└── regression_reports/                 # Regression analysis
```

## 🎯 Usage Examples

### Basic Testing
```bash
# Test localhost (default)
just perf-quick

# Test specific environment  
just perf-load http://staging.example.com

# Custom Goose test
just perf-goose http://localhost:8080 500 50/1s 600
```

### Advanced Usage
```bash
# Custom orchestration
./infrastructure/benchmarking/orchestrate-performance-tests.sh \
  http://localhost:8080 load

# Individual tool with custom settings
cd infrastructure/benchmarking
./k6/run-k6.sh http://localhost:8080 stress

# Run with monitoring disabled
MONITORING_ENABLED=false just perf-load
```

### Results Analysis
```bash
# View all results
just perf-results

# Check for regressions with custom threshold
REGRESSION_THRESHOLD=5 just perf-regression

# Clean all test results
just perf-clean
```

## 📋 Reports Generated

### Orchestration Report
- **Location**: `orchestrated_results/orchestration_report_TIMESTAMP.md`
- **Content**: Summary across all tools, performance comparison, recommendations

### Individual Tool Reports
- **Vegeta**: Text and JSON reports with latency percentiles
- **Goose**: HTML reports with detailed user simulation metrics
- **Criterion**: Statistical HTML reports with regression detection
- **k6**: JSON summaries with custom metrics
- **Yandex Tank**: Comprehensive load testing reports

### Regression Reports
- **Location**: `regression_reports/regression_report_TIMESTAMP.md`
- **Content**: Baseline comparison, regression detection, actionable insights

## ⚡ Performance Targets

### Response Time Targets
- **Health Check**: p95 < 100ms, p99 < 200ms
- **Event Creation**: p95 < 500ms, p99 < 1000ms
- **Event Listing**: p95 < 300ms, p99 < 600ms
- **Event Retrieval**: p95 < 150ms, p99 < 300ms

### Throughput Targets
- **Total RPS**: 10,000+ requests per second
- **Event Creation**: 4,000+ events per second
- **Read Operations**: 5,000+ reads per second

### Error Rate Targets
- **Maximum Error Rate**: < 1%
- **Maximum Timeout Rate**: < 0.5%

## 🔧 Troubleshooting

### Common Issues

#### Server Not Available
```bash
# Check server health
curl http://localhost:8080/health

# Start local server
just dev
```

#### Tool Not Found
```bash
# Install missing tools
brew install vegeta k6        # macOS
pip3 install yandextank       # Yandex Tank
```

#### Permission Errors
```bash
# Make scripts executable
chmod +x infrastructure/benchmarking/*.sh
chmod +x infrastructure/benchmarking/*/*.sh
```

#### Memory Issues
```bash
# Reduce load for resource-constrained environments
just perf-goose http://localhost:8080 100 10/1s 60
```

### Performance Debugging

#### High Latency
1. Check system resources during tests
2. Review application logs for bottlenecks
3. Analyze database query performance
4. Check network latency

#### High Error Rates
1. Review error patterns by endpoint
2. Check rate limiting configurations
3. Verify database connection limits
4. Analyze timeout settings

## 🚀 Next Steps

### CI/CD Integration
```yaml
# GitHub Actions example
- name: Performance Testing
  run: |
    just perf-quick
    just perf-regression
```

### Production Monitoring
- Set up continuous performance monitoring
- Establish performance SLAs
- Create alerting for regressions
- Schedule regular performance tests

### Optimization
- Use results to identify bottlenecks
- Set performance budgets for new features
- Track performance trends over time
- Optimize based on real-world usage patterns

## 📖 Additional Resources

- [Vegeta Documentation](https://github.com/tsenart/vegeta)
- [Goose Load Testing](https://book.goose.rs/)
- [Criterion Benchmarking](https://bheisler.github.io/criterion.rs/book/)
- [k6 Documentation](https://k6.io/docs/)
- [Yandex Tank](https://yandextank.readthedocs.io/)
- [Prometheus + Grafana Setup](../config/monitoring/)

---

**Happy Performance Testing! 🎯⚡**