// BTC Miner Dashboard - Idle Mining Module

let idleEnabled = false;
let idlePollingInterval = null;

// Update idle mining UI
function updateIdleUI(enabled, userIdle, miningActive) {
    idleEnabled = enabled;
    const indicator = document.getElementById('idle-indicator');
    const idleDetails = document.getElementById('idle-details');
    const toggle = document.getElementById('idle-toggle');

    if (toggle) toggle.checked = enabled;

    if (!indicator) return;

    const iconEl = indicator.querySelector('.idle-icon');
    const textEl = indicator.querySelector('.idle-text');

    if (!enabled) {
        indicator.className = 'idle-status-display disabled';
        if (iconEl) iconEl.textContent = '🌙';
        if (textEl) textEl.textContent = 'Disabled';
        if (idleDetails) idleDetails.style.display = 'none';
    } else if (miningActive) {
        indicator.className = 'idle-status-display mining';
        if (iconEl) iconEl.textContent = '⛏️';
        if (textEl) textEl.textContent = 'Mining!';
        if (idleDetails) idleDetails.style.display = 'block';
    } else if (userIdle) {
        indicator.className = 'idle-status-display active';
        if (iconEl) iconEl.textContent = '💤';
        if (textEl) textEl.textContent = 'User Idle';
        if (idleDetails) idleDetails.style.display = 'block';
    } else {
        indicator.className = 'idle-status-display waiting';
        if (iconEl) iconEl.textContent = '🌙';
        if (textEl) textEl.textContent = 'Waiting...';
        if (idleDetails) idleDetails.style.display = 'block';
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

        const idleTimeDisplay = document.getElementById('idle-time-display');
        if (idleTimeDisplay) idleTimeDisplay.textContent = formatIdleTime(idleTime);

        const idleTimeoutDisplay = document.getElementById('idle-timeout-display');
        if (idleTimeoutDisplay) idleTimeoutDisplay.textContent = formatIdleTime(timeout);

        // Update the dashboard timeout input with the actual configured value
        const timeoutInput = document.getElementById('idle-timeout');
        if (timeoutInput && !timeoutInput.matches(':focus')) {
            timeoutInput.value = Math.floor(timeout / 60);
        }

        // Update settings page timeout input
        const settingTimeoutInput = document.getElementById('idle-timeout-setting');
        if (settingTimeoutInput && !settingTimeoutInput.matches(':focus')) {
            settingTimeoutInput.value = Math.floor(timeout / 60);
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
    const timeoutInput = document.getElementById('idle-timeout');
    const timeout = timeoutInput ? (parseInt(timeoutInput.value) * 60) || 900 : 900;

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
    const timeoutInput = document.getElementById('idle-timeout');
    const timeout = timeoutInput ? (parseInt(timeoutInput.value) * 60) || 900 : 900;

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
