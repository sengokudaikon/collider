import http from 'k6/http';
import { check, sleep } from 'k6';
import { Counter, Rate, Trend, Gauge } from 'k6/metrics';
import { SharedArray } from 'k6/data';

// Custom metrics
const eventCreateErrors = new Counter('event_create_errors');
const eventCreateSuccessRate = new Rate('event_create_success_rate');
const eventCreateLatency = new Trend('event_create_latency', true);
const activeUsers = new Gauge('active_users');

// Test data
const testUsers = new SharedArray('test_users', function () {
  return Array.from({ length: 1000 }, (_, i) => ({
    id: `k6_user_${i.toString().padStart(4, '0')}`,
    name: `K6 Test User ${i}`
  }));
});

const testEvents = new SharedArray('test_events', function () {
  const actions = ['click', 'view', 'scroll', 'hover', 'submit', 'search'];
  const pages = ['/dashboard', '/profile', '/settings', '/analytics', '/events'];
  const elements = ['button', 'link', 'form', 'menu', 'card', 'table'];
  
  return Array.from({ length: 100 }, (_, i) => ({
    event_type: 'user_action',
    action: actions[i % actions.length],
    element: `${elements[i % elements.length]}_${i}`,
    page: pages[i % pages.length],
    timestamp: new Date().toISOString(),
    metadata: {
      browser: 'Chrome',
      version: '120.0.0.0',
      platform: 'Linux',
      screen_resolution: '1920x1080',
      test_run: true,
      k6_iteration: i
    }
  }));
});

// Configuration
export let options = {
  stages: [
    // Ramp-up
    { duration: '30s', target: 100 },   // Warm up
    { duration: '1m', target: 500 },    // Gradual increase
    { duration: '2m', target: 1000 },   // Steady medium load
    { duration: '1m', target: 2000 },   // High load
    { duration: '30s', target: 5000 },  // Peak load
    
    // Sustained load
    { duration: '5m', target: 5000 },   // Sustain peak
    
    // Cool down
    { duration: '1m', target: 1000 },   // Reduce load
    { duration: '30s', target: 0 },     // Ramp down
  ],
  
  thresholds: {
    http_req_duration: ['p(95)<500', 'p(99)<1000'],
    http_req_failed: ['rate<0.05'], // Error rate < 5%
    event_create_success_rate: ['rate>0.95'],
    event_create_latency: ['p(95)<300', 'p(99)<500'],
  },
  
  ext: {
    loadimpact: {
      distribution: {
        'amazon:us:ashburn': { loadZone: 'amazon:us:ashburn', percent: 50 },
        'amazon:us:portland': { loadZone: 'amazon:us:portland', percent: 25 },
        'amazon:eu:dublin': { loadZone: 'amazon:eu:dublin', percent: 25 },
      },
    },
  },
};

const BASE_URL = __ENV.TARGET_URL || 'http://app:8080';

