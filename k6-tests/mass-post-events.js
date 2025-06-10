// Mass POST /events - Stress test event creation with 10k+ RPS target
import { check, sleep } from 'k6';
import http from 'k6/http';
import { Rate, Counter, Trend } from 'k6/metrics';
import { generateEvent, generateUuid, defaultHeaders, BASE_URL, waitForService, verifyResponse } from './setup.js';

// Custom metrics
const eventCreationRate = new Rate('event_creation_success_rate');
const eventCreationTime = new Trend('event_creation_time');
const eventCreationThroughput = new Counter('events_created_total');

// Test configuration for different load levels
export const options = {
    scenarios: {
        // Ramp up test - gradual increase to 10k RPS
        ramp_up: {
            executor: 'ramping-vus',
            startVUs: 0,
            stages: [
                { duration: '2m', target: 100 },   // Warm up
                { duration: '5m', target: 1000 },  // 1k RPS
                { duration: '5m', target: 2500 },  // 2.5k RPS
                { duration: '5m', target: 5000 },  // 5k RPS
                { duration: '10m', target: 10000 }, // 10k RPS target
                { duration: '5m', target: 10000 },  // Sustain 10k RPS
                { duration: '2m', target: 0 },     // Cool down
            ],
        },
        
        // Spike test - sudden burst
        spike_test: {
            executor: 'ramping-vus',
            startTime: '35m',
            startVUs: 0,
            stages: [
                { duration: '30s', target: 15000 }, // Spike to 15k RPS
                { duration: '2m', target: 15000 },  // Hold spike
                { duration: '30s', target: 0 },     // Drop
            ],
        },
        
        // Sustained high load
        sustained_load: {
            executor: 'constant-vus',
            startTime: '40m',
            vus: 8000,
            duration: '15m',
        }
    },
    
    thresholds: {
        http_req_duration: ['p(95)<2000', 'p(99)<5000'], // 95% under 2s, 99% under 5s
        http_req_failed: ['rate<0.05'],                   // Error rate under 5%
        event_creation_success_rate: ['rate>0.95'],      // 95% success rate
        event_creation_time: ['p(95)<1500'],             // 95% under 1.5s
        http_reqs: ['rate>8000'],                        // Target 8k+ RPS sustained
    },
    
    // Resource limits
    noConnectionReuse: false,
    userAgent: 'k6-mass-post-events/1.0',
    insecureSkipTLSVerify: true,
    
    // Test duration and setup
    setupTimeout: '5m',
    teardownTimeout: '2m',
};

// Pre-generated user IDs for consistent testing
let userIds = [];

export function setup() {
    console.log('🚀 Starting Mass POST Events Test Setup...');
    
    // Wait for service
    waitForService();
    
    // Generate user IDs for testing
    console.log('📝 Generating user IDs...');
    for (let i = 0; i < 1000; i++) {
        userIds.push(generateUuid());
    }
    
    console.log('✅ Setup complete');
    return { userIds };
}

export default function(data) {
    // Select random user ID from pre-generated list
    const userId = data.userIds[Math.floor(Math.random() * data.userIds.length)];
    
    // Generate event data
    const eventData = generateEvent(userId);
    
    // Add some variation to event types for more realistic load
    const eventTypes = ['user_action', 'page_view', 'click', 'purchase', 'login', 'logout', 'scroll', 'hover'];
    eventData.event_type = eventTypes[Math.floor(Math.random() * eventTypes.length)];
    
    // Add timestamp variation (within last hour)
    const now = new Date();
    const hourAgo = new Date(now.getTime() - (60 * 60 * 1000));
    const randomTime = new Date(hourAgo.getTime() + Math.random() * (now.getTime() - hourAgo.getTime()));
    eventData.timestamp = randomTime.toISOString();
    
    // Make the request
    const response = http.post(
        `${BASE_URL}/api/events`,
        JSON.stringify(eventData),
        {
            headers: defaultHeaders,
            timeout: '10s',
        }
    );
    
    // Record metrics
    eventCreationTime.add(response.timings.duration);
    
    // Verify response
    const success = verifyResponse(response, 201, 'event creation');
    eventCreationRate.add(success);
    
    if (success) {
        eventCreationThroughput.add(1);
    }
    
    // Additional checks for high load
    check(response, {
        'event creation status is 201': (r) => r.status === 201,
        'event creation time < 3000ms': (r) => r.timings.duration < 3000,
        'response has event ID': (r) => {
            try {
                const body = JSON.parse(r.body);
                return body.id !== undefined;
            } catch (e) {
                return false;
            }
        },
    });
    
    // Brief pause to prevent overwhelming (adjust based on target RPS)
    // For 10k RPS with varying VUs, this will be dynamically adjusted
    sleep(0.01); // 10ms base delay
}

export function teardown(data) {
    console.log('🧹 Mass POST Events Test Teardown...');
    console.log(`📊 Test completed with ${eventCreationThroughput.count} events created`);
    
    // Get final stats
    const healthResponse = http.get(`${BASE_URL}/health`, { headers: defaultHeaders });
    console.log(`🏥 Final health check: ${healthResponse.status}`);
    
    // Optional: Get analytics after load test
    const statsResponse = http.get(`${BASE_URL}/api/analytics/stats`, { headers: defaultHeaders });
    if (statsResponse.status === 200) {
        console.log('📈 Final analytics stats:', statsResponse.body);
    }
}

// Configuration for different test profiles
export const testProfiles = {
    // Quick validation test
    smoke: {
        scenarios: {
            smoke_test: {
                executor: 'constant-vus',
                vus: 10,
                duration: '1m',
            }
        }
    },
    
    // Medium load test  
    load: {
        scenarios: {
            load_test: {
                executor: 'ramping-vus',
                startVUs: 0,
                stages: [
                    { duration: '2m', target: 100 },
                    { duration: '5m', target: 1000 },
                    { duration: '2m', target: 0 },
                ],
            }
        }
    },
    
    // Stress test pushing limits
    stress: {
        scenarios: {
            stress_test: {
                executor: 'ramping-vus',
                startVUs: 0,
                stages: [
                    { duration: '2m', target: 1000 },
                    { duration: '5m', target: 5000 },
                    { duration: '10m', target: 12000 },
                    { duration: '5m', target: 15000 },
                    { duration: '2m', target: 0 },
                ],
            }
        }
    }
};