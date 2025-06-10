// K6 Performance Test Setup and Utilities
import { check, sleep } from 'k6';
import http from 'k6/http';
import { Rate, Counter, Trend } from 'k6/metrics';

// Custom metrics
export const errorRate = new Rate('errors');
export const eventCreations = new Counter('event_creations');
export const eventDeletions = new Counter('event_deletions');
export const eventQueries = new Counter('event_queries');
export const eventResponseTime = new Trend('event_response_time');

// Base configuration
export const BASE_URL = __ENV.BASE_URL || 'http://localhost:8880';

// Test data generators
export function generateUser() {
    const userId = generateUuid();
    return {
        id: userId,
        username: `user_${userId.slice(0, 8)}`,
        email: `user_${userId.slice(0, 8)}@example.com`,
        created_at: new Date().toISOString()
    };
}

export function generateEvent(userId) {
    return {
        user_id: userId || generateUuid(),
        event_type: randomChoice(['user_action', 'page_view', 'click', 'purchase', 'login']),
        timestamp: new Date().toISOString(),
        metadata: {
            session_id: generateUuid(),
            action: randomChoice(['click', 'view', 'scroll', 'hover', 'submit']),
            element: randomChoice(['button', 'link', 'form', 'image', 'text']),
            page: randomChoice(['/dashboard', '/profile', '/settings', '/events', '/analytics']),
            user_agent: 'k6-load-test',
            ip_address: `192.168.1.${Math.floor(Math.random() * 255)}`
        }
    };
}

export function generateUuid() {
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {
        const r = Math.random() * 16 | 0;
        const v = c == 'x' ? r : (r & 0x3 | 0x8);
        return v.toString(16);
    });
}

export function randomChoice(array) {
    return array[Math.floor(Math.random() * array.length)];
}

// Common request options
export const defaultHeaders = {
    'Content-Type': 'application/json',
    'Accept': 'application/json',
    'User-Agent': 'k6-performance-test'
};

// Health check utility
export function healthCheck() {
    const response = http.get(`${BASE_URL}/health`, { headers: defaultHeaders });
    return check(response, {
        'health check status is 200': (r) => r.status === 200,
        'health check response time < 100ms': (r) => r.timings.duration < 100,
    });
}

// Wait for service to be ready
export function waitForService(maxRetries = 30, retryInterval = 1000) {
    for (let i = 0; i < maxRetries; i++) {
        try {
            const response = http.get(`${BASE_URL}/health`, { headers: defaultHeaders });
            if (response.status === 200) {
                console.log(`✅ Service is ready after ${i + 1} attempts`);
                return true;
            }
        } catch (e) {
            console.log(`⏳ Waiting for service... attempt ${i + 1}/${maxRetries}`);
        }
        sleep(retryInterval / 1000);
    }
    throw new Error(`❌ Service not ready after ${maxRetries} attempts`);
}

// Seed initial data utility
export function seedUsers(count = 100) {
    console.log(`🌱 Seeding ${count} users...`);
    const users = [];
    const batchSize = 50;
    
    for (let i = 0; i < count; i += batchSize) {
        const batch = [];
        const remaining = Math.min(batchSize, count - i);
        
        for (let j = 0; j < remaining; j++) {
            batch.push(generateUser());
        }
        
        // Create users in batch
        batch.forEach(user => {
            const response = http.post(`${BASE_URL}/api/users`, JSON.stringify(user), { headers: defaultHeaders });
            if (response.status === 201 || response.status === 200) {
                users.push(user);
            }
        });
        
        if (i % 500 === 0) {
            console.log(`Created ${Math.min(i + batchSize, count)} users...`);
        }
    }
    
    console.log(`✅ Created ${users.length} users`);
    return users;
}

export function seedEvents(users, eventsPerUser = 100) {
    const totalEvents = users.length * eventsPerUser;
    console.log(`🌱 Seeding ${totalEvents} events (${eventsPerUser} per user)...`);
    
    let createdEvents = 0;
    const batchSize = 100;
    
    for (const user of users) {
        for (let i = 0; i < eventsPerUser; i += batchSize) {
            const batch = [];
            const remaining = Math.min(batchSize, eventsPerUser - i);
            
            for (let j = 0; j < remaining; j++) {
                batch.push(generateEvent(user.id));
            }
            
            // Create events in batch
            batch.forEach(event => {
                const response = http.post(`${BASE_URL}/api/events`, JSON.stringify(event), { headers: defaultHeaders });
                if (response.status === 201 || response.status === 200) {
                    createdEvents++;
                }
            });
        }
        
        if (createdEvents % 10000 === 0) {
            console.log(`Created ${createdEvents} events...`);
        }
    }
    
    console.log(`✅ Created ${createdEvents} events`);
    return createdEvents;
}

// Verify response utility
export function verifyResponse(response, expectedStatus = 200, operation = 'request') {
    const checks = {};
    checks[`${operation} status is ${expectedStatus}`] = (r) => r.status === expectedStatus;
    checks[`${operation} response time < 5000ms`] = (r) => r.timings.duration < 5000;
    
    if (expectedStatus === 200) {
        checks[`${operation} has valid JSON response`] = (r) => {
            try {
                JSON.parse(r.body);
                return true;
            } catch (e) {
                return false;
            }
        };
    }
    
    const result = check(response, checks);
    errorRate.add(!result);
    return result;
}