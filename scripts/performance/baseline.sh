# Run miner in background
./target/release/btcminer --debug 2>&1 > /tmp/miner.log &
PID=$!
echo "Started miner PID: $PID"

# Wait for API to return valid stats (up to 60 seconds)
echo "Waiting for mining to start..."
retries=0
# {"block_height":0,"blocks_found":0,"difficulty":0.0,"est_time_to_share":0.0,"gpu_device":"","gpu_enabled":false,"gpu_hashrate":0.0,"hashes":3129149489152,"hashrate":77576530331.37267,"hashrate_formatted":"77.58 GH/s","hashrate_unit":"77.58 GH/s","idle_mining_active":false,"idle_mining_enabled":false,"mining_paused":false,"mode":"solo","network_hashrate":0.0,"peers":0,"pool_preset":"solo","shares_accepted":0,"shares_rejected":0,"state":"running","threads":24,"total_hashes":3129149489152,"uptime":44,"uptime_formatted":"44s","uptime_seconds":44,"user_idle":false}%   
until curl -s --fail http://localhost:9080/api/stats 2>/dev/null 2>/dev/null; do
    sleep 2
    retries=$((retries + 1))
    if [ $retries -ge 60 ]; then
        echo "Timeout waiting for mining"
        kill $PID 2>/dev/null
        exit 1
    fi
done

echo "Waiting for bitcoind to connect..."
retries=0
until curl -s --fail http://localhost:9080/api/stats 2>/dev/null | jq -e '.hashes > 0'; do
    sleep 2
    retries=$((retries + 1))
    if [ $retries -ge 60 ]; then
        echo "Timeout waiting for bitcoind to connect"
        kill $PID 2>/dev/null
        exit 1
    fi
done

# Collect 3 samples with 5 seconds between
echo ""
echo "=== Collecting 3 samples ==="
for i in 1 2 3; do
    echo ""
    echo "--- Sample $i ---"
    curl -s http://localhost:9080/api/stats 2>/dev/null
    sleep 5
done

echo ""
echo "=== Done ==="
kill $PID 2>/dev/null
wait $PID 2>/dev/null
echo "Stopped miner"