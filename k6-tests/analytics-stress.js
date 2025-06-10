// Analytics Endpoints Stress Test - Test all analytics aggregation endpoints
import { check, sleep } from 'k6';
import http from 'k6/http';
import { Rate, Counter, Trend, Gauge } from 'k6/metrics';
import { generateUuid, defaultHeaders, BASE_URL, waitForService, verifyResponse } from './setup.js';

// Custom metrics for analytics
const analyticsQueryRate = new Rate('analytics_query_success_rate');
const analyticsQueryTime = new Trend('analytics_query_time');
const analyticsQueryThroughput = new Counter('analytics_queries_total');
const aggregationComplexity = new Trend('aggregation_complexity_time');
const realTimeMetricsLatency = new Trend('realtime_metrics_latency');
const cacheHitRate = new Rate('analytics_cache_hit_rate');

export const options = {
    scenarios: {
        // General analytics load
        analytics_load: {
            executor: 'ramping-vus',
            startVUs: 0,
            stages: [
                { duration: '2m', target: 100 },   // Warm up
                { duration: '5m', target: 500 },   // 500 concurrent analytics users
                { duration: '5m', target: 1000 },  // 1k concurrent users
                { duration: '10m', target: 2000 }, // 2k concurrent users - heavy analytics load
                { duration: '5m', target: 2000 },  // Sustain heavy load
                { duration: '2m', target: 0 },     // Cool down
            ],
        },
        
        // Real-time metrics stress
        realtime_stress: {
            executor: 'constant-arrival-rate',
            startTime: '30m',
            rate: 1000,     // 1000 requests per second
            timeUnit: '1s',
            duration: '10m',
            preAllocatedVUs: 100,
            maxVUs: 500,
        },
        
        // Complex aggregation queries
        complex_aggregations: {
            executor: 'ramping-vus',
            startTime: '42m',
            startVUs: 0,
            stages: [
                { duration: '2m', target: 50 },
                { duration: '8m', target: 200 },
                { duration: '2m', target: 0 },
            ],
        }
    },
    
    thresholds: {
        http_req_duration: ['p(95)<5000', 'p(99)<10000'], // Analytics can be slower
        http_req_failed: ['rate<0.02'],                    // Error rate under 2%
        analytics_query_success_rate: ['rate>0.98'],      // 98% success rate
        analytics_query_time: ['p(95)<4000'],             // 95% under 4s
        aggregation_complexity_time: ['p(90)<8000'],      // Complex queries under 8s
        realtime_metrics_latency: ['p(95)<1000'],         // Real-time under 1s
        http_reqs: ['rate>800'],                          // Target 800+ RPS
    },
    
    setupTimeout: '5m',
    teardownTimeout: '2m',
    userAgent: 'k6-analytics-stress/1.0',
};

let testUsers = [];

export function setup() {
    console.log('🚀 Starting Analytics Stress Test Setup...');
    
    waitForService();
    
    // Generate user IDs for analytics queries
    console.log('📝 Generating test user IDs for analytics...');
    for (let i = 0; i < 200; i++) {
        testUsers.push(generateUuid());
    }
    
    console.log('✅ Analytics stress test setup complete');
    return { testUsers };
}

export default function(data) {
    const testType = __ITER % 10;
    
    if (testType < 3) {
        // 30% - General stats endpoint
        testGeneralStats(data);
    } else if (testType < 5) {
        // 20% - User analytics
        testUserAnalytics(data);
    } else if (testType < 7) {
        // 20% - Real-time metrics
        testRealTimeMetrics(data);
    } else if (testType < 8) {
        // 10% - Time-based aggregations
        testTimeBasedAggregations(data);
    } else if (testType < 9) {
        // 10% - Event type analytics
        testEventTypeAnalytics(data);
    } else {
        // 10% - Complex multi-dimensional queries
        testComplexAggregations(data);
    }
}

