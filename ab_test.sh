#!/bin/bash

if [ -z "$1" ]; then
    echo "Usage: $0 <host> <user_id> <event_type>"
    echo "Example: $0 http://localhost 123 click"
    exit 1
fi

HOST="$1"
UID="${2:-123}"
TYPE="${3:-click}"
REQUESTS=5000
CONCURRENCY=50

cat > event_data.json << EOF
{
  "user_id": $UID,
  "event_type": "$TYPE",
  "timestamp": "2025-06-24T12:34:56Z",
  "metadata": {
    "page": "/home",
    "button": "login"
  }
}
EOF

echo "🚀 Запуск ab тестов"
echo "Host: $HOST"
echo "Requests: $REQUESTS"
echo "Concurrency: $CONCURRENCY"
echo "UID: $UID"
echo "TYPE: $TYPE"
echo "=================================="

echo "📊 Testing Events list..."
ab -n $REQUESTS -c $CONCURRENCY "$HOST/events?page=1&limit=1000"
echo ""

echo "📊 Testing User events..."
ab -n $REQUESTS -c $CONCURRENCY "$HOST/users/$UID/events?limit=1000"
echo ""

echo "📊 Testing POST /event..."
ab -n $REQUESTS -c $CONCURRENCY \
   -T "application/json" \
   -p event_data.json \
   "$HOST/event"

echo "📊 Testing Stats..."
ab -n $REQUESTS -c $CONCURRENCY "$HOST/stats?from=2025-06-01T00:00:00Z&to=2025-06-30T23:59:59Z&type=$TYPE&limit=10"
echo ""

echo "✅ Тесты завершены!"
