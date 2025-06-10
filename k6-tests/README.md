# K6 Performance Testing Suite

Comprehensive performance testing suite for the Collider application targeting 10k+ RPS production readiness.

## Quick Start

```bash
# Install K6 (if not already installed)
brew install k6

# Run smoke tests
./run-tests.sh smoke

# Run full 10k+ RPS benchmark suite
./run-tests.sh 10k-rps stress

# Run individual tests
./run-tests.sh post stress
./run-tests.sh analytics load
```

## Test Suite Overview

### 🔧 Available Tests

| Test | File | Purpose | Target RPS |
|------|------|---------|------------|
| **Mass POST Events** | `mass-post-events.js` | Event creation stress test | 10k+ |
| **Mass GET Events** | `mass-get-events.js` | Event retrieval with pagination | 8k+ |
| **Analytics Stress** | `analytics-stress.js` | Analytics endpoints load | 2k+ |
| **Mass Delete Events** | `mass-delete-events.js` | Bulk deletion operations | 500+ |
| **10M Seeding** | `seed-10million.js` | Database seeding performance | N/A |
| **Full System Stress** | `full-system-stress.js` | Combined mixed workload | 10k+ |

### 📊 Test Profiles

- **smoke**: Quick validation (2-5 minutes)
- **load**: Normal load testing (10-15 minutes)  
- **stress**: Maximum stress testing (30-60 minutes)
- **10million**: Special profile for seeding 10M events (3+ hours)

## Detailed Test Descriptions

### Mass POST Events Test
- **Target**: 10k+ RPS event creation
- **Features**: 
  - Gradual ramp-up to 10k RPS
  - Spike testing to 15k RPS
  - Sustained high load
  - Event data variety
- **Thresholds**: 95% under 2s, 99% under 5s, <5% error rate

### Mass GET Events Test
- **Target**: 8k+ RPS event retrieval
- **Features**:
  - Pagination stress testing
  - Deep pagination (pages 50-500)
  - Large page sizes (up to 10k events)
  - User-specific queries
- **Thresholds**: 95% under 3s, 99% under 8s, <3% error rate

### Analytics Stress Test
- **Target**: 2k+ RPS analytics queries
- **Features**:
  - General stats endpoints
  - Real-time metrics (<1s response)
  - Complex aggregations
  - Time-based analytics
- **Thresholds**: 95% under 4s, complex queries under 8s

### Mass Delete Events Test
- **Target**: 500+ RPS deletion operations
- **Features**:
  - Single event deletions
  - Bulk deletion by date
  - Bulk deletion by user
  - Bulk deletion by event type
- **Thresholds**: 95% under 5s, bulk operations under 12s

### 10M Event Seeding Test
- **Target**: Seed 10 million events
- **Features**:
  - 200 concurrent workers
  - 1000 events per batch
  - Progress monitoring
  - Memory usage tracking
- **Duration**: ~3 hours

### Full System Stress Test
- **Target**: 10k+ RPS mixed workload
- **Features**:
  - 40% event creation
  - 35% event queries  
  - 20% analytics
  - 5% deletions
  - System monitoring
- **Duration**: 1 hour

## Usage Examples

### Quick Validation
```bash
# Test basic functionality
./run-tests.sh smoke

# Test specific endpoint
./run-tests.sh post smoke
```

### Load Testing
```bash
# Standard load test
./run-tests.sh load

# Analytics load test
./run-tests.sh analytics load
```

### Stress Testing
```bash
# Maximum stress test
./run-tests.sh stress

# Full system stress
./run-tests.sh full-system stress
```

### Production Readiness
```bash
# Complete 10k+ RPS benchmark
./run-tests.sh 10k-rps stress

# Against production environment
./run-tests.sh 10k-rps stress https://your-prod-url.com
```

### Database Seeding
```bash
# Regular seeding test
./run-tests.sh seeding load

# 10 million event challenge
./run-tests.sh seeding 10million
```

## Environment Setup

### Local Development
```bash
# Start development environment
just dev-up

# Run migrations and seed
just dev-setup

# Run smoke tests
./run-tests.sh smoke
```

### Production Testing
```bash
# Build production image
docker build -t collider-app:latest .

# Start production environment
just prod-up

# Run full benchmark
./run-tests.sh 10k-rps stress http://localhost:8880
```

## Results and Analysis

### Output Files
Each test run creates a timestamped results directory:
```
results/YYYYMMDD_HHMMSS/
├── test_summary.md          # Summary report
├── *_dashboard.html         # K6 web dashboard
├── *_results.json          # Detailed JSON results
├── *_results.csv           # CSV data
└── *_output.log            # Test execution logs
```

### Key Metrics to Monitor

#### Response Times
- p95 < 2s for event operations
- p99 < 5s for event operations  
- p95 < 1s for real-time analytics
- p95 < 4s for complex analytics

#### Throughput
- 10k+ RPS sustained for event creation
- 8k+ RPS sustained for event queries
- 2k+ RPS sustained for analytics
- 500+ RPS for deletion operations

#### Error Rates
- <5% error rate for all operations
- <3% error rate for read operations
- <2% error rate for analytics

#### System Health
- Memory usage stable
- Database connections within limits
- Health checks responding <100ms

## Troubleshooting

### Common Issues

#### Low RPS Performance
1. Check database configuration
2. Verify connection pooling settings
3. Monitor CPU and memory usage
4. Check for database locks

#### High Error Rates
1. Verify service is healthy
2. Check database connectivity
3. Monitor resource constraints
4. Review application logs

#### Timeouts
1. Increase timeout values for complex queries
2. Optimize database queries
3. Add database indexes
4. Scale database resources

### Performance Tuning Tips

#### Application Level
- Optimize database queries
- Add appropriate indexes
- Configure connection pooling
- Enable query caching

#### Database Level
- Tune PostgreSQL configuration
- Monitor query performance
- Add database indexes
- Consider read replicas

#### Infrastructure Level
- Scale horizontally
- Optimize container resources
- Use CDN for static content
- Configure load balancing

## Integration with CI/CD

### GitHub Actions Example
```yaml
- name: Performance Tests
  run: |
    just prod-up
    ./k6-tests/run-tests.sh smoke
    ./k6-tests/run-tests.sh load
```

### Local Development Workflow
```bash
# Development cycle
just dev-setup              # Setup environment
./k6-tests/run-tests.sh smoke   # Quick validation
# Make changes...
./k6-tests/run-tests.sh load    # Full test
```

## Custom Test Development

### Adding New Tests
1. Create new test file in `k6-tests/`
2. Follow existing patterns from `setup.js`
3. Add to `run-tests.sh` script
4. Document in this README

### Test Structure
```javascript,ignore
// Import common utilities
import { ... } from './setup.js';

// Define test options
export const options = { ... };

// Setup function
export function setup() { ... }

// Main test function  
export default function(data) { ... }

// Cleanup function
export function teardown(data) { ... }
```

## Performance Targets Summary

| Metric | Target | Measurement |
|--------|--------|-------------|
| Event Creation RPS | 10,000+ | Sustained 5+ minutes |
| Event Query RPS | 8,000+ | Mixed pagination load |
| Analytics RPS | 2,000+ | Complex queries included |
| Total System RPS | 10,000+ | Mixed workload |
| P95 Response Time | <2s | Event operations |
| P99 Response Time | <5s | Event operations |
| Error Rate | <5% | All operations |
| 10M Event Seeding | <3 hours | Database population |

These targets ensure production readiness for high-scale deployment on GCP with headroom for traffic spikes.