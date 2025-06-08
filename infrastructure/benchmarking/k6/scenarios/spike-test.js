import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';

// Custom metrics for spike test
const spikeSuccessRate = new Rate('spike_success_rate');
const spikeLatency = new Trend('spike_latency', true);

export let options = {
  stages: [
    { duration: '2m', target: 100 },    // Normal load
    { duration: '30s', target: 2000 },  // Spike to 2000 users
    { duration: '1m', target: 2000 },   // Stay at 2000 users
    { duration: '30s', target: 100 },   // Drop back to 100 users
    { duration: '2m', target: 100 },    // Recovery period
  ],
  
  thresholds: {
    http_req_duration: ['p(99)<2000'], // Relaxed threshold for spike test
    http_req_failed: ['rate<0.1'],     // Error rate < 10%
    spike_success_rate: ['rate>0.8'],  // 80% success during spike
  },
};

const BASE_URL = __ENV.TARGET_URL || 'http://localhost:8080';

export default function () {
  // Simulate real user behavior during spike
  const responses = http.batch([
    ['GET', `${BASE_URL}/health`],
    ['GET', `${BASE_URL}/api/events/count`],
    ['POST', `${BASE_URL}/api/events`, JSON.stringify({
      data: {
        event_type: 'spike_event',
        user_id: `spike_user_${__VU}`,
        action: 'spike_action',
        timestamp: new Date().toISOString(),
        metadata: {
          test_type: 'spike',
          vu: __VU,
          iter: __ITER,
        }
      }
    }), { headers: { 'Content-Type': 'application/json' } }],
  ]);
  
  // Check all responses
  const allSuccess = responses.every(response => {
    const success = check(response, {
      'status is 2xx': (r) => r.status >= 200 && r.status < 300,
      'response time OK': (r) => r.timings.duration < 5000,
    });
    spikeLatency.add(response.timings.duration);
    return success;
  });
  
  spikeSuccessRate.add(allSuccess);
  
  // Shorter sleep during spike
  sleep(Math.random() * 2 + 0.5); // 0.5-2.5 seconds
}