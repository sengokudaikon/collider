// Mass GET /events - Stress test event retrieval with pagination and 10M+ records
import { check, sleep } from 'k6';
import http from 'k6/http';
import { Rate, Counter, Trend } from 'k6/metrics';
import { generateUuid, defaultHeaders, BASE_URL, waitForService, verifyResponse } from './setup.js';

// Custom metrics
const eventQueryRate = new Rate('event_query_success_rate');
const eventQueryTime = new Trend('event_query_time');
const eventQueryThroughput = new Counter('event_queries_total');
const paginationEfficiency = new Trend('pagination_efficiency');

// Test configuration targeting high pagination loads
export const options = {
    scenarios: {
        // Basic pagination load test
        pagination_load: {
            executor: 'ramping-vus',
            startVUs: 0,
            stages: [
                { duration: '2m', target: 200 },   // Warm up
                { duration: '5m', target: 1000 },  // 1k concurrent users
                { duration: '5m', target: 2000 },  // 2k concurrent users
                { duration: '10m', target: 3000 }, // 3k concurrent users
                { duration: '5m', target: 3000 },  // Sustain 3k
                { duration: '2m', target: 0 },     // Cool down
            ],
        },
        
        // Deep pagination stress test
        deep_pagination: {
            executor: 'constant-vus',
            startTime: '30m',
            vus: 500,
            duration: '10m',
        },
        
        // High concurrency burst
        burst_queries: {
            executor: 'ramping-arrival-rate',
            startTime: '42m',
            preAllocatedVUs: 1000,
            maxVUs: 5000,
            stages: [
                { duration: '1m', target: 5000 },  // 5k RPS
                { duration: '3m', target: 8000 },  // 8k RPS
                { duration: '2m', target: 12000 }, // 12k RPS burst
                { duration: '1m', target: 0 },     // Cool down
            ],
        }
    },
    
    thresholds: {
        http_req_duration: ['p(95)<3000', 'p(99)<8000'], // 95% under 3s, 99% under 8s
        http_req_failed: ['rate<0.03'],                   // Error rate under 3%
        event_query_success_rate: ['rate>0.97'],         // 97% success rate
        event_query_time: ['p(95)<2500'],                // 95% under 2.5s
        pagination_efficiency: ['p(90)<5000'],           // 90% of paginated queries under 5s
        http_reqs: ['rate>5000'],                        // Target 5k+ RPS sustained
    },
    
    setupTimeout: '5m',
    teardownTimeout: '2m',
    noConnectionReuse: false,
    userAgent: 'k6-mass-get-events/1.0',
};

// Pre-generated user IDs and pagination configs
let testConfig = {};

export function setup() {
    console.log('🚀 Starting Mass GET Events Test Setup...');
    
    waitForService();
    
    // Generate user IDs for testing
    console.log('📝 Generating test user IDs...');
    const userIds = [];
    for (let i = 0; i < 500; i++) {
        userIds.push(generateUuid());
    }
    
    // Generate pagination configurations
    console.log('📄 Setting up pagination test scenarios...');
    const paginationScenarios = [
        // Small pages - high frequency
        { limit: 10, maxPages: 100 },
        { limit: 20, maxPages: 50 },
        { limit: 50, maxPages: 20 },
        
        // Medium pages
        { limit: 100, maxPages: 100 },
        { limit: 200, maxPages: 50 },
        { limit: 500, maxPages: 20 },
        
        // Large pages - stress test
        { limit: 1000, maxPages: 10 },
        { limit: 2000, maxPages: 5 },
        { limit: 5000, maxPages: 2 },
        
        // Edge cases
        { limit: 1, maxPages: 10 },     // Extreme pagination
        { limit: 10000, maxPages: 1 },  // Single huge page
    ];
    
    console.log('✅ Setup complete');
    return { userIds, paginationScenarios };
}

export default function(data) {
    const scenario = __ITER % 10; // Vary test scenarios
    
    if (scenario < 6) {
        // 60% - Basic event listing with pagination
        testBasicPagination(data);
    } else if (scenario < 8) {
        // 20% - User-specific event queries
        testUserEventsPagination(data);
    } else if (scenario < 9) {
        // 10% - Deep pagination testing
        testDeepPagination(data);
    } else {
        // 10% - Large page size testing
        testLargePageSizes(data);
    }
}

function testBasicPagination(data) {
    const scenario = data.paginationScenarios[Math.floor(Math.random() * data.paginationScenarios.length)];
    const page = Math.floor(Math.random() * scenario.maxPages) + 1;
    
    const url = `${BASE_URL}/api/events?limit=${scenario.limit}&offset=${(page - 1) * scenario.limit}`;
    
    const response = http.get(url, {
        headers: defaultHeaders,
        timeout: '15s',
    });
    
    eventQueryTime.add(response.timings.duration);
    
    const success = verifyResponse(response, 200, 'basic pagination');
    eventQueryRate.add(success);
    
    if (success) {
        eventQueryThroughput.add(1);
        
        // Check pagination response structure
        check(response, {
            'has events array': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return Array.isArray(body.events);
                } catch (e) {
                    return false;
                }
            },
            'has pagination metadata': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return body.total !== undefined && body.limit !== undefined;
                } catch (e) {
                    return false;
                }
            },
            'page size within limits': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return body.events.length <= scenario.limit;
                } catch (e) {
                    return false;
                }
            },
        });
        
        paginationEfficiency.add(response.timings.duration);
    }
    
    sleep(0.05); // 50ms delay for basic queries
}

