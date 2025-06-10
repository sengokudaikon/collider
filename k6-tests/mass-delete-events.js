// Mass Event Deletion - Stress test bulk deletion operations
import { check, sleep } from 'k6';
import http from 'k6/http';
import { Rate, Counter, Trend } from 'k6/metrics';
import { generateEvent, generateUuid, defaultHeaders, BASE_URL, waitForService, verifyResponse, seedUsers, seedEvents } from './setup.js';

// Custom metrics for deletion operations
const deletionSuccessRate = new Rate('deletion_success_rate');
const deletionTime = new Trend('deletion_time');
const deletionThroughput = new Counter('deletions_executed');
const bulkDeletionEfficiency = new Trend('bulk_deletion_efficiency');
const deletionCleanupTime = new Trend('deletion_cleanup_time');

export const options = {
    scenarios: {
        // Setup phase - create data to delete
        setup_data: {
            executor: 'shared-iterations',
            vus: 10,
            iterations: 100,
            maxDuration: '10m',
        },
        
        // Single event deletions
        single_deletions: {
            executor: 'ramping-vus',
            startTime: '12m',
            startVUs: 0,
            stages: [
                { duration: '2m', target: 50 },
                { duration: '5m', target: 200 },
                { duration: '3m', target: 200 },
                { duration: '2m', target: 0 },
            ],
        },
        
        // Bulk deletion stress test
        bulk_deletions: {
            executor: 'ramping-vus',
            startTime: '25m',
            startVUs: 0,
            stages: [
                { duration: '2m', target: 20 },
                { duration: '8m', target: 100 },
                { duration: '5m', target: 150 },
                { duration: '2m', target: 0 },
            ],
        },
        
        // Concurrent deletion chaos test
        chaos_deletions: {
            executor: 'constant-arrival-rate',
            startTime: '42m',
            rate: 200,
            timeUnit: '1s',
            duration: '8m',
            preAllocatedVUs: 50,
            maxVUs: 200,
        }
    },
    
    thresholds: {
        http_req_duration: ['p(95)<8000', 'p(99)<15000'], // Deletions can take longer
        http_req_failed: ['rate<0.05'],                    // Error rate under 5%
        deletion_success_rate: ['rate>0.95'],             // 95% deletion success
        deletion_time: ['p(95)<5000'],                     // 95% under 5s
        bulk_deletion_efficiency: ['p(90)<12000'],         // Bulk operations under 12s
        http_reqs: ['rate>100'],                          // Target 100+ deletion RPS
    },
    
    setupTimeout: '15m',
    teardownTimeout: '5m',
    userAgent: 'k6-mass-delete-events/1.0',
};

let createdEventIds = [];
let createdUserIds = [];

export function setup() {
    console.log('🚀 Starting Mass Delete Events Test Setup...');
    
    waitForService();
    
    // Create users for event creation
    console.log('👥 Creating test users...');
    const users = seedUsers(50);
    createdUserIds = users.map(u => u.id);
    
    // Create events to delete
    console.log('📝 Creating events for deletion testing...');
    const eventsCreated = seedEvents(users, 200); // 50 users * 200 events = 10k events
    
    // Get some event IDs for single deletion tests
    console.log('🔍 Fetching event IDs for deletion tests...');
    const eventsResponse = http.get(`${BASE_URL}/api/events?limit=1000`, { headers: defaultHeaders });
    if (eventsResponse.status === 200) {
        try {
            const body = JSON.parse(eventsResponse.body);
            createdEventIds = body.events.map(e => e.id);
            console.log(`✅ Retrieved ${createdEventIds.length} event IDs for testing`);
        } catch (e) {
            console.log('⚠️ Could not parse events response for IDs');
        }
    }
    
    console.log('✅ Mass deletion setup complete');
    return { createdEventIds, createdUserIds, eventsCreated };
}

