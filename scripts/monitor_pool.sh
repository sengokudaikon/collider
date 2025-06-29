#!/bin/bash

# Monitor database connection pool status
# Usage: ./monitor_pool.sh [host] [interval]

HOST="${1:-http://localhost:8880}"
INTERVAL="${2:-1}"

echo "📊 Monitoring Database Connection Pool Status"
echo "Host: $HOST"
echo "Interval: ${INTERVAL}s"
echo "Press Ctrl+C to stop"
echo ""

# Headers
printf "%-20s | %-10s | %-10s | %-10s | %-10s | %-10s | %-10s | %-10s\n" \
    "Timestamp" "Pri Avail" "Pri Size" "Pri Max" "Pri Use%" "Rep Avail" "Rep Size" "Rep Use%"
echo "--------------------------------------------------------------------------------"

while true; do
    # Get pool status
    RESPONSE=$(curl -s "$HOST/pool_status")
    
    if [ $? -eq 0 ] && [ -n "$RESPONSE" ]; then
        # Parse JSON response
        PRI_AVAIL=$(echo "$RESPONSE" | jq -r '.primary.available')
        PRI_SIZE=$(echo "$RESPONSE" | jq -r '.primary.size')
        PRI_MAX=$(echo "$RESPONSE" | jq -r '.primary.max_size')
        PRI_UTIL=$(echo "$RESPONSE" | jq -r '.primary.utilization_percent' | cut -d'.' -f1)
        
        # Check for read replica
        HAS_REPLICA=$(echo "$RESPONSE" | jq -r '.read_replica != null')
        if [ "$HAS_REPLICA" = "true" ]; then
            REP_AVAIL=$(echo "$RESPONSE" | jq -r '.read_replica.available')
            REP_SIZE=$(echo "$RESPONSE" | jq -r '.read_replica.size')
            REP_UTIL=$(echo "$RESPONSE" | jq -r '.read_replica.utilization_percent' | cut -d'.' -f1)
        else
            REP_AVAIL="-"
            REP_SIZE="-"
            REP_UTIL="-"
        fi
        
        # Color coding for utilization
        if [ "$PRI_UTIL" -gt 90 ]; then
            PRI_COLOR="\033[0;31m"  # Red
        elif [ "$PRI_UTIL" -gt 70 ]; then
            PRI_COLOR="\033[0;33m"  # Yellow
        else
            PRI_COLOR="\033[0;32m"  # Green
        fi
        
        # Print status
        printf "%-20s | ${PRI_COLOR}%-10s | %-10s | %-10s | %9s%%\033[0m | %-10s | %-10s | %9s%%\n" \
            "$(date '+%Y-%m-%d %H:%M:%S')" \
            "$PRI_AVAIL" "$PRI_SIZE" "$PRI_MAX" "$PRI_UTIL" \
            "$REP_AVAIL" "$REP_SIZE" "$REP_UTIL"
        
        # Alert on critical conditions
        if [ "$PRI_AVAIL" -eq 0 ] && [ "$PRI_SIZE" -gt 0 ]; then
            echo "⚠️  CRITICAL: Primary pool exhausted!"
        elif [ "$PRI_SIZE" -eq 0 ]; then
            echo "💤 Primary pool idle (no connections created yet)"
        fi
        if [ "$PRI_UTIL" -gt 95 ]; then
            echo "⚠️  WARNING: Primary pool utilization above 95%"
        fi
    else
        echo "❌ Failed to get pool status from $HOST"
    fi
    
    sleep "$INTERVAL"
done