function testGeneralStats(data) {
    const response = http.get(`${BASE_URL}/api/analytics/stats`, {
        headers: defaultHeaders,
        timeout: '8s',
    });
    
    analyticsQueryTime.add(response.timings.duration);
    
    const success = verifyResponse(response, 200, 'general stats');
    analyticsQueryRate.add(success);
    
    if (success) {
        analyticsQueryThroughput.add(1);
        
        check(response, {
            'stats has total events': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return body.total_events !== undefined;
                } catch (e) {
                    return false;
                }
            },
            'stats has total users': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return body.total_users !== undefined;
                } catch (e) {
                    return false;
                }
            },
            'stats response time acceptable': (r) => r.timings.duration < 3000,
        });
        
        // Check for cache headers
        const cacheHit = response.headers['X-Cache-Status'] === 'HIT';
        cacheHitRate.add(cacheHit);
    }
    
    sleep(0.1);
}

function testUserAnalytics(data) {
    const userId = data.testUsers[Math.floor(Math.random() * data.testUsers.length)];
    
    const response = http.get(`${BASE_URL}/api/users/${userId}/analytics`, {
        headers: defaultHeaders,
        timeout: '10s',
    });
    
    analyticsQueryTime.add(response.timings.duration);
    
    const success = verifyResponse(response, 200, 'user analytics');
    analyticsQueryRate.add(success);
    
    if (success) {
        analyticsQueryThroughput.add(1);
        
        check(response, {
            'user analytics has event count': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return body.event_count !== undefined;
                } catch (e) {
                    return false;
                }
            },
            'user analytics has user_id': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return body.user_id === userId;
                } catch (e) {
                    return false;
                }
            },
            'user analytics response time': (r) => r.timings.duration < 5000,
        });
    }
    
    sleep(0.05);
}

function testRealTimeMetrics(data) {
    const response = http.get(`${BASE_URL}/api/analytics/metrics/realtime`, {
        headers: defaultHeaders,
        timeout: '3s',
    });
    
    analyticsQueryTime.add(response.timings.duration);
    realTimeMetricsLatency.add(response.timings.duration);
    
    const success = verifyResponse(response, 200, 'realtime metrics');
    analyticsQueryRate.add(success);
    
    if (success) {
        analyticsQueryThroughput.add(1);
        
        check(response, {
            'realtime metrics fast response': (r) => r.timings.duration < 1500,
            'realtime metrics has current data': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return body.current_active_users !== undefined;
                } catch (e) {
                    return false;
                }
            },
            'realtime metrics has timestamp': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return body.timestamp !== undefined;
                } catch (e) {
                    return false;
                }
            },
        });
    }
    
    sleep(0.02); // Real-time should be fast
}

function testTimeBasedAggregations(data) {
    const timeRanges = [
        'last_hour',
        'last_24_hours', 
        'last_7_days',
        'last_30_days'
    ];
    
    const timeRange = timeRanges[Math.floor(Math.random() * timeRanges.length)];
    
    const response = http.get(`${BASE_URL}/api/analytics/events/timeline?range=${timeRange}`, {
        headers: defaultHeaders,
        timeout: '12s',
    });
    
    analyticsQueryTime.add(response.timings.duration);
    aggregationComplexity.add(response.timings.duration);
    
    const success = verifyResponse(response, 200, 'time-based aggregation');
    analyticsQueryRate.add(success);
    
    if (success) {
        analyticsQueryThroughput.add(1);
        
        check(response, {
            'timeline has data points': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return Array.isArray(body.timeline) && body.timeline.length > 0;
                } catch (e) {
                    return false;
                }
            },
            'timeline aggregation performance': (r) => r.timings.duration < 6000,
        });
    }
    
    sleep(0.15);
}