export default function(data) {
    const scenario = __ITER % 10;
    
    if (scenario < 4) {
        // 40% - Single event deletions
        testSingleEventDeletion(data);
    } else if (scenario < 7) {
        // 30% - Bulk deletion by date range
        testBulkDeletionByDate(data);
    } else if (scenario < 8) {
        // 10% - Bulk deletion by user
        testBulkDeletionByUser(data);
    } else if (scenario < 9) {
        // 10% - Bulk deletion by event type
        testBulkDeletionByEventType(data);
    } else {
        // 10% - Create more events for continuous testing
        createEventsForDeletion(data);
    }
}

function testSingleEventDeletion(data) {
    if (data.createdEventIds.length === 0) {
        sleep(0.1);
        return;
    }
    
    const eventId = data.createdEventIds[Math.floor(Math.random() * data.createdEventIds.length)];
    
    const response = http.del(`${BASE_URL}/api/events/${eventId}`, null, {
        headers: defaultHeaders,
        timeout: '8s',
    });
    
    deletionTime.add(response.timings.duration);
    
    const success = verifyResponse(response, 204, 'single event deletion');
    deletionSuccessRate.add(success);
    
    if (success) {
        deletionThroughput.add(1);
        
        check(response, {
            'single deletion status 204': (r) => r.status === 204,
            'single deletion time < 3s': (r) => r.timings.duration < 3000,
        });
        
        // Remove from our list to avoid double deletion
        const index = data.createdEventIds.indexOf(eventId);
        if (index > -1) {
            data.createdEventIds.splice(index, 1);
        }
    }
    
    sleep(0.05);
}

function testBulkDeletionByDate(data) {
    // Delete events older than X hours
    const hoursAgo = Math.floor(Math.random() * 12) + 1; // 1-12 hours ago
    const beforeDate = new Date(Date.now() - (hoursAgo * 60 * 60 * 1000));
    
    const url = `${BASE_URL}/api/events?before=${beforeDate.toISOString()}`;
    
    const response = http.del(url, null, {
        headers: defaultHeaders,
        timeout: '20s',
    });
    
    deletionTime.add(response.timings.duration);
    bulkDeletionEfficiency.add(response.timings.duration);
    
    const success = verifyResponse(response, 200, 'bulk deletion by date');
    deletionSuccessRate.add(success);
    
    if (success) {
        deletionThroughput.add(1);
        
        check(response, {
            'bulk date deletion succeeds': (r) => r.status === 200,
            'bulk date deletion time < 15s': (r) => r.timings.duration < 15000,
            'bulk date deletion has count': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return body.deleted_count !== undefined;
                } catch (e) {
                    return false;
                }
            },
        });
        
        // Log deletion count for monitoring
        try {
            const body = JSON.parse(response.body);
            console.log(`🗑️ Bulk date deletion removed ${body.deleted_count} events`);
        } catch (e) {
            // Continue
        }
    }
    
    sleep(0.2);
}

function testBulkDeletionByUser(data) {
    if (data.createdUserIds.length === 0) {
        sleep(0.1);
        return;
    }
    
    const userId = data.createdUserIds[Math.floor(Math.random() * data.createdUserIds.length)];
    
    const response = http.del(`${BASE_URL}/api/users/${userId}/events`, null, {
        headers: defaultHeaders,
        timeout: '15s',
    });
    
    deletionTime.add(response.timings.duration);
    bulkDeletionEfficiency.add(response.timings.duration);
    
    const success = verifyResponse(response, 200, 'bulk deletion by user');
    deletionSuccessRate.add(success);
    
    if (success) {
        deletionThroughput.add(1);
        
        check(response, {
            'bulk user deletion succeeds': (r) => r.status === 200,
            'bulk user deletion time < 10s': (r) => r.timings.duration < 10000,
            'bulk user deletion has count': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return body.deleted_count !== undefined;
                } catch (e) {
                    return false;
                }
            },
        });
    }
    
    sleep(0.15);
}

