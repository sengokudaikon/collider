import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';

// Custom metrics for endurance test
const enduranceSuccessRate = new Rate('endurance_success_rate');
const enduranceLatency = new Trend('endurance_latency', true);
const memoryLeaks = new Counter('potential_memory_leaks');

export let options = {
  stages: [
    { duration: '5m', target: 500 },    // Ramp up to steady load
    { duration: '30m', target: 500 },   // Maintain load for 30 minutes
    { duration: '5m', target: 0 },      // Ramp down
  ],
  
  thresholds: {
    http_req_duration: ['p(95)<1000', 'p(99)<2000'],
    http_req_failed: ['rate<0.05'],
    endurance_success_rate: ['rate>0.95'],
    endurance_latency: ['p(95)<800'],
  },
};

const BASE_URL = __ENV.TARGET_URL || 'http://localhost:8080';

export default function () {
  const iterationStart = Date.now();
  
  // Simulate consistent user behavior over time
  group('Endurance Test Scenario', function () {
    // Health check every 10th iteration
    if (__ITER % 10 === 0) {
      const healthResponse = http.get(`${BASE_URL}/health`);
      check(healthResponse, {
        'health check OK': (r) => r.status === 200,
      });
    }
    
    // Create event with realistic data
    // Create event with correct format
    const eventPayload = {
      user_id: `550e8400-e29b-41d4-a716-${(__VU % 100).toString().padStart(12, '0')}`, // Use UUID format
      event_type: 'endurance_event',
      timestamp: new Date().toISOString(),
      metadata: {
        session_id: `session_${__VU}_${Math.floor(__ITER / 50)}`, // New session every 50 iterations
        action: ['click', 'view', 'scroll', 'submit'][__ITER % 4],
        element: `element_${__ITER % 20}`,
        page: ['/dashboard', '/profile', '/settings', '/analytics'][__ITER % 4],
        test_type: 'endurance',
        duration_minutes: Math.floor((__ITER * 1000) / (60 * 1000)), // Track test duration
        browser: 'Chrome',
        version: '120.0.0.0',
      }
    };
    
    const eventResponse = http.post(
      `${BASE_URL}/api/events`,
      JSON.stringify(eventPayload),
      {
        headers: {
          'Content-Type': 'application/json',
          'User-Agent': 'k6-endurance-test/1.0',
        },
      }
    );
    
    const success = check(eventResponse, {
      'event creation successful': (r) => [200, 201].includes(r.status),
      'response time acceptable': (r) => r.timings.duration < 2000,
      'response has body': (r) => r.body && r.body.length > 0,
    });
    
    enduranceSuccessRate.add(success);
    enduranceLatency.add(eventResponse.timings.duration);
    
    // Check for potential memory leaks (increasing response times)
    if (eventResponse.timings.duration > 3000) {
      memoryLeaks.add(1);
    }
    
    // Get user analytics occasionally
    if (__ITER % 20 === 0) {
      const userId = `endurance_user_${(__VU % 100).toString().padStart(3, '0')}`;
      const analyticsResponse = http.get(`${BASE_URL}/api/users/${userId}/analytics`);
      check(analyticsResponse, {
        'analytics request OK': (r) => [200, 404].includes(r.status),
      });
    }
  });
  
  // Realistic think time
  const iterationTime = Date.now() - iterationStart;
  const targetIterationTime = 3000; // 3 seconds per iteration
  const sleepTime = Math.max(0, targetIterationTime - iterationTime) / 1000;
  
  sleep(sleepTime + Math.random() * 2); // Add some randomness
}

export function setup() {
  console.log('🕐 Starting endurance test - 40 minutes duration');
  console.log('📊 Monitoring for performance degradation over time');
  
  // Verify server health before starting long test
  const healthCheck = http.get(`${BASE_URL}/health`);
  if (healthCheck.status !== 200) {
    throw new Error(`Server not healthy before endurance test: ${healthCheck.status}`);
  }
  
  return { startTime: new Date().toISOString() };
}

export function teardown(data) {
  console.log('✅ Endurance test completed');
  console.log(`Duration: 40 minutes from ${data.startTime}`);
  console.log('📈 Check for performance degradation patterns in the results');
}