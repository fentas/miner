// BTC Miner Dashboard - WebSocket Module

let ws = null;
let connected = false;
let reconnectTimeout = null;
let lastBlockHeight = 0;
let lastSharesAccepted = 0;
let lastSharesRejected = 0;

// Connect to WebSocket
function connectWebSocket() {
    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
    ws = new WebSocket(`${protocol}//${location.host}/ws`);

    ws.onopen = () => {
        connected = true;
        updateConnectionStatus(true);
        window.app?.addLog('Connected to miner', 'success');
    };

    ws.onclose = () => {
        const wasConnected = connected;
        connected = false;
        updateConnectionStatus(false);

        if (wasConnected) {
            window.app?.addLog('Disconnected from miner', 'error');
            window.notifications?.notifyConnectionLost();
        }

        // Reconnect after 3 seconds
        reconnectTimeout = setTimeout(connectWebSocket, 3000);
    };

    ws.onerror = () => {
        window.app?.addLog('WebSocket error', 'error');
    };

    ws.onmessage = (event) => {
        try {
            const data = JSON.parse(event.data);
            handleStats(data);
        } catch (e) {
            console.error('Failed to parse message:', e);
        }
    };
}

// Update connection status in header
function updateConnectionStatus(isConnected) {
    const statusEl = document.getElementById('connection-status');
    if (!statusEl) return;

    const textEl = statusEl.querySelector('.connection-text');

    if (isConnected) {
        statusEl.classList.add('connected');
        statusEl.classList.remove('disconnected');
        if (textEl) textEl.textContent = 'Connected';
    } else {
        statusEl.classList.add('disconnected');
        statusEl.classList.remove('connected');
        if (textEl) textEl.textContent = 'Disconnected';
    }
}

// Handle incoming stats data
function handleStats(data) {
    // Update hashrate
    if (data.hashrate !== undefined) {
        const hashrateEl = document.getElementById('hashrate');
        if (hashrateEl) hashrateEl.textContent = window.chart?.formatHashrate(data.hashrate) || '0 H/s';
        window.chart?.addHashrateData(data.hashrate);
    }

    // Update total hashes
    if (data.total_hashes !== undefined) {
        const el = document.getElementById('total-hashes');
        if (el) el.textContent = window.app?.formatNumber(data.total_hashes) || data.total_hashes;
    }

    // Update shares
    if (data.shares !== undefined) {
        const el = document.getElementById('shares');
        if (el) el.textContent = data.shares;
    }

    if (data.shares_accepted !== undefined) {
        const el = document.getElementById('shares-accepted');
        if (el) el.textContent = data.shares_accepted;

        // Notify on new share accepted
        if (data.shares_accepted > lastSharesAccepted && lastSharesAccepted > 0) {
            window.notifications?.notifyShareAccepted(data.shares_accepted);
        }
        lastSharesAccepted = data.shares_accepted;
    }

    if (data.shares_rejected !== undefined) {
        const el = document.getElementById('shares-rejected');
        if (el) el.textContent = data.shares_rejected;

        // Notify on new share rejected
        if (data.shares_rejected > lastSharesRejected && lastSharesRejected > 0) {
            window.notifications?.notifyShareRejected(data.shares_rejected);
        }
        lastSharesRejected = data.shares_rejected;
    }

    // Update threads
    if (data.threads !== undefined) {
        const el = document.getElementById('threads');
        if (el) el.textContent = data.threads;
    }

    // Update block height
    if (data.block_height !== undefined) {
        const el = document.getElementById('block-height');
        if (el) el.textContent = data.block_height > 0 ? data.block_height : '-';

        // Notify on new block (only if height increased)
        if (data.block_height > lastBlockHeight && lastBlockHeight > 0) {
            window.notifications?.notifyNewBlock(data.block_height);
        }
        lastBlockHeight = data.block_height;
    }

    // Update difficulty
    if (data.difficulty !== undefined) {
        const el = document.getElementById('difficulty');
        if (el) el.textContent = data.difficulty > 0 ? data.difficulty.toFixed(4) : '-';
    }

    // Update estimated time
    if (data.est_time_to_share !== undefined) {
        const el = document.getElementById('est-time');
        if (el) el.textContent = data.est_time_to_share > 0 ? '~' + window.app?.formatDuration(data.est_time_to_share) : '-';
    }

    // Update uptime
    if (data.uptime !== undefined) {
        const el = document.getElementById('uptime');
        if (el) el.textContent = window.app?.formatUptime(data.uptime * 1000) || '0:00:00';
    }

    // GPU stats
    if (data.gpu_enabled !== undefined) {
        const gpuCard = document.getElementById('gpu-hashrate-card');
        if (gpuCard) gpuCard.style.display = data.gpu_enabled ? 'block' : 'none';
    }

    if (data.gpu_hashrate !== undefined) {
        const gpuHrEl = document.getElementById('gpu-hashrate');
        if (gpuHrEl) gpuHrEl.textContent = data.gpu_hashrate > 0 ? window.chart?.formatHashrate(data.gpu_hashrate) : '-';

        const dashGpuHrEl = document.getElementById('dashboard-gpu-hashrate');
        if (dashGpuHrEl) dashGpuHrEl.textContent = data.gpu_hashrate > 0 ? window.chart?.formatHashrate(data.gpu_hashrate) : '0 H/s';
    }

    if (data.gpu_status !== undefined) {
        const el = document.getElementById('gpu-status-text');
        if (el) el.textContent = data.gpu_status;
    }

    if (data.gpu_device !== undefined) {
        const el = document.getElementById('gpu-device-info');
        if (el) el.textContent = data.gpu_device || '-';
    }

    // Log messages from backend
    if (data.log) {
        window.app?.addLog(data.log, data.log_type || 'info');

        // Notify on errors
        if (data.log_type === 'error') {
            window.notifications?.notifyError(data.log);
        }
    }

    // Block found notification
    if (data.block_found) {
        window.notifications?.notifyFoundBlock(data.block_found_height, data.block_found_hash);
    }

    // Idle mining started notification
    if (data.idle_mining_started) {
        window.notifications?.notifyIdleMiningStart();
    }

    // Miner state
    if (data.state) {
        window.app?.updateMinerState(data.state);
    }

    // Idle mining status
    if (data.idle_mining_enabled !== undefined) {
        window.idle?.updateIdleUI(data.idle_mining_enabled, data.user_idle, data.idle_mining_active);
    }
}

// Disconnect WebSocket
function disconnect() {
    if (reconnectTimeout) {
        clearTimeout(reconnectTimeout);
        reconnectTimeout = null;
    }
    if (ws) {
        ws.close();
        ws = null;
    }
}

// Check connection status
function isConnected() {
    return connected;
}

// Export functions
window.websocket = {
    connect: connectWebSocket,
    disconnect,
    isConnected,
    handleStats,
    updateConnectionStatus
};