function testBulkDeletionByEventType(data) {
    const eventTypes = ['user_action', 'page_view', 'click', 'purchase', 'login'];
    const eventType = eventTypes[Math.floor(Math.random() * eventTypes.length)];
    
    const response = http.del(`${BASE_URL}/api/events?event_type=${eventType}`, null, {
        headers: defaultHeaders,
        timeout: '18s',
    });
    
    deletionTime.add(response.timings.duration);
    bulkDeletionEfficiency.add(response.timings.duration);
    
    const success = verifyResponse(response, 200, 'bulk deletion by event type');
    deletionSuccessRate.add(success);
    
    if (success) {
        deletionThroughput.add(1);
        
        check(response, {
            'bulk event type deletion succeeds': (r) => r.status === 200,
            'bulk event type deletion time < 12s': (r) => r.timings.duration < 12000,
        });
    }
    
    sleep(0.18);
}

function createEventsForDeletion(data) {
    // Create new events during the test to maintain deletion targets
    if (data.createdUserIds.length === 0) {
        sleep(0.1);
        return;
    }
    
    const userId = data.createdUserIds[Math.floor(Math.random() * data.createdUserIds.length)];
    const eventData = generateEvent(userId);
    
    const response = http.post(`${BASE_URL}/api/events`, JSON.stringify(eventData), {
        headers: defaultHeaders,
        timeout: '5s',
    });
    
    if (response.status === 201) {
        try {
            const body = JSON.parse(response.body);
            if (body.id) {
                data.createdEventIds.push(body.id);
            }
        } catch (e) {
            // Continue
        }
    }
    
    sleep(0.02);
}

export function teardown(data) {
    console.log('🧹 Mass Delete Events Test Teardown...');
    console.log(`📊 Test completed with ${deletionThroughput.count} deletion operations`);
    
    // Final cleanup - delete any remaining test data
    console.log('🗑️ Final cleanup of test data...');
    
    const cleanupStart = Date.now();
    
    // Delete all remaining events from test users
    for (const userId of data.createdUserIds) {
        const response = http.del(`${BASE_URL}/api/users/${userId}/events`, null, {
            headers: defaultHeaders,
            timeout: '10s',
        });
        
        if (response.status === 200) {
            try {
                const body = JSON.parse(response.body);
                console.log(`🧽 Cleaned up ${body.deleted_count} events for user ${userId}`);
            } catch (e) {
                // Continue
            }
        }
    }
    
    // Delete test users
    for (const userId of data.createdUserIds) {
        http.del(`${BASE_URL}/api/users/${userId}`, null, {
            headers: defaultHeaders,
            timeout: '5s',
        });
    }
    
    const cleanupTime = Date.now() - cleanupStart;
    deletionCleanupTime.add(cleanupTime);
    console.log(`✅ Cleanup completed in ${cleanupTime}ms`);
    
    // Final health check
    const healthResponse = http.get(`${BASE_URL}/health`, { headers: defaultHeaders });
    console.log(`🏥 Final health check: ${healthResponse.status}`);
    
    // Get final stats
    const statsResponse = http.get(`${BASE_URL}/api/analytics/stats`, { headers: defaultHeaders });
    if (statsResponse.status === 200) {
        try {
            const stats = JSON.parse(statsResponse.body);
            console.log('📈 Final stats after cleanup:', {
                totalEvents: stats.total_events,
                totalUsers: stats.total_users
            });
        } catch (e) {
            console.log('📊 Could not parse final stats');
        }
    }
}

export const testProfiles = {
    smoke: {
        scenarios: {
            smoke_deletion: {
                executor: 'constant-vus',
                vus: 2,
                duration: '3m',
            }
        }
    },
    
    load: {
        scenarios: {
            load_deletion: {
                executor: 'ramping-vus',
                startVUs: 0,
                stages: [
                    { duration: '2m', target: 20 },
                    { duration: '5m', target: 50 },
                    { duration: '2m', target: 0 },
                ],
            }
        }
    },
    
    stress: {
        scenarios: {
            stress_deletion: {
                executor: 'ramping-arrival-rate',
                preAllocatedVUs: 100,
                maxVUs: 300,
                stages: [
                    { duration: '2m', target: 100 },
                    { duration: '8m', target: 300 },
                    { duration: '5m', target: 500 },
                    { duration: '2m', target: 0 },
                ],
            }
        }
    }
};