function testEventTypeAnalytics(data) {
    const response = http.get(`${BASE_URL}/api/analytics/events/by-type`, {
        headers: defaultHeaders,
        timeout: '8s',
    });
    
    analyticsQueryTime.add(response.timings.duration);
    
    const success = verifyResponse(response, 200, 'event type analytics');
    analyticsQueryRate.add(success);
    
    if (success) {
        analyticsQueryThroughput.add(1);
        
        check(response, {
            'event types has breakdown': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return body.event_types && Object.keys(body.event_types).length > 0;
                } catch (e) {
                    return false;
                }
            },
            'event type analytics performance': (r) => r.timings.duration < 4000,
        });
    }
    
    sleep(0.08);
}

function testComplexAggregations(data) {
    // Test complex multi-dimensional analytics queries
    const complexQueries = [
        '/api/analytics/funnel?steps=login,page_view,purchase',
        '/api/analytics/cohort?period=weekly&cohort_size=30',
        '/api/analytics/retention?period=daily&cohorts=7',
        '/api/analytics/segmentation?segment=power_users&metric=engagement'
    ];
    
    const query = complexQueries[Math.floor(Math.random() * complexQueries.length)];
    
    const response = http.get(`${BASE_URL}${query}`, {
        headers: defaultHeaders,
        timeout: '20s', // Complex queries can take longer
    });
    
    analyticsQueryTime.add(response.timings.duration);
    aggregationComplexity.add(response.timings.duration);
    
    const success = verifyResponse(response, 200, 'complex aggregation');
    analyticsQueryRate.add(success);
    
    if (success) {
        analyticsQueryThroughput.add(1);
        
        check(response, {
            'complex query completes': (r) => r.status === 200,
            'complex query reasonable time': (r) => r.timings.duration < 15000,
            'complex query has results': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return Object.keys(body).length > 0;
                } catch (e) {
                    return false;
                }
            },
        });
    }
    
    sleep(0.3); // Longer delay for complex queries
}

export function teardown(data) {
    console.log('🧹 Analytics Stress Test Teardown...');
    console.log(`📊 Test completed with ${analyticsQueryThroughput.count} analytics queries executed`);
    
    // Final health check
    const healthResponse = http.get(`${BASE_URL}/health`, { headers: defaultHeaders });
    console.log(`🏥 Final health check: ${healthResponse.status}`);
    
    // Get final analytics performance stats
    const statsResponse = http.get(`${BASE_URL}/api/analytics/stats`, { headers: defaultHeaders });
    if (statsResponse.status === 200) {
        console.log('📈 Final analytics stats response time:', statsResponse.timings.duration, 'ms');
        try {
            const stats = JSON.parse(statsResponse.body);
            console.log('📊 Final analytics data:', {
                totalEvents: stats.total_events,
                totalUsers: stats.total_users,
                responseTime: statsResponse.timings.duration
            });
        } catch (e) {
            console.log('❌ Could not parse final analytics stats');
        }
    }
    
    // Test cache performance under load
    const cacheTestResponse = http.get(`${BASE_URL}/api/analytics/metrics/realtime`, { headers: defaultHeaders });
    console.log('⚡ Final cache test response time:', cacheTestResponse.timings.duration, 'ms');
}

export const testProfiles = {
    smoke: {
        scenarios: {
            smoke_analytics: {
                executor: 'constant-vus',
                vus: 3,
                duration: '2m',
            }
        }
    },
    
    load: {
        scenarios: {
            load_analytics: {
                executor: 'ramping-vus',
                startVUs: 0,
                stages: [
                    { duration: '2m', target: 50 },
                    { duration: '5m', target: 200 },
                    { duration: '2m', target: 0 },
                ],
            }
        }
    },
    
    stress: {
        scenarios: {
            stress_analytics: {
                executor: 'ramping-arrival-rate',
                preAllocatedVUs: 200,
                maxVUs: 1000,
                stages: [
                    { duration: '2m', target: 500 },
                    { duration: '8m', target: 1500 },
                    { duration: '5m', target: 3000 },
                    { duration: '2m', target: 0 },
                ],
            }
        }
    }
};