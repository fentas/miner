// BTC Miner Dashboard - Idle Mining Module

let idleEnabled = false;
let idlePollingInterval = null;

// Update idle mining UI
function updateIdleUI(enabled, userIdle, miningActive) {
    idleEnabled = enabled;
    const indicator = document.getElementById('idle-indicator');
    const visualStatus = document.getElementById('idle-visual-status');
    const activeStatus = document.getElementById('idle-active-status');
    const toggle = document.getElementById('idle-toggle');

    toggle.checked = enabled;

    if (!enabled) {
        indicator.className = 'idle-mining-indicator disabled';
        indicator.innerHTML = '<span class="moon-icon">🌙</span><span class="status-text">Disabled</span>';
        visualStatus.style.display = 'none';
        activeStatus.style.display = 'none';
    } else if (miningActive) {
        indicator.className = 'idle-mining-indicator active';
        indicator.innerHTML = '<span class="moon-icon">⛏️</span><span class="status-text" style="color: var(--success);">Mining!</span>';
        visualStatus.style.display = 'none';
        activeStatus.style.display = 'flex';
    } else if (userIdle) {
        indicator.className = 'idle-mining-indicator active';
        indicator.innerHTML = '<span class="moon-icon">💤</span><span class="status-text">User idle</span>';
        visualStatus.style.display = 'none';
        activeStatus.style.display = 'flex';
    } else {
        indicator.className = 'idle-mining-indicator waiting';
        indicator.innerHTML = '<span class="moon-icon">🌙</span><span class="status-text">Waiting...</span>';
        visualStatus.style.display = 'flex';
        activeStatus.style.display = 'none';
    }
}

// Fetch idle status from API
async function fetchIdleStatus() {
    try {
        const res = await fetch('/api/idle');
        const data = await res.json();

        updateIdleUI(data.enabled, data.is_idle, data.mining_due_to_idle);

        const idleTime = data.idle_time || 0;
        const timeout = data.timeout || 900;
        document.getElementById('idle-time-display').textContent = formatIdleTime(idleTime);
        document.getElementById('idle-timeout-display').textContent = formatIdleTime(timeout);

        if (data.is_idle && !data.mining_due_to_idle) {
            document.getElementById('idle-status-text').textContent = 'User is idle, starting soon...';
        } else if (!data.is_idle) {
            document.getElementById('idle-status-text').textContent = 'Waiting for idle...';
        }
    } catch (e) {
        console.error('Failed to fetch idle status:', e);
    }
}

// Format idle time
function formatIdleTime(seconds) {
    if (seconds >= 3600) {
        return Math.floor(seconds / 3600) + 'h ' + Math.floor((seconds % 3600) / 60) + 'm';
    } else if (seconds >= 60) {
        return Math.floor(seconds / 60) + 'm ' + (seconds % 60) + 's';
    }
    return seconds + 's';
}

// Toggle idle mining
async function toggleIdleMining(enable) {
    const timeout = parseInt(document.getElementById('idle-timeout').value) * 60 || 900;

    try {
        if (enable) {
            const res = await fetch(`/api/idle/enable?timeout=${timeout}`, { method: 'POST' });
            const data = await res.json();
            if (data.success) {
                window.app?.addLog(`Idle mining enabled (timeout: ${timeout/60}m)`, 'success');
                startIdlePolling();
            }
        } else {
            const res = await fetch('/api/idle/disable', { method: 'POST' });
            const data = await res.json();
            if (data.success) {
                window.app?.addLog('Idle mining disabled', 'info');
                stopIdlePolling();
            }
        }
        fetchIdleStatus();
    } catch (e) {
        window.app?.addLog('Failed to toggle idle mining: ' + e.message, 'error');
    }
}

// Update idle timeout
async function updateIdleTimeout() {
    const timeout = parseInt(document.getElementById('idle-timeout').value) * 60 || 900;

    try {
        const res = await fetch(`/api/idle/timeout?timeout=${timeout}`, { method: 'POST' });
        const data = await res.json();
        if (data.success) {
            window.app?.addLog(`Idle timeout updated to ${timeout/60} minutes`, 'info');
        }
    } catch (e) {
        console.error('Failed to update idle timeout:', e);
    }
}

// Polling functions
function startIdlePolling() {
    if (idlePollingInterval) return;
    idlePollingInterval = setInterval(fetchIdleStatus, 2000);
}

function stopIdlePolling() {
    if (idlePollingInterval) {
        clearInterval(idlePollingInterval);
        idlePollingInterval = null;
    }
}

// Check if enabled
function isEnabled() {
    return idleEnabled;
}

// Export functions
window.idle = {
    updateIdleUI,
    fetchIdleStatus,
    toggleIdleMining,
    updateIdleTimeout,
    startIdlePolling,
    stopIdlePolling,
    isEnabled
};
