import http from 'k6/http';
import { check, sleep, group } from 'k6';
import { Counter, Rate, Trend, Gauge } from 'k6/metrics';
import { SharedArray } from 'k6/data';

// Custom metrics
const scenarioMetrics = {
  health_check: new Trend('health_check_duration', true),
  create_event: new Trend('create_event_duration', true),
  list_events: new Trend('list_events_duration', true),
  get_event: new Trend('get_event_duration', true),
  update_event: new Trend('update_event_duration', true),
  delete_event: new Trend('delete_event_duration', true),
};

const operationSuccess = new Rate('operation_success_rate');
const totalOperations = new Counter('total_operations');
const activeUsers = new Gauge('active_virtual_users');

// Test data arrays
const eventTypes = new SharedArray('event_types', function () {
  return [
    'user_action', 'page_view', 'click', 'form_submission', 'search',
    'download', 'purchase', 'registration', 'login', 'logout',
    'error', 'performance_metric'
  ];
});

const testScenarios = new SharedArray('scenarios', function () {
  return [
    { name: 'health_check', weight: 5, endpoint: '/health', method: 'GET' },
    { name: 'create_event', weight: 40, endpoint: '/api/events', method: 'POST' },
    { name: 'list_events', weight: 30, endpoint: '/api/events', method: 'GET' },
    { name: 'get_event', weight: 15, endpoint: '/api/events/{id}', method: 'GET' },
    { name: 'update_event', weight: 7, endpoint: '/api/events/{id}', method: 'PUT' },
    { name: 'delete_event', weight: 3, endpoint: '/api/events/{id}', method: 'DELETE' }
  ];
});

// Weighted scenario selection
function selectScenario() {
  const random = Math.random() * 100;
  let cumulative = 0;
  
  for (const scenario of testScenarios) {
    cumulative += scenario.weight;
    if (random <= cumulative) {
      return scenario;
    }
  }
  return testScenarios[0]; // fallback
}

// Generate realistic test data
function generateEventData() {
  const actions = ['click', 'view', 'scroll', 'hover', 'submit', 'search'];
  const pages = ['/', '/dashboard', '/profile', '/settings', '/analytics'];
  const browsers = ['Chrome/120.0.0.0', 'Firefox/119.0', 'Safari/17.1'];
  const platforms = ['Windows', 'macOS', 'Linux', 'iOS', 'Android'];
  
  return {
    user_id: `test_user_${Math.floor(Math.random() * 10000)}`,
    event_type: eventTypes[Math.floor(Math.random() * eventTypes.length)],
    timestamp: new Date().toISOString(),
    metadata: {
      action: actions[Math.floor(Math.random() * actions.length)],
      page: pages[Math.floor(Math.random() * pages.length)],
      browser: browsers[Math.floor(Math.random() * browsers.length)],
      platform: platforms[Math.floor(Math.random() * platforms.length)],
      session_id: `session_${__VU}_${Math.floor(__ITER / 10)}`,
      test_iteration: __ITER,
      virtual_user: __VU,
      test_run: true
    }
  };
}

// Scenario execution functions
function executeHealthCheck() {
  const response = http.get(`${BASE_URL}/health`, {
    headers: { 'User-Agent': 'k6-mixed-workload/1.0' }
  });
  
  const success = check(response, {
    'health check status 200': (r) => r.status === 200,
    'health check fast': (r) => r.timings.duration < 100,
  });
  
  scenarioMetrics.health_check.add(response.timings.duration);
  return success;
}

function executeCreateEvent() {
  const eventData = generateEventData();
  
  const response = http.post(`${BASE_URL}/api/events`, JSON.stringify({ data: eventData }), {
    headers: {
      'Content-Type': 'application/json',
      'User-Agent': 'k6-mixed-workload/1.0'
    }
  });
  
  const success = check(response, {
    'create event status ok': (r) => [200, 201].includes(r.status),
    'create event has response': (r) => r.body && r.body.length > 0,
    'create event fast': (r) => r.timings.duration < 1000,
  });
  
  scenarioMetrics.create_event.add(response.timings.duration);
  
  // Store created event ID for later use
  if (success && response.body) {
    try {
      const responseData = JSON.parse(response.body);
      if (responseData.id) {
        // Store in a shared array (simplified for this example)
        globalThis.createdEventIds = globalThis.createdEventIds || [];
        globalThis.createdEventIds.push(responseData.id);
      }
    } catch (e) {
      // Ignore JSON parse errors
    }
  }
  
  return success;
}

function executeListEvents() {
  const queryParams = new URLSearchParams();
  
  // Add random pagination
  if (Math.random() < 0.7) {
    queryParams.append('limit', String(Math.floor(Math.random() * 50) + 10));
    queryParams.append('offset', String(Math.floor(Math.random() * 100)));
  }
  
  // Add random user filter
  if (Math.random() < 0.3) {
    queryParams.append('user_id', `test_user_${Math.floor(Math.random() * 1000)}`);
  }
  
  const url = `${BASE_URL}/api/events${queryParams.toString() ? '?' + queryParams.toString() : ''}`;
  const response = http.get(url, {
    headers: { 'User-Agent': 'k6-mixed-workload/1.0' }
  });
  
  const success = check(response, {
    'list events status 200': (r) => r.status === 200,
    'list events has array': (r) => {
      try {
        const data = JSON.parse(r.body);
        return Array.isArray(data) || Array.isArray(data.events);
      } catch (e) {
        return false;
      }
    },
    'list events fast': (r) => r.timings.duration < 500,
  });
  
  scenarioMetrics.list_events.add(response.timings.duration);
  return success;
}

