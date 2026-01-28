#!/bin/bash
# Quick test to verify the web server starts and responds

PORT=18080  # Use non-standard port for testing
TIMEOUT=10

echo "Testing BTC Miner Web Server..."
echo "================================"

# Create a minimal test config
cat > /tmp/btcminer_test.conf << EOF
Pool = solo.ckpool.org:3333
PayoutAddress = bc1qtest123456789
WebPort = $PORT
Threads = 1
EOF

# Start the miner in background
echo "[TEST] Starting miner on port $PORT..."
./btcminer -c /tmp/btcminer_test.conf &
MINER_PID=$!

# Give it time to start
sleep 2

# Check if miner is still running
if ! kill -0 $MINER_PID 2>/dev/null; then
    echo "[FAIL] Miner process died"
    exit 1
fi

echo "[TEST] Miner started (PID: $MINER_PID)"

# Test web endpoints
test_endpoint() {
    local endpoint=$1
    local expected=$2
    local response

    response=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:$PORT$endpoint" 2>/dev/null)

    if [ "$response" = "$expected" ]; then
        echo "[PASS] $endpoint -> HTTP $response"
        return 0
    else
        echo "[FAIL] $endpoint -> HTTP $response (expected $expected)"
        return 1
    fi
}

FAILED=0

# Test static file serving
test_endpoint "/" "200" || FAILED=1

# Test API endpoints
test_endpoint "/api/stats" "200" || FAILED=1
test_endpoint "/api/settings" "200" || FAILED=1
test_endpoint "/api/benchmark" "200" || FAILED=1
test_endpoint "/api/idle" "200" || FAILED=1

# Test that API returns valid JSON
echo "[TEST] Checking API response format..."
STATS=$(curl -s "http://localhost:$PORT/api/stats" 2>/dev/null)
if echo "$STATS" | grep -q '"hashrate"'; then
    echo "[PASS] /api/stats returns valid JSON with hashrate"
else
    echo "[FAIL] /api/stats response invalid: $STATS"
    FAILED=1
fi

IDLE=$(curl -s "http://localhost:$PORT/api/idle" 2>/dev/null)
if echo "$IDLE" | grep -q '"enabled"'; then
    echo "[PASS] /api/idle returns valid JSON with enabled field"
else
    echo "[FAIL] /api/idle response invalid: $IDLE"
    FAILED=1
fi

# Cleanup
echo "[TEST] Stopping miner..."
kill $MINER_PID 2>/dev/null
wait $MINER_PID 2>/dev/null
rm -f /tmp/btcminer_test.conf

echo "================================"
if [ $FAILED -eq 0 ]; then
    echo "All web tests PASSED!"
    exit 0
else
    echo "Some tests FAILED"
    exit 1
fi
