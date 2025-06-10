// Seed 10 Million Events - Extreme seeding performance test
import { check, sleep } from 'k6';
import http from 'k6/http';
import { Rate, Counter, Trend, Gauge } from 'k6/metrics';
import { generateEvent, generateUser, generateUuid, defaultHeaders, BASE_URL, waitForService, verifyResponse } from './setup.js';

// Custom metrics for seeding performance
const seedingRate = new Rate('seeding_success_rate');
const seedingThroughput = new Counter('events_seeded_total');
const seedingLatency = new Trend('seeding_latency');
const batchEfficiency = new Trend('batch_efficiency');
const memoryUsage = new Gauge('estimated_memory_usage_mb');
const databaseLoad = new Trend('database_load_time');

// Target: 10 million events
const TARGET_EVENTS = 10_000_000;
const BATCH_SIZE = 1000;           // Events per batch
const CONCURRENT_WORKERS = 200;    // Number of concurrent seeding workers
const USERS_COUNT = 10000;         // Number of unique users

export const options = {
    scenarios: {
        // Phase 1: Create users
        user_creation: {
            executor: 'shared-iterations',
            vus: 50,
            iterations: USERS_COUNT / 50, // Each VU creates 200 users
            maxDuration: '15m',
        },
        
        // Phase 2: Massive event seeding
        event_seeding: {
            executor: 'constant-vus',
            startTime: '16m',
            vus: CONCURRENT_WORKERS,
            duration: '180m', // 3 hours for 10M events
        },
        
        // Phase 3: Verification and monitoring
        monitoring: {
            executor: 'constant-vus',
            startTime: '17m',
            vus: 5,
            duration: '179m',
        }
    },
    
    thresholds: {
        http_req_duration: ['p(95)<5000', 'p(99)<10000'], // Allow longer for batch operations
        http_req_failed: ['rate<0.01'],                    // Very low error rate for seeding
        seeding_success_rate: ['rate>0.99'],              // 99% success rate
        seeding_latency: ['p(95)<3000'],                  // 95% under 3s
        events_seeded_total: [`count>${TARGET_EVENTS * 0.95}`], // At least 95% of target
        http_reqs: ['rate>500'],                          // Sustained seeding rate
    },
    
    setupTimeout: '10m',
    teardownTimeout: '10m',
    userAgent: 'k6-seed-10million/1.0',
    noConnectionReuse: false,
    
    // Resource management
    discardResponseBodies: true, // Save memory during seeding
};

let createdUsers = [];
let seedingProgress = 0;
let startTime = 0;

export function setup() {
    console.log('🚀 Starting 10 Million Events Seeding Setup...');
    console.log(`🎯 Target: ${TARGET_EVENTS.toLocaleString()} events`);
    console.log(`👥 Users: ${USERS_COUNT.toLocaleString()}`);
    console.log(`📦 Batch size: ${BATCH_SIZE}`);
    console.log(`⚡ Concurrent workers: ${CONCURRENT_WORKERS}`);
    
    waitForService();
    
    startTime = Date.now();
    
    console.log('✅ 10M seeding setup complete - users will be created in Phase 1');
    return { startTime };
}

export default function(data) {
    const scenario = __ENV.K6_SCENARIO_NAME;
    
    if (scenario === 'user_creation') {
        createUsersPhase(data);
    } else if (scenario === 'event_seeding') {
        seedEventsPhase(data);
    } else if (scenario === 'monitoring') {
        monitoringPhase(data);
    }
}

function createUsersPhase(data) {
    // Create batch of users
    const batchSize = 50;
    const users = [];
    
    for (let i = 0; i < batchSize; i++) {
        const user = generateUser();
        
        const response = http.post(`${BASE_URL}/api/users`, JSON.stringify(user), {
            headers: defaultHeaders,
            timeout: '8s',
        });
        
        if (response.status === 201 || response.status === 200) {
            users.push(user);
            createdUsers.push(user.id);
        }
        
        seedingRate.add(response.status === 201 || response.status === 200);
    }
    
    console.log(`👥 Created ${users.length} users (total: ${createdUsers.length})`);
    sleep(0.1);
}