function executeGetEvent() {
  // Use a previously created event ID or generate a random UUID
  let eventId;
  if (globalThis.createdEventIds && globalThis.createdEventIds.length > 0) {
    eventId = globalThis.createdEventIds[Math.floor(Math.random() * globalThis.createdEventIds.length)];
  } else {
    // Generate a random UUID format for testing 404 responses
    eventId = `${Math.random().toString(36).substr(2, 8)}-${Math.random().toString(36).substr(2, 4)}-4${Math.random().toString(36).substr(2, 3)}-${Math.random().toString(36).substr(2, 4)}-${Math.random().toString(36).substr(2, 12)}`;
  }
  
  const response = http.get(`${BASE_URL}/api/events/${eventId}`, {
    headers: { 'User-Agent': 'k6-mixed-workload/1.0' }
  });
  
  const success = check(response, {
    'get event status ok': (r) => [200, 404].includes(r.status),
    'get event fast': (r) => r.timings.duration < 300,
  });
  
  scenarioMetrics.get_event.add(response.timings.duration);
  return success;
}

function executeUpdateEvent() {
  // Use a previously created event ID or skip
  if (!globalThis.createdEventIds || globalThis.createdEventIds.length === 0) {
    return true; // Skip if no events available
  }
  
  const eventId = globalThis.createdEventIds[Math.floor(Math.random() * globalThis.createdEventIds.length)];
  const updateData = {
    metadata: {
      updated_at: new Date().toISOString(),
      update_reason: 'load_test_modification',
      iteration: __ITER,
      virtual_user: __VU
    }
  };
  
  const response = http.put(`${BASE_URL}/api/events/${eventId}`, JSON.stringify(updateData), {
    headers: {
      'Content-Type': 'application/json',
      'User-Agent': 'k6-mixed-workload/1.0'
    }
  });
  
  const success = check(response, {
    'update event status ok': (r) => [200, 404].includes(r.status),
    'update event fast': (r) => r.timings.duration < 500,
  });
  
  scenarioMetrics.update_event.add(response.timings.duration);
  return success;
}

function executeDeleteEvent() {
  // Use a previously created event ID or skip
  if (!globalThis.createdEventIds || globalThis.createdEventIds.length === 0) {
    return true; // Skip if no events available
  }
  
  const eventId = globalThis.createdEventIds.pop(); // Remove from array
  
  const response = http.del(`${BASE_URL}/api/events/${eventId}`, null, {
    headers: { 'User-Agent': 'k6-mixed-workload/1.0' }
  });
  
  const success = check(response, {
    'delete event status ok': (r) => [204, 404].includes(r.status),
    'delete event fast': (r) => r.timings.duration < 400,
  });
  
  scenarioMetrics.delete_event.add(response.timings.duration);
  return success;
}

// Configuration
export let options = {
  stages: [
    { duration: '2m', target: 200 },   // Ramp up
    { duration: '5m', target: 500 },   // Steady load
    { duration: '2m', target: 1000 },  // Higher load
    { duration: '3m', target: 1000 },  // Sustain
    { duration: '2m', target: 0 },     // Ramp down
  ],
  
  thresholds: {
    http_req_duration: ['p(95)<500', 'p(99)<1000'],
    http_req_failed: ['rate<0.05'],
    operation_success_rate: ['rate>0.95'],
    health_check_duration: ['p(95)<100'],
    create_event_duration: ['p(95)<800'],
    list_events_duration: ['p(95)<400'],
    get_event_duration: ['p(95)<200'],
  },
};

const BASE_URL = __ENV.TARGET_URL || 'http://localhost:8080';

export default function () {
  activeUsers.add(1);
  totalOperations.add(1);
  
  const scenario = selectScenario();
  let success = false;
  
  group(`Mixed Workload - ${scenario.name}`, function () {
    switch (scenario.name) {
      case 'health_check':
        success = executeHealthCheck();
        break;
      case 'create_event':
        success = executeCreateEvent();
        break;
      case 'list_events':
        success = executeListEvents();
        break;
      case 'get_event':
        success = executeGetEvent();
        break;
      case 'update_event':
        success = executeUpdateEvent();
        break;
      case 'delete_event':
        success = executeDeleteEvent();
        break;
      default:
        success = executeHealthCheck();
    }
    
    operationSuccess.add(success);
  });
  
  // Realistic think time based on scenario
  const thinkTime = scenario.name === 'health_check' ? 0.1 : Math.random() * 2 + 0.5;
  sleep(thinkTime);
  
  activeUsers.add(-1);
}

export function setup() {
  console.log('🎯 Starting mixed workload performance test');
  console.log(`Target: ${BASE_URL}`);
  console.log('Scenario distribution:');
  testScenarios.forEach(scenario => {
    console.log(`  ${scenario.name}: ${scenario.weight}%`);
  });
  
  // Health check
  const healthResponse = http.get(`${BASE_URL}/health`);
  if (healthResponse.status !== 200) {
    throw new Error(`Health check failed: ${healthResponse.status}`);
  }
  
  return { startTime: new Date().toISOString() };
}

export function teardown(data) {
  console.log('✅ Mixed workload test completed');
  console.log(`Total operations executed across all scenarios`);
  console.log('Check individual scenario metrics for detailed performance data');
}