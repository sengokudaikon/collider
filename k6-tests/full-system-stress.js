// Full System Stress Test - Combined load targeting 10k+ RPS
import { check, sleep } from 'k6';
import http from 'k6/http';
import { Rate, Counter, Trend, Gauge } from 'k6/metrics';
import { generateEvent, generateUser, generateUuid, defaultHeaders, BASE_URL, waitForService, verifyResponse } from './setup.js';

// System-wide metrics
const systemThroughput = new Counter('system_throughput_rps');
const systemLatency = new Trend('system_latency');
const systemErrorRate = new Rate('system_error_rate');
const concurrentUsers = new Gauge('concurrent_users');
const memoryPressure = new Gauge('memory_pressure_indicator');
const databaseConnections = new Gauge('estimated_db_connections');

// Operation-specific metrics
const createEventMetrics = new Counter('create_events_count');
const queryEventMetrics = new Counter('query_events_count');
const analyticsMetrics = new Counter('analytics_queries_count');
const deleteMetrics = new Counter('delete_operations_count');

export const options = {
    scenarios: {
        // Event creation load - 40% of traffic
        event_creation: {
            executor: 'ramping-arrival-rate',
            startRate: 0,
            timeUnit: '1s',
            preAllocatedVUs: 1000,
            maxVUs: 3000,
            stages: [
                { duration: '5m', target: 2000 },   // Ramp to 2k RPS
                { duration: '10m', target: 4000 },  // 4k RPS
                { duration: '15m', target: 6000 },  // 6k RPS - sustained load
                { duration: '10m', target: 8000 },  // 8k RPS - stress
                { duration: '5m', target: 10000 },  // 10k RPS - peak
                { duration: '10m', target: 10000 }, // Sustain peak
                { duration: '5m', target: 0 },      // Cool down
            ],
        },
        
        // Event querying load - 35% of traffic
        event_querying: {
            executor: 'ramping-arrival-rate',
            startTime: '1m',
            startRate: 0,
            timeUnit: '1s',
            preAllocatedVUs: 800,
            maxVUs: 2500,
            stages: [
                { duration: '5m', target: 1500 },   // 1.5k RPS
                { duration: '10m', target: 3000 },  // 3k RPS
                { duration: '15m', target: 4500 },  // 4.5k RPS
                { duration: '10m', target: 6000 },  // 6k RPS
                { duration: '5m', target: 8000 },   // 8k RPS peak
                { duration: '10m', target: 8000 },  // Sustain
                { duration: '4m', target: 0 },      // Cool down
            ],
        },
        
        // Analytics load - 20% of traffic
        analytics_load: {
            executor: 'ramping-arrival-rate',
            startTime: '2m',
            startRate: 0,
            timeUnit: '1s',
            preAllocatedVUs: 400,
            maxVUs: 1000,
            stages: [
                { duration: '5m', target: 500 },    // 500 RPS
                { duration: '10m', target: 1000 },  // 1k RPS
                { duration: '15m', target: 1500 },  // 1.5k RPS
                { duration: '10m', target: 2000 },  // 2k RPS
                { duration: '5m', target: 2500 },   // 2.5k RPS peak
                { duration: '10m', target: 2500 },  // Sustain
                { duration: '5m', target: 0 },      // Cool down
            ],
        },
        
        // Deletion operations - 5% of traffic
        deletion_load: {
            executor: 'ramping-arrival-rate',
            startTime: '3m',
            startRate: 0,
            timeUnit: '1s',
            preAllocatedVUs: 100,
            maxVUs: 300,
            stages: [
                { duration: '5m', target: 100 },    // 100 RPS
                { duration: '10m', target: 200 },   // 200 RPS
                { duration: '15m', target: 300 },   // 300 RPS
                { duration: '10m', target: 500 },   // 500 RPS
                { duration: '5m', target: 750 },    // 750 RPS peak
                { duration: '10m', target: 750 },   // Sustain
                { duration: '5m', target: 0 },      // Cool down
            ],
        },
        
        // System monitoring
        system_monitor: {
            executor: 'constant-vus',
            startTime: '30s',
            vus: 3,
            duration: '59m',
        }
    },
    
    thresholds: {
        // System-wide thresholds for 10k+ RPS
        http_req_duration: ['p(95)<3000', 'p(99)<8000'], // Response times under load
        http_req_failed: ['rate<0.05'],                   // Error rate under 5%
        system_error_rate: ['rate<0.03'],                // System error rate under 3%
        system_latency: ['p(95)<2500'],                  // System latency under 2.5s
        system_throughput_rps: ['count>600000'],         // Total requests > 600k (10k RPS * 60s)
        
        // Operation-specific thresholds
        create_events_count: ['count>200000'],           // 200k+ event creations
        query_events_count: ['count>150000'],            // 150k+ event queries
        analytics_queries_count: ['count>50000'],        // 50k+ analytics queries
        delete_operations_count: ['count>20000'],        // 20k+ delete operations
    },
    
    setupTimeout: '10m',
    teardownTimeout: '10m',
    userAgent: 'k6-full-system-stress/1.0',
    
    // Resource optimization for high load
    noConnectionReuse: false,
    noVUConnectionReuse: false,
    batch: 1,
    batchPerHost: 6,
};