function seedEventsPhase(data) {
    // Massive event creation phase
    if (createdUsers.length === 0) {
        // Get users from database if not available
        const usersResponse = http.get(`${BASE_URL}/api/users?limit=1000`, { headers: defaultHeaders });
        if (usersResponse.status === 200) {
            try {
                const body = JSON.parse(usersResponse.body);
                createdUsers = body.users.map(u => u.id);
            } catch (e) {
                sleep(1);
                return;
            }
        }
    }
    
    if (createdUsers.length === 0) {
        sleep(1);
        return;
    }
    
    // Create batch of events
    const batchStart = Date.now();
    const events = [];
    
    for (let i = 0; i < BATCH_SIZE; i++) {
        const userId = createdUsers[Math.floor(Math.random() * createdUsers.length)];
        const event = generateEvent(userId);
        
        // Add variety to events for realistic load
        const eventTypes = [
            'user_action', 'page_view', 'click', 'purchase', 'login', 
            'logout', 'scroll', 'hover', 'submit', 'navigation'
        ];
        event.event_type = eventTypes[Math.floor(Math.random() * eventTypes.length)];
        
        // Distribute events across time for realism
        const now = new Date();
        const dayAgo = new Date(now.getTime() - (24 * 60 * 60 * 1000));
        const randomTime = new Date(dayAgo.getTime() + Math.random() * (now.getTime() - dayAgo.getTime()));
        event.timestamp = randomTime.toISOString();
        
        events.push(event);
    }
    
    // Send batch of events
    let successCount = 0;
    const promises = [];
    
    for (const event of events) {
        const response = http.post(`${BASE_URL}/api/events`, JSON.stringify(event), {
            headers: defaultHeaders,
            timeout: '8s',
        });
        
        const success = response.status === 201;
        seedingRate.add(success);
        
        if (success) {
            successCount++;
            seedingThroughput.add(1);
        }
        
        seedingLatency.add(response.timings.duration);
    }
    
    const batchTime = Date.now() - batchStart;
    batchEfficiency.add(batchTime);
    
    // Update progress
    seedingProgress += successCount;
    
    // Log progress every 10k events
    if (seedingProgress % 10000 < BATCH_SIZE) {
        const elapsed = (Date.now() - data.startTime) / 1000 / 60; // minutes
        const rate = seedingProgress / elapsed; // events per minute
        const eta = (TARGET_EVENTS - seedingProgress) / rate; // minutes remaining
        
        console.log(`📈 Progress: ${seedingProgress.toLocaleString()}/${TARGET_EVENTS.toLocaleString()} events`);
        console.log(`⚡ Rate: ${Math.round(rate)} events/min, ETA: ${Math.round(eta)} min`);
        
        // Estimate memory usage
        const estimatedMemoryMB = (seedingProgress * 0.5) / 1024; // ~0.5KB per event
        memoryUsage.set(estimatedMemoryMB);
    }
    
    // Adaptive delay based on success rate
    const recentSuccessRate = successCount / BATCH_SIZE;
    if (recentSuccessRate < 0.8) {
        sleep(0.5); // Slow down if error rate is high
    } else if (recentSuccessRate > 0.95) {
        sleep(0.01); // Speed up if doing well
    } else {
        sleep(0.1); // Normal rate
    }
}

function monitoringPhase(data) {
    // Monitor system health during seeding
    const response = http.get(`${BASE_URL}/health`, {
        headers: defaultHeaders,
        timeout: '5s',
    });
    
    check(response, {
        'system healthy during seeding': (r) => r.status === 200,
        'health check fast during load': (r) => r.timings.duration < 2000,
    });
    
    // Get database stats periodically
    const statsResponse = http.get(`${BASE_URL}/api/analytics/stats`, {
        headers: defaultHeaders,
        timeout: '10s',
    });
    
    if (statsResponse.status === 200) {
        databaseLoad.add(statsResponse.timings.duration);
        
        try {
            const stats = JSON.parse(statsResponse.body);
            const currentCount = stats.total_events || 0;
            
            console.log(`📊 Database reports ${currentCount.toLocaleString()} total events`);
            
            // Check if we're hitting our target
            if (currentCount >= TARGET_EVENTS) {
                console.log('🎉 TARGET ACHIEVED! 10 million events seeded successfully!');
            }
        } catch (e) {
            // Continue monitoring
        }
    }
    
    sleep(30); // Monitor every 30 seconds
}

export function teardown(data) {
    console.log('🧹 10 Million Events Seeding Teardown...');
    
    const totalTime = (Date.now() - data.startTime) / 1000 / 60; // minutes
    const finalRate = seedingThroughput.count / totalTime;
    
    console.log(`📊 Seeding Summary:`);
    console.log(`  • Events created: ${seedingThroughput.count.toLocaleString()}`);
    console.log(`  • Total time: ${Math.round(totalTime)} minutes`);
    console.log(`  • Average rate: ${Math.round(finalRate)} events/min`);
    console.log(`  • Target achievement: ${((seedingThroughput.count / TARGET_EVENTS) * 100).toFixed(1)}%`);
    
    // Final verification
    console.log('🔍 Final verification...');
    const statsResponse = http.get(`${BASE_URL}/api/analytics/stats`, { headers: defaultHeaders });
    if (statsResponse.status === 200) {
        try {
            const stats = JSON.parse(statsResponse.body);
            console.log(`✅ Database confirms ${stats.total_events?.toLocaleString()} events`);
            console.log(`✅ Database confirms ${stats.total_users?.toLocaleString()} users`);
            
            if (stats.total_events >= TARGET_EVENTS) {
                console.log('🎯 SUCCESS: 10 million event target achieved!');
            } else {
                console.log(`⚠️ Partial success: ${((stats.total_events / TARGET_EVENTS) * 100).toFixed(1)}% of target`);
            }
        } catch (e) {
            console.log('❌ Could not verify final counts');
        }
    }
    
    // Health check
    const healthResponse = http.get(`${BASE_URL}/health`, { headers: defaultHeaders });
    console.log(`🏥 Final health check: ${healthResponse.status}`);
    
    // Performance summary
    console.log('⚡ Performance Summary:');
    console.log(`  • Peak seeding rate: ${Math.round(finalRate)} events/min`);
    console.log(`  • Database load time: ${databaseLoad.avg?.toFixed(0) || 'N/A'}ms avg`);
    console.log(`  • Batch efficiency: ${batchEfficiency.avg?.toFixed(0) || 'N/A'}ms avg`);
    console.log(`  • Success rate: ${(seedingRate.rate * 100).toFixed(2)}%`);
}

// Additional configurations for different scenarios
export const testProfiles = {
    // Quick validation test
    smoke: {
        scenarios: {
            smoke_seed: {
                executor: 'constant-vus',
                vus: 5,
                duration: '5m',
            }
        },
        TARGET_EVENTS: 1000,
    },
    
    // Medium scale test
    load: {
        scenarios: {
            load_seed: {
                executor: 'constant-vus',
                vus: 20,
                duration: '30m',
            }
        },
        TARGET_EVENTS: 100_000,
    },
    
    // Full 10M stress test
    stress: {
        scenarios: {
            stress_seed: {
                executor: 'constant-vus',
                vus: 500,
                duration: '240m', // 4 hours
            }
        },
        TARGET_EVENTS: 10_000_000,
    }
};