function testUserEventsPagination(data) {
    const userId = data.userIds[Math.floor(Math.random() * data.userIds.length)];
    const scenario = data.paginationScenarios[Math.floor(Math.random() * 6)]; // Use smaller scenarios for user queries
    const page = Math.floor(Math.random() * Math.min(scenario.maxPages, 20)) + 1;
    
    const url = `${BASE_URL}/api/users/${userId}/events?limit=${scenario.limit}&offset=${(page - 1) * scenario.limit}`;
    
    const response = http.get(url, {
        headers: defaultHeaders,
        timeout: '10s',
    });
    
    eventQueryTime.add(response.timings.duration);
    
    const success = verifyResponse(response, 200, 'user events pagination');
    eventQueryRate.add(success);
    
    if (success) {
        eventQueryThroughput.add(1);
        
        check(response, {
            'user events has correct structure': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return Array.isArray(body.events) && body.user_id === userId;
                } catch (e) {
                    return false;
                }
            },
        });
    }
    
    sleep(0.03); // 30ms delay for user queries
}

function testDeepPagination(data) {
    // Test very deep pagination - simulating users browsing far into results
    const scenario = { limit: 50, maxPages: 500 };
    const deepPage = Math.floor(Math.random() * 450) + 50; // Pages 50-500
    
    const url = `${BASE_URL}/api/events?limit=${scenario.limit}&offset=${(deepPage - 1) * scenario.limit}`;
    
    const startTime = Date.now();
    const response = http.get(url, {
        headers: defaultHeaders,
        timeout: '20s',
    });
    const endTime = Date.now();
    
    eventQueryTime.add(response.timings.duration);
    
    const success = verifyResponse(response, 200, 'deep pagination');
    eventQueryRate.add(success);
    
    if (success) {
        eventQueryThroughput.add(1);
        paginationEfficiency.add(endTime - startTime);
        
        check(response, {
            'deep pagination responds within 10s': (r) => r.timings.duration < 10000,
            'deep pagination has results': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return body.events !== undefined;
                } catch (e) {
                    return false;
                }
            },
        });
    }
    
    sleep(0.1); // 100ms delay for deep pagination
}

function testLargePageSizes(data) {
    // Test large page sizes to stress memory and response times
    const largeScenarios = [
        { limit: 5000, maxPages: 2 },
        { limit: 10000, maxPages: 1 },
        { limit: 2500, maxPages: 4 },
    ];
    
    const scenario = largeScenarios[Math.floor(Math.random() * largeScenarios.length)];
    const page = Math.floor(Math.random() * scenario.maxPages) + 1;
    
    const url = `${BASE_URL}/api/events?limit=${scenario.limit}&offset=${(page - 1) * scenario.limit}`;
    
    const response = http.get(url, {
        headers: defaultHeaders,
        timeout: '30s', // Longer timeout for large pages
    });
    
    eventQueryTime.add(response.timings.duration);
    
    const success = verifyResponse(response, 200, 'large page size');
    eventQueryRate.add(success);
    
    if (success) {
        eventQueryThroughput.add(1);
        
        check(response, {
            'large page responds within 15s': (r) => r.timings.duration < 15000,
            'large page has expected size': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return body.events.length <= scenario.limit;
                } catch (e) {
                    return false;
                }
            },
            'large page response size reasonable': (r) => r.body.length < 50 * 1024 * 1024, // Under 50MB
        });
    }
    
    sleep(0.2); // 200ms delay for large page queries
}

export function teardown(data) {
    console.log('🧹 Mass GET Events Test Teardown...');
    console.log(`📊 Test completed with ${eventQueryThroughput.count} queries executed`);
    
    // Final health check
    const healthResponse = http.get(`${BASE_URL}/health`, { headers: defaultHeaders });
    console.log(`🏥 Final health check: ${healthResponse.status}`);
    
    // Get final database stats
    const statsResponse = http.get(`${BASE_URL}/api/analytics/stats`, { headers: defaultHeaders });
    if (statsResponse.status === 200) {
        try {
            const stats = JSON.parse(statsResponse.body);
            console.log('📈 Final event count:', stats.total_events || 'unknown');
            console.log('📈 Final user count:', stats.total_users || 'unknown');
        } catch (e) {
            console.log('📊 Could not parse final stats');
        }
    }
}

// Additional test configurations
export const testProfiles = {
    smoke: {
        scenarios: {
            smoke_pagination: {
                executor: 'constant-vus',
                vus: 5,
                duration: '2m',
            }
        }
    },
    
    load: {
        scenarios: {
            load_pagination: {
                executor: 'ramping-vus',
                startVUs: 0,
                stages: [
                    { duration: '2m', target: 100 },
                    { duration: '5m', target: 500 },
                    { duration: '2m', target: 0 },
                ],
            }
        }
    },
    
    stress: {
        scenarios: {
            stress_pagination: {
                executor: 'ramping-arrival-rate',
                preAllocatedVUs: 500,
                maxVUs: 2000,
                stages: [
                    { duration: '2m', target: 2000 },
                    { duration: '5m', target: 8000 },
                    { duration: '5m', target: 15000 },
                    { duration: '2m', target: 0 },
                ],
            }
        }
    }
};