let systemUsers = [];
let systemEvents = [];
let testStartTime = 0;

export function setup() {
    console.log('🚀 Starting Full System Stress Test...');
    console.log('🎯 Target: 10,000+ RPS sustained load');
    console.log('📊 Mixed workload: 40% creates, 35% queries, 20% analytics, 5% deletes');
    
    waitForService();
    
    testStartTime = Date.now();
    
    // Pre-populate some data for realistic testing
    console.log('📝 Pre-populating test data...');
    
    // Create initial users
    for (let i = 0; i < 1000; i++) {
        const user = generateUser();
        const response = http.post(`${BASE_URL}/api/users`, JSON.stringify(user), { headers: defaultHeaders });
        if (response.status === 201) {
            systemUsers.push(user.id);
        }
    }
    
    // Create initial events
    for (let i = 0; i < 5000; i++) {
        const userId = systemUsers[Math.floor(Math.random() * systemUsers.length)];
        const event = generateEvent(userId);
        const response = http.post(`${BASE_URL}/api/events`, JSON.stringify(event), { headers: defaultHeaders });
        if (response.status === 201) {
            try {
                const body = JSON.parse(response.body);
                systemEvents.push(body.id);
            } catch (e) {
                // Continue
            }
        }
    }
    
    console.log(`✅ Setup complete: ${systemUsers.length} users, ${systemEvents.length} events`);
    return { systemUsers, systemEvents, testStartTime };
}

export default function(data) {
    const scenario = __ENV.K6_SCENARIO_NAME;
    
    switch (scenario) {
        case 'event_creation':
            handleEventCreation(data);
            break;
        case 'event_querying':
            handleEventQuerying(data);
            break;
        case 'analytics_load':
            handleAnalyticsLoad(data);
            break;
        case 'deletion_load':
            handleDeletionLoad(data);
            break;
        case 'system_monitor':
            handleSystemMonitoring(data);
            break;
        default:
            handleEventCreation(data); // Fallback
    }
}

function handleEventCreation(data) {
    const userId = data.systemUsers[Math.floor(Math.random() * data.systemUsers.length)];
    const event = generateEvent(userId);
    
    // Add variety for realistic load
    const eventTypes = ['user_action', 'page_view', 'click', 'purchase', 'login', 'logout'];
    event.event_type = eventTypes[Math.floor(Math.random() * eventTypes.length)];
    
    const response = http.post(`${BASE_URL}/api/events`, JSON.stringify(event), {
        headers: defaultHeaders,
        timeout: '8s',
    });
    
    const success = response.status === 201;
    systemErrorRate.add(!success);
    systemLatency.add(response.timings.duration);
    systemThroughput.add(1);
    
    if (success) {
        createEventMetrics.add(1);
        
        // Store event ID for deletion tests
        try {
            const body = JSON.parse(response.body);
            if (body.id && data.systemEvents.length < 10000) {
                data.systemEvents.push(body.id);
            }
        } catch (e) {
            // Continue
        }
    }
    
    check(response, {
        'event creation success': (r) => r.status === 201,
        'event creation latency ok': (r) => r.timings.duration < 5000,
    });
}

