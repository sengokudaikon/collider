#!/bin/bash

# Monitor database connection pool status (BRRRRR mode - single consolidated pool)
# Usage: ./monitor_pool.sh [host] [interval]

HOST="${1:-http://localhost:8880}"
INTERVAL="${2:-1}"

echo "📊 Monitoring Database Connection Pool Status"
echo "Host: $HOST"
echo "Interval: ${INTERVAL}s"
echo "Press Ctrl+C to stop"
echo ""

# Headers
printf "%-20s | %-10s | %-10s | %-10s | %-10s | %-20s\n" \
    "Timestamp" "Available" "Size" "Max" "Use%" "Status"
echo "--------------------------------------------------------------------------------"

while true; do
    # Get pool status
    RESPONSE=$(curl -s "$HOST/pool_status")
    
    if [ $? -eq 0 ] && [ -n "$RESPONSE" ]; then
        # Parse JSON response - BRRRRR mode only has primary pool
        AVAIL=$(echo "$RESPONSE" | jq -r '.primary.available')
        SIZE=$(echo "$RESPONSE" | jq -r '.primary.size')
        MAX=$(echo "$RESPONSE" | jq -r '.primary.max_size')
        UTIL=$(echo "$RESPONSE" | jq -r '.primary.utilization_percent' | cut -d'.' -f1)
        
        # Color coding for utilization
        if [ "$UTIL" -gt 90 ]; then
            COLOR="\033[0;31m"  # Red
            STATUS="CRITICAL"
        elif [ "$UTIL" -gt 70 ]; then
            COLOR="\033[0;33m"  # Yellow
            STATUS="WARNING"
        else
            COLOR="\033[0;32m"  # Green
            STATUS="HEALTHY"
        fi
        
        # Print status
        printf "%-20s | ${COLOR}%-10s | %-10s | %-10s | %9s%%\033[0m | %-20s\n" \
            "$(date '+%Y-%m-%d %H:%M:%S')" \
            "$AVAIL" "$SIZE" "$MAX" "$UTIL" \
            "$STATUS"
        
        # Alert on critical conditions
        if [ "$AVAIL" -eq 0 ] && [ "$SIZE" -gt 0 ]; then
            echo "🚨 CRITICAL: Connection pool exhausted! (BRRRRR mode: $MAX max connections)"
        elif [ "$SIZE" -eq 0 ]; then
            echo "💤 Pool idle (no connections created yet)"
        fi
        if [ "$UTIL" -gt 95 ]; then
            echo "⚠️  WARNING: Pool utilization above 95% - consider increasing max_conn"
        fi
    else
        echo "❌ Failed to get pool status from $HOST"
    fi
    
    sleep "$INTERVAL"
done