// Test scenarios
export default function () {
  // Track active users
  activeUsers.add(1);
  
  const testUser = testUsers[Math.floor(Math.random() * testUsers.length)];
  const testEvent = testEvents[Math.floor(Math.random() * testEvents.length)];
  
  // Create a test group for better organization
  group('Health Check', function () {
    const healthResponse = http.get(`${BASE_URL}/health`);
    check(healthResponse, {
      'health check status is 200': (r) => r.status === 200,
      'health check responds quickly': (r) => r.timings.duration < 100,
    });
  });
  
  group('Event Operations', function () {
    // Create event
    const eventPayload = {
      data: {
        ...testEvent,
        user_id: testUser.id,
        session_id: `k6_session_${__VU}_${__ITER}`,
        timestamp: new Date().toISOString(),
      }
    };
    
    const eventResponse = http.post(
      `${BASE_URL}/api/events`,
      JSON.stringify(eventPayload),
      {
        headers: {
          'Content-Type': 'application/json',
          'User-Agent': 'k6-load-test/1.0',
        },
      }
    );
    
    const eventCreateSuccess = check(eventResponse, {
      'event creation status is 201 or 200': (r) => [200, 201].includes(r.status),
      'event creation has valid response': (r) => r.body && r.body.length > 0,
      'event creation is fast': (r) => r.timings.duration < 1000,
    });
    
    // Track custom metrics
    eventCreateSuccessRate.add(eventCreateSuccess);
    eventCreateLatency.add(eventResponse.timings.duration);
    
    if (!eventCreateSuccess) {
      eventCreateErrors.add(1);
      console.error(`Event creation failed: ${eventResponse.status} ${eventResponse.body}`);
    }
    
    // Get events count
    const countResponse = http.get(`${BASE_URL}/api/events/count`);
    check(countResponse, {
      'events count status is 200': (r) => r.status === 200,
      'events count returns number': (r) => !isNaN(parseInt(r.body)),
    });
  });
  
  group('User Operations', function () {
    // Create user (less frequently)
    if (Math.random() < 0.1) { // 10% chance
      const userPayload = {
        data: {
          ...testUser,
          id: `${testUser.id}_${__VU}_${__ITER}`,
          created_at: new Date().toISOString(),
          metadata: {
            test_run: true,
            k6_vu: __VU,
            k6_iteration: __ITER,
          }
        }
      };
      
      const userResponse = http.post(
        `${BASE_URL}/api/users`,
        JSON.stringify(userPayload),
        {
          headers: {
            'Content-Type': 'application/json',
            'User-Agent': 'k6-load-test/1.0',
          },
        }
      );
      
      check(userResponse, {
        'user creation status is 201 or 200': (r) => [200, 201].includes(r.status),
        'user creation is fast': (r) => r.timings.duration < 500,
      });
    }
    
    // Get user analytics
    const analyticsResponse = http.get(`${BASE_URL}/api/users/${testUser.id}/analytics`);
    check(analyticsResponse, {
      'user analytics status is 200 or 404': (r) => [200, 404].includes(r.status),
    });
  });

  group('DELETE Operations', function () {
    // Bulk delete events (less frequently)
    if (Math.random() < 0.05) { // 5% chance
      const beforeDate = new Date(Date.now() - 60 * 60 * 1000).toISOString(); // 1 hour ago
      const bulkDeleteResponse = http.del(`${BASE_URL}/api/events?before=${beforeDate}`);
      
      check(bulkDeleteResponse, {
        'bulk delete status is 200': (r) => r.status === 200,
        'bulk delete responds quickly': (r) => r.timings.duration < 2000,
        'bulk delete has valid response': (r) => r.body && r.body.length > 0,
      });
    }
    
    // Single event delete (very rarely, may fail if event doesn't exist)
    if (Math.random() < 0.02) { // 2% chance
      const fakeEventId = '550e8400-e29b-41d4-a716-446655440000';
      const deleteResponse = http.del(`${BASE_URL}/api/events/${fakeEventId}`);
      
      check(deleteResponse, {
        'single delete status is 204 or 404': (r) => [204, 404].includes(r.status),
        'single delete responds quickly': (r) => r.timings.duration < 500,
      });
    }
  });

  group('Analytics and Stats', function () {
    // Get general stats
    const statsResponse = http.get(`${BASE_URL}/api/analytics/stats`);
    check(statsResponse, {
      'stats status is 200': (r) => r.status === 200,
      'stats response has data': (r) => r.body && r.body.length > 0,
      'stats responds quickly': (r) => r.timings.duration < 1000,
    });

    // Get real-time metrics
    if (Math.random() < 0.3) { // 30% chance
      const realtimeResponse = http.get(`${BASE_URL}/api/analytics/metrics/realtime`);
      check(realtimeResponse, {
        'realtime metrics status is 200': (r) => r.status === 200,
        'realtime metrics responds quickly': (r) => r.timings.duration < 800,
      });
    }

    // Get time series data
    if (Math.random() < 0.1) { // 10% chance
      const fromDate = new Date(Date.now() - 24 * 60 * 60 * 1000).toISOString(); // 24 hours ago
      const toDate = new Date().toISOString();
      const timeSeriesResponse = http.get(
        `${BASE_URL}/api/analytics/metrics/timeseries?from=${fromDate}&to=${toDate}&bucket=hour`
      );
      
      check(timeSeriesResponse, {
        'time series status is 200': (r) => r.status === 200,
        'time series responds in reasonable time': (r) => r.timings.duration < 2000,
      });
    }

    // Get popular events
    if (Math.random() < 0.05) { // 5% chance
      const popularResponse = http.get(`${BASE_URL}/api/analytics/events/popular?period=daily&limit=10`);
      check(popularResponse, {
        'popular events status is 200': (r) => r.status === 200,
        'popular events responds quickly': (r) => r.timings.duration < 1000,
      });
    }
  });
  
  // Think time - simulate real user behavior
  sleep(Math.random() * 3 + 1); // 1-4 seconds
  
  activeUsers.add(-1);
}

// Setup function - runs once before the test
export function setup() {
  console.log('🚀 Starting k6 load test');
  console.log(`Target: ${BASE_URL}`);
  console.log('Test configuration:');
  console.log(`- Peak VUs: 5000`);
  console.log(`- Duration: ~11 minutes`);
  console.log(`- Scenarios: Health, Events, Users`);
  
  // Verify server is available
  const healthCheck = http.get(`${BASE_URL}/health`);
  if (healthCheck.status !== 200) {
    throw new Error(`Server health check failed: ${healthCheck.status}`);
  }
  
  console.log('✅ Server health check passed');
  return { startTime: new Date().toISOString() };
}

// Teardown function - runs once after the test
export function teardown(data) {
  console.log('🏁 k6 load test completed');
  console.log(`Started: ${data.startTime}`);
  console.log(`Finished: ${new Date().toISOString()}`);
  console.log('Check the results above for performance metrics');
}

// Handle different test scenarios
export function smokeTest() {
  // Quick validation test
  const response = http.get(`${BASE_URL}/health`);
  check(response, {
    'smoke test status is 200': (r) => r.status === 200,
  });
}

export function stressTest() {
  // High load test
  for (let i = 0; i < 10; i++) {
    const eventPayload = {
      data: {
        event_type: 'stress_test',
        user_id: `stress_user_${__VU}`,
        action: 'stress_action',
        timestamp: new Date().toISOString(),
      }
    };
    
    http.post(`${BASE_URL}/api/events`, JSON.stringify(eventPayload), {
      headers: { 'Content-Type': 'application/json' },
    });
  }
}

export function spikeTest() {
  // Sudden load spike
  const responses = http.batch([
    ['GET', `${BASE_URL}/health`],
    ['GET', `${BASE_URL}/api/events/count`],
    ['POST', `${BASE_URL}/api/events`, JSON.stringify({
      data: {
        event_type: 'spike_test',
        user_id: `spike_user_${__VU}`,
        timestamp: new Date().toISOString(),
      }
    }), { headers: { 'Content-Type': 'application/json' } }],
  ]);
  
  responses.forEach((response, index) => {
    check(response, {
      [`spike test request ${index} successful`]: (r) => r.status < 400,
    });
  });
}