function handleEventQuerying(data) {
    const queryType = Math.random();
    let response;
    
    if (queryType < 0.4) {
        // 40% - Basic event listing with pagination
        const limit = [10, 20, 50, 100][Math.floor(Math.random() * 4)];
        const offset = Math.floor(Math.random() * 1000);
        response = http.get(`${BASE_URL}/api/events?limit=${limit}&offset=${offset}`, {
            headers: defaultHeaders,
            timeout: '10s',
        });
    } else if (queryType < 0.7) {
        // 30% - User-specific events
        const userId = data.systemUsers[Math.floor(Math.random() * data.systemUsers.length)];
        response = http.get(`${BASE_URL}/api/users/${userId}/events?limit=50`, {
            headers: defaultHeaders,
            timeout: '8s',
        });
    } else {
        // 30% - Single event lookup
        if (data.systemEvents.length > 0) {
            const eventId = data.systemEvents[Math.floor(Math.random() * data.systemEvents.length)];
            response = http.get(`${BASE_URL}/api/events/${eventId}`, {
                headers: defaultHeaders,
                timeout: '5s',
            });
        } else {
            // Fallback to list
            response = http.get(`${BASE_URL}/api/events?limit=20`, {
                headers: defaultHeaders,
                timeout: '10s',
            });
        }
    }
    
    const success = response.status === 200;
    systemErrorRate.add(!success);
    systemLatency.add(response.timings.duration);
    systemThroughput.add(1);
    
    if (success) {
        queryEventMetrics.add(1);
    }
    
    check(response, {
        'event query success': (r) => r.status === 200,
        'event query latency ok': (r) => r.timings.duration < 8000,
    });
}

function handleAnalyticsLoad(data) {
    const analyticsType = Math.random();
    let response;
    
    if (analyticsType < 0.3) {
        // 30% - General stats
        response = http.get(`${BASE_URL}/api/analytics/stats`, {
            headers: defaultHeaders,
            timeout: '12s',
        });
    } else if (analyticsType < 0.5) {
        // 20% - Real-time metrics
        response = http.get(`${BASE_URL}/api/analytics/metrics/realtime`, {
            headers: defaultHeaders,
            timeout: '5s',
        });
    } else if (analyticsType < 0.7) {
        // 20% - User analytics
        const userId = data.systemUsers[Math.floor(Math.random() * data.systemUsers.length)];
        response = http.get(`${BASE_URL}/api/users/${userId}/analytics`, {
            headers: defaultHeaders,
            timeout: '10s',
        });
    } else if (analyticsType < 0.85) {
        // 15% - Event type analytics
        response = http.get(`${BASE_URL}/api/analytics/events/by-type`, {
            headers: defaultHeaders,
            timeout: '8s',
        });
    } else {
        // 15% - Time-based analytics
        const ranges = ['last_hour', 'last_24_hours', 'last_7_days'];
        const range = ranges[Math.floor(Math.random() * ranges.length)];
        response = http.get(`${BASE_URL}/api/analytics/events/timeline?range=${range}`, {
            headers: defaultHeaders,
            timeout: '15s',
        });
    }
    
    const success = response.status === 200;
    systemErrorRate.add(!success);
    systemLatency.add(response.timings.duration);
    systemThroughput.add(1);
    
    if (success) {
        analyticsMetrics.add(1);
    }
    
    check(response, {
        'analytics query success': (r) => r.status === 200,
        'analytics query latency ok': (r) => r.timings.duration < 12000,
    });
}

function handleDeletionLoad(data) {
    const deletionType = Math.random();
    let response;
    
    if (deletionType < 0.6) {
        // 60% - Single event deletion
        if (data.systemEvents.length > 0) {
            const eventId = data.systemEvents.pop(); // Remove from list
            response = http.del(`${BASE_URL}/api/events/${eventId}`, null, {
                headers: defaultHeaders,
                timeout: '8s',
            });
        } else {
            sleep(0.1);
            return;
        }
    } else if (deletionType < 0.8) {
        // 20% - Bulk deletion by date
        const hoursAgo = Math.floor(Math.random() * 6) + 1;
        const beforeDate = new Date(Date.now() - (hoursAgo * 60 * 60 * 1000));
        response = http.del(`${BASE_URL}/api/events?before=${beforeDate.toISOString()}`, null, {
            headers: defaultHeaders,
            timeout: '15s',
        });
    } else {
        // 20% - Bulk deletion by user
        const userId = data.systemUsers[Math.floor(Math.random() * data.systemUsers.length)];
        response = http.del(`${BASE_URL}/api/users/${userId}/events`, null, {
            headers: defaultHeaders,
            timeout: '12s',
        });
    }
    
    const success = response.status === 204 || response.status === 200;
    systemErrorRate.add(!success);
    systemLatency.add(response.timings.duration);
    systemThroughput.add(1);
    
    if (success) {
        deleteMetrics.add(1);
    }
    
    check(response, {
        'deletion success': (r) => r.status === 204 || r.status === 200,
        'deletion latency ok': (r) => r.timings.duration < 10000,
    });
}

function handleSystemMonitoring(data) {
    // Monitor system health during stress test
    const response = http.get(`${BASE_URL}/health`, {
        headers: defaultHeaders,
        timeout: '5s',
    });
    
    check(response, {
        'system health check': (r) => r.status === 200,
        'health check latency': (r) => r.timings.duration < 2000,
    });
    
    // Update monitoring metrics
    concurrentUsers.set(__VU);
    
    // Estimate database connections (rough approximation)
    const estimatedConnections = Math.floor(__VU * 0.8); // Assume 80% connection utilization
    databaseConnections.set(estimatedConnections);
    
    // Memory pressure indicator (based on response times)
    const avgLatency = systemLatency.avg || 0;
    if (avgLatency > 3000) {
        memoryPressure.set(1); // High pressure
    } else if (avgLatency > 1500) {
        memoryPressure.set(0.5); // Medium pressure
    } else {
        memoryPressure.set(0); // Low pressure
    }
    
    // Get system stats periodically
    const statsResponse = http.get(`${BASE_URL}/api/analytics/stats`, {
        headers: defaultHeaders,
        timeout: '10s',
    });
    
    if (statsResponse.status === 200) {
        try {
            const stats = JSON.parse(statsResponse.body);
            console.log(`📊 System stats: ${stats.total_events} events, ${stats.total_users} users`);
        } catch (e) {
            // Continue
        }
    }
    
    sleep(10); // Monitor every 10 seconds
}

export function teardown(data) {
    console.log('🧹 Full System Stress Test Teardown...');
    
    const totalTime = (Date.now() - data.testStartTime) / 1000; // seconds
    const totalRequests = systemThroughput.count;
    const avgRPS = totalRequests / totalTime;
    
    console.log('📊 System Performance Summary:');
    console.log(`  • Total requests: ${totalRequests.toLocaleString()}`);
    console.log(`  • Total time: ${Math.round(totalTime)}s`);
    console.log(`  • Average RPS: ${Math.round(avgRPS)}`);
    console.log(`  • Peak concurrent users: ${concurrentUsers.value || 'N/A'}`);
    console.log(`  • System error rate: ${(systemErrorRate.rate * 100).toFixed(2)}%`);
    console.log(`  • Average latency: ${systemLatency.avg?.toFixed(0) || 'N/A'}ms`);
    
    console.log('📈 Operation Breakdown:');
    console.log(`  • Event creations: ${createEventMetrics.count.toLocaleString()}`);
    console.log(`  • Event queries: ${queryEventMetrics.count.toLocaleString()}`);
    console.log(`  • Analytics queries: ${analyticsMetrics.count.toLocaleString()}`);
    console.log(`  • Delete operations: ${deleteMetrics.count.toLocaleString()}`);
    
    // Final health check
    const healthResponse = http.get(`${BASE_URL}/health`, { headers: defaultHeaders });
    console.log(`🏥 Final health check: ${healthResponse.status} (${healthResponse.timings.duration}ms)`);
    
    // Success criteria evaluation
    if (avgRPS >= 10000) {
        console.log('🎯 SUCCESS: Achieved 10k+ RPS target!');
    } else if (avgRPS >= 8000) {
        console.log('✅ GOOD: Achieved 8k+ RPS (80% of target)');
    } else if (avgRPS >= 5000) {
        console.log('⚠️ PARTIAL: Achieved 5k+ RPS (50% of target)');
    } else {
        console.log('❌ NEEDS WORK: Below 5k RPS');
    }
    
    if (systemErrorRate.rate < 0.05) {
        console.log('✅ Error rate within acceptable limits (<5%)');
    } else {
        console.log(`❌ High error rate: ${(systemErrorRate.rate * 100).toFixed(2)}%`);
    }
}

export const testProfiles = {
    smoke: {
        scenarios: {
            smoke_system: {
                executor: 'constant-arrival-rate',
                rate: 100,
                timeUnit: '1s',
                duration: '5m',
                preAllocatedVUs: 50,
            }
        }
    },
    
    load: {
        scenarios: {
            load_system: {
                executor: 'ramping-arrival-rate',
                stages: [
                    { duration: '5m', target: 1000 },
                    { duration: '10m', target: 3000 },
                    { duration: '5m', target: 0 },
                ],
                preAllocatedVUs: 200,
                maxVUs: 1000,
            }
        }
    }
};