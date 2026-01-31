// BTC Miner Dashboard - Main Application

// Application state
let minerState = 'stopped';
let benchmarkInterval = null;
let logAutoScroll = true;

// Pool presets configuration
const poolPresets = {
    ckpool: {
        host: 'solo.ckpool.org',
        port: 3333,
        stratum: 'v1',
        description: 'Solo mining pool with Stratum V1. 2% fee on successful block rewards.'
    },
    braiins: {
        host: 'solo.stratum.braiins.com',
        port: 3333,
        stratum: 'v1',
        description: 'Braiins Solo mining pool with Stratum V1. 0.5% fee on found blocks.'
    },
    ocean: {
        host: 'mine.ocean.xyz',
        port: 3334,
        stratum: 'v1',
        description: 'Ocean Pool with DATUM support. 0% pool fee.'
    },
    none: {
        host: '',
        port: 0,
        stratum: 'none',
        description: 'True solo mining - connect directly to your Bitcoin Core node. 0% fees.'
    }
};

// DOM elements
let logOutput;

// Initialize DOM references
function initDom() {
    logOutput = document.getElementById('log-output');
}

// Utility Functions
function formatNumber(n) {
    if (n >= 1e12) return (n / 1e12).toFixed(2) + 'T';
    if (n >= 1e9) return (n / 1e9).toFixed(2) + 'G';
    if (n >= 1e6) return (n / 1e6).toFixed(2) + 'M';
    if (n >= 1e3) return (n / 1e3).toFixed(2) + 'K';
    return n.toString();
}

function formatUptime(ms) {
    const s = Math.floor(ms / 1000);
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = s % 60;
    return `${h}:${m.toString().padStart(2, '0')}:${sec.toString().padStart(2, '0')}`;
}

function formatDuration(seconds) {
    if (seconds < 60) {
        return Math.round(seconds) + 's';
    } else if (seconds < 3600) {
        const mins = Math.floor(seconds / 60);
        const secs = Math.round(seconds % 60);
        return mins + 'm ' + secs + 's';
    } else if (seconds < 86400) {
        const hours = Math.floor(seconds / 3600);
        const mins = Math.floor((seconds % 3600) / 60);
        return hours + 'h ' + mins + 'm';
    } else if (seconds < 31536000) {
        // Less than a year
        const days = Math.floor(seconds / 86400);
        const hours = Math.floor((seconds % 86400) / 3600);
        return days + 'd ' + hours + 'h';
    } else {
        // Years - use compact format for huge numbers
        const years = seconds / 31536000;
        if (years >= 1e9) return (years / 1e9).toFixed(1) + 'B years';
        if (years >= 1e6) return (years / 1e6).toFixed(1) + 'M years';
        if (years >= 1e3) return (years / 1e3).toFixed(1) + 'K years';
        return years.toFixed(1) + ' years';
    }
}

// Logging
function addLog(message, type = 'info') {
    if (!logOutput) initDom();
    if (!logOutput) return;

    const time = new Date().toLocaleTimeString();
    const entry = document.createElement('div');
    entry.className = 'log-entry';
    entry.innerHTML = `<span class="log-time">[${time}]</span> <span class="log-${type}">${message}</span>`;
    logOutput.appendChild(entry);

    // Auto-scroll if enabled
    if (logAutoScroll) {
        logOutput.scrollTop = logOutput.scrollHeight;
    }

    // Keep only last 500 entries
    while (logOutput.children.length > 500) {
        logOutput.removeChild(logOutput.firstChild);
    }
}

// Tab Navigation
function initNavigation() {
    document.querySelectorAll('.nav-btn').forEach(btn => {
        btn.addEventListener('click', () => {
            document.querySelectorAll('.nav-btn').forEach(b => b.classList.remove('active'));
            document.querySelectorAll('.tab-content').forEach(t => {
                t.style.display = 'none';
                t.classList.remove('active');
            });

            btn.classList.add('active');
            const tab = document.getElementById(btn.dataset.tab);
            if (tab) {
                tab.style.display = 'block';
                tab.classList.add('active');
            }
        });
    });
}

// Pool preset handling
function updatePoolPreset() {
    const preset = document.getElementById('solo-pool-preset')?.value;
    const poolHostInput = document.getElementById('pool-host');
    const presetInfo = document.getElementById('pool-preset-info');
    const poolHostRow = document.getElementById('pool-host-row');

    if (!preset) return;

    if (preset === 'custom') {
        if (poolHostInput) poolHostInput.disabled = false;
        if (poolHostRow) poolHostRow.style.display = 'block';
        if (presetInfo) {
            presetInfo.textContent = '';
            presetInfo.style.display = 'none';
        }
    } else if (preset === 'none') {
        if (poolHostInput) poolHostInput.disabled = true;
        if (poolHostRow) poolHostRow.style.display = 'none';
        if (presetInfo) {
            presetInfo.textContent = poolPresets.none.description;
            presetInfo.style.display = 'block';
        }
    } else {
        const config = poolPresets[preset];
        if (config) {
            if (poolHostInput) {
                poolHostInput.value = `${config.host}:${config.port}`;
                poolHostInput.disabled = true;
            }
            if (poolHostRow) poolHostRow.style.display = 'block';
            if (presetInfo) {
                presetInfo.textContent = config.description;
                presetInfo.style.display = 'block';
            }
        }
    }
}

// Address validation
function validateAddress(address) {
    const validationEl = document.getElementById('address-validation');
    if (!validationEl) return true;

    if (!address) {
        validationEl.className = 'form-validation error';
        validationEl.textContent = 'Bitcoin address is required';
        return false;
    }

    const legacy = /^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$/;
    const bech32 = /^bc1[a-z0-9]{39,59}$/i;
    const bech32m = /^bc1p[a-z0-9]{58}$/i;

    if (legacy.test(address)) {
        validationEl.className = 'form-validation success';
        validationEl.textContent = 'Valid legacy address';
        return true;
    } else if (bech32.test(address) || bech32m.test(address)) {
        validationEl.className = 'form-validation success';
        validationEl.textContent = 'Valid SegWit address';
        return true;
    } else if (address.length < 26) {
        validationEl.className = 'form-validation error';
        validationEl.textContent = 'Address is too short';
        return false;
    } else {
        validationEl.className = 'form-validation';
        validationEl.textContent = '';
        return true;
    }
}

// Update GPU settings visibility
function updateGpuSettings(enabled) {
    const gpuSettings = document.getElementById('gpu-settings');
    if (gpuSettings) gpuSettings.style.display = enabled ? 'block' : 'none';

    const gpuCard = document.getElementById('gpu-hashrate-card');
    if (gpuCard) gpuCard.style.display = enabled ? 'block' : 'none';
}

// Update CPU threads visibility
function updateCpuSettings(enabled) {
    const cpuThreadsRow = document.getElementById('cpu-threads-row');
    if (cpuThreadsRow) cpuThreadsRow.style.display = enabled ? 'block' : 'none';
}

// Miner state management
function updateMinerState(state) {
    minerState = state;
    const indicator = document.getElementById('state-indicator');
    const stateText = document.getElementById('state-text');

    if (indicator) {
        indicator.className = 'state-dot ' + state;
    }
    if (stateText) {
        stateText.textContent = state.charAt(0).toUpperCase() + state.slice(1);
    }

    const btnStart = document.getElementById('btn-start');
    const btnStop = document.getElementById('btn-stop');
    const btnPause = document.getElementById('btn-pause');
    const btnResume = document.getElementById('btn-resume');
    const btnBenchStart = document.getElementById('btn-benchmark-start');
    const btnBenchStop = document.getElementById('btn-benchmark-stop');

    switch (state) {
        case 'stopped':
            if (btnStart) btnStart.disabled = false;
            if (btnStop) btnStop.disabled = true;
            if (btnPause) btnPause.disabled = true;
            if (btnResume) btnResume.disabled = true;
            if (btnBenchStart) btnBenchStart.disabled = false;
            if (btnBenchStop) btnBenchStop.disabled = true;
            break;
        case 'running':
            if (btnStart) btnStart.disabled = true;
            if (btnStop) btnStop.disabled = false;
            if (btnPause) btnPause.disabled = false;
            if (btnResume) btnResume.disabled = true;
            if (btnBenchStart) btnBenchStart.disabled = false;
            if (btnBenchStop) btnBenchStop.disabled = true;
            break;
        case 'paused':
            if (btnStart) btnStart.disabled = true;
            if (btnStop) btnStop.disabled = false;
            if (btnPause) btnPause.disabled = true;
            if (btnResume) btnResume.disabled = false;
            if (btnBenchStart) btnBenchStart.disabled = true;
            if (btnBenchStop) btnBenchStop.disabled = true;
            break;
        case 'benchmarking':
            if (btnStart) btnStart.disabled = true;
            if (btnStop) btnStop.disabled = true;
            if (btnPause) btnPause.disabled = false;
            if (btnResume) btnResume.disabled = true;
            if (btnBenchStart) btnBenchStart.disabled = true;
            if (btnBenchStop) btnBenchStop.disabled = false;
            break;
    }
}

// Miner control
async function controlMiner(action) {
    try {
        const res = await fetch(`/api/control/${action}`);
        const data = await res.json();
        if (data.state) updateMinerState(data.state);
        addLog(`Miner ${action}: ${data.status || 'OK'}`, 'success');
    } catch (e) {
        addLog(`Failed to ${action} miner: ${e.message}`, 'error');
    }
}

// Benchmark functions
async function startBenchmark() {
    const duration = parseInt(document.getElementById('benchmark-duration')?.value) || 0;
    try {
        const res = await fetch(`/api/control/benchmark?duration=${duration}`, { method: 'POST' });
        const data = await res.json();
        if (data.state) updateMinerState(data.state);
        addLog(`Benchmark started${duration > 0 ? ' for ' + duration + 's' : ''}`, 'success');

        if (benchmarkInterval) clearInterval(benchmarkInterval);
        benchmarkInterval = setInterval(updateBenchmarkResults, 1000);
    } catch (e) {
        addLog('Failed to start benchmark: ' + e.message, 'error');
    }
}

async function stopBenchmark() {
    try {
        const res = await fetch('/api/control/benchmark/stop', { method: 'POST' });
        const data = await res.json();
        if (data.state) updateMinerState(data.state);
        addLog('Benchmark stopped', 'info');

        if (benchmarkInterval) {
            clearInterval(benchmarkInterval);
            benchmarkInterval = null;
        }

        updateBenchmarkResults();
    } catch (e) {
        addLog('Failed to stop benchmark: ' + e.message, 'error');
    }
}

async function updateBenchmarkResults() {
    try {
        const res = await fetch('/api/benchmark');
        const data = await res.json();

        const hrEl = document.getElementById('benchmark-hashrate');
        if (hrEl) hrEl.textContent = window.chart?.formatHashrate(data.hashrate || 0) || '0 H/s';

        const hashesEl = document.getElementById('benchmark-hashes');
        if (hashesEl) hashesEl.textContent = formatNumber(data.total_hashes || 0);

        const timeEl = document.getElementById('benchmark-time');
        if (timeEl) timeEl.textContent = (data.duration || 0).toFixed(1) + 's';

        const cpuHashrate = (data.hashrate || 0) - (data.gpu_hashrate || 0);
        const cpuHrEl = document.getElementById('benchmark-cpu-hashrate');
        if (cpuHrEl) cpuHrEl.textContent = window.chart?.formatHashrate(cpuHashrate > 0 ? cpuHashrate : data.hashrate || 0) || '0 H/s';

        const gpuRow = document.getElementById('benchmark-gpu-row');
        if (data.gpu_hashrate !== undefined && data.gpu_hashrate > 0) {
            if (gpuRow) gpuRow.style.display = 'flex';
            const gpuHrEl = document.getElementById('benchmark-gpu-hashrate');
            if (gpuHrEl) gpuHrEl.textContent = window.chart?.formatHashrate(data.gpu_hashrate) || '0 H/s';
        }

        if (data.state && data.state !== 'benchmarking' && minerState === 'benchmarking') {
            updateMinerState(data.state);
            if (benchmarkInterval) {
                clearInterval(benchmarkInterval);
                benchmarkInterval = null;
            }
            addLog('Benchmark completed', 'success');
        }
    } catch (e) {
        console.error('Failed to get benchmark results:', e);
    }
}

// Fetch logs from backend
async function fetchLogs() {
    try {
        const res = await fetch('/api/logs');
        const data = await res.json();

        if (data.logs && Array.isArray(data.logs)) {
            // Clear existing logs and add backend logs
            if (logOutput) {
                logOutput.innerHTML = '';
                data.logs.forEach(log => {
                    const time = new Date(log.timestamp * 1000).toLocaleTimeString();
                    const entry = document.createElement('div');
                    entry.className = 'log-entry';
                    entry.innerHTML = `<span class="log-time">[${time}]</span> <span class="log-${log.level}">${log.message}</span>`;
                    logOutput.appendChild(entry);
                });
                if (logAutoScroll) {
                    logOutput.scrollTop = logOutput.scrollHeight;
                }
            }
        }
    } catch (e) {
        console.error('Failed to fetch logs:', e);
    }
}

// Settings functions
async function loadSettings() {
    try {
        // Load from /api/info for basic config
        const infoRes = await fetch('/api/info');
        const info = await infoRes.json();

        // Load idle state
        const idleRes = await fetch('/api/idle');
        const idle = await idleRes.json();

        // Update form fields with available data
        if (info.pool) {
            const poolHost = document.getElementById('pool-host');
            if (poolHost) poolHost.value = info.pool;

            // Detect preset from pool
            let detectedPreset = 'custom';
            for (const [key, config] of Object.entries(poolPresets)) {
                if (info.pool.includes(config.host) && config.host) {
                    detectedPreset = key;
                    break;
                }
            }
            const presetSelect = document.getElementById('solo-pool-preset');
            if (presetSelect) presetSelect.value = detectedPreset;
        }

        if (info.workers) {
            const threadsInput = document.getElementById('threads-input');
            if (threadsInput) threadsInput.value = info.workers;
        }

        // Update idle settings
        if (idle) {
            const idleEnabled = document.getElementById('idle-enabled-setting');
            if (idleEnabled) idleEnabled.checked = idle.enabled;

            const idleTimeout = document.getElementById('idle-timeout-setting');
            if (idleTimeout) idleTimeout.value = Math.floor((idle.timeout || 900) / 60);

            // Also update dashboard idle timeout
            const dashIdleTimeout = document.getElementById('idle-timeout');
            if (dashIdleTimeout) dashIdleTimeout.value = Math.floor((idle.timeout || 900) / 60);
        }

        updatePoolPreset();
    } catch (e) {
        console.error('Failed to load settings:', e);
    }
}

async function saveSettings() {
    const address = document.getElementById('payout-address')?.value || '';

    if (address && !validateAddress(address)) {
        addLog('Please fix validation errors before saving', 'error');
        return;
    }

    const preset = document.getElementById('solo-pool-preset')?.value;
    let pool = document.getElementById('pool-host')?.value || '';

    if (preset && preset !== 'custom' && preset !== 'none' && poolPresets[preset]) {
        const config = poolPresets[preset];
        pool = `${config.host}:${config.port}`;
    }

    const settings = {
        pool: pool,
        address: address,
        threads: parseInt(document.getElementById('threads-input')?.value) || 0,
        enable_cpu: document.getElementById('enable-cpu')?.checked ?? true,
        enable_gpu: document.getElementById('enable-gpu')?.checked ?? false,
    };

    try {
        const res = await fetch('/api/settings', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(settings)
        });

        if (res.ok) {
            addLog('Settings saved successfully', 'success');
        } else {
            const errData = await res.json().catch(() => ({}));
            addLog('Failed to save settings: ' + (errData.error || 'Unknown error'), 'error');
        }
    } catch (e) {
        addLog('Failed to save settings: ' + e.message, 'error');
    }
}

async function fetchMinerState() {
    try {
        const res = await fetch('/api/stats');
        const data = await res.json();
        if (data.state) updateMinerState(data.state);

        if (data.idle_mining_enabled !== undefined) {
            window.idle?.updateIdleUI(data.idle_mining_enabled, data.user_idle, data.idle_mining_active);
        }
    } catch (e) {
        console.error('Failed to fetch miner state:', e);
    }
}

// Floating control panel toggle
function initFloatingControl() {
    const toggleBtn = document.getElementById('control-toggle');
    const floatingControl = document.getElementById('floating-control');

    if (toggleBtn && floatingControl) {
        toggleBtn.addEventListener('click', () => {
            floatingControl.classList.toggle('collapsed');
            toggleBtn.textContent = floatingControl.classList.contains('collapsed') ? '+' : '-';
        });
    }
}

// Initialize event listeners
function initEventListeners() {
    // Settings buttons
    const saveBtn = document.getElementById('save-settings');
    if (saveBtn) saveBtn.addEventListener('click', saveSettings);

    const reloadBtn = document.getElementById('reload-settings');
    if (reloadBtn) reloadBtn.addEventListener('click', loadSettings);

    const clearLogsBtn = document.getElementById('clear-logs');
    if (clearLogsBtn) {
        clearLogsBtn.addEventListener('click', () => {
            if (logOutput) logOutput.innerHTML = '';
        });
    }

    // Log auto-scroll
    const autoScrollCheckbox = document.getElementById('log-auto-scroll');
    if (autoScrollCheckbox) {
        autoScrollCheckbox.addEventListener('change', (e) => {
            logAutoScroll = e.target.checked;
        });
    }

    // Pool preset
    const presetSelect = document.getElementById('solo-pool-preset');
    if (presetSelect) presetSelect.addEventListener('change', updatePoolPreset);

    // CPU/GPU toggles
    const enableCpu = document.getElementById('enable-cpu');
    if (enableCpu) enableCpu.addEventListener('change', (e) => updateCpuSettings(e.target.checked));

    const enableGpu = document.getElementById('enable-gpu');
    if (enableGpu) enableGpu.addEventListener('change', (e) => updateGpuSettings(e.target.checked));

    // Address validation
    const payoutAddr = document.getElementById('payout-address');
    if (payoutAddr) payoutAddr.addEventListener('input', (e) => validateAddress(e.target.value));

    // Control buttons
    const btnStart = document.getElementById('btn-start');
    if (btnStart) btnStart.addEventListener('click', () => controlMiner('start'));

    const btnStop = document.getElementById('btn-stop');
    if (btnStop) btnStop.addEventListener('click', () => controlMiner('stop'));

    const btnPause = document.getElementById('btn-pause');
    if (btnPause) btnPause.addEventListener('click', () => controlMiner('pause'));

    const btnResume = document.getElementById('btn-resume');
    if (btnResume) btnResume.addEventListener('click', () => controlMiner('resume'));

    const btnBenchStart = document.getElementById('btn-benchmark-start');
    if (btnBenchStart) btnBenchStart.addEventListener('click', startBenchmark);

    const btnBenchStop = document.getElementById('btn-benchmark-stop');
    if (btnBenchStop) btnBenchStop.addEventListener('click', stopBenchmark);

    // Idle mining (dashboard)
    const idleToggle = document.getElementById('idle-toggle');
    if (idleToggle) {
        idleToggle.addEventListener('change', (e) => window.idle?.toggleIdleMining(e.target.checked));
    }

    const idleTimeoutInput = document.getElementById('idle-timeout');
    if (idleTimeoutInput) {
        idleTimeoutInput.addEventListener('change', () => {
            if (window.idle?.isEnabled()) window.idle.updateIdleTimeout();
        });
    }

    // Notification test
    const testNotifBtn = document.getElementById('test-notification');
    if (testNotifBtn) {
        testNotifBtn.addEventListener('click', () => window.notifications?.testNotification());
    }

    // Header notification toggle
    const notifToggle = document.getElementById('notification-toggle');
    if (notifToggle) {
        // Initialize state
        updateNotificationToggleUI();

        notifToggle.addEventListener('click', () => {
            window.notifications?.toggle();
            updateNotificationToggleUI();
        });
    }
}

// Update notification toggle button UI
function updateNotificationToggleUI() {
    const notifToggle = document.getElementById('notification-toggle');
    if (!notifToggle) return;

    const enabled = window.notifications?.isEnabled() ?? false;
    const icon = notifToggle.querySelector('.notification-icon');

    if (enabled) {
        notifToggle.classList.remove('disabled');
        notifToggle.title = 'Notifications Enabled - Click to disable';
        if (icon) icon.textContent = '🔔';
    } else {
        notifToggle.classList.add('disabled');
        notifToggle.title = 'Notifications Disabled - Click to enable';
        if (icon) icon.textContent = '🔕';
    }
}

// Initialize application
function init() {
    initDom();
    initNavigation();
    initFloatingControl();
    initEventListeners();

    // Initialize notifications
    window.notifications?.init();

    // Connect WebSocket
    window.websocket?.connect();

    // Load initial data
    loadSettings();
    fetchMinerState();
    fetchLogs();
    window.idle?.fetchIdleStatus();
    window.chart?.drawChart();

    // Resize handler
    window.addEventListener('resize', () => window.chart?.drawChart());

    // Poll miner state periodically
    setInterval(() => {
        if (window.websocket?.isConnected()) {
            fetchMinerState();
        }
    }, 5000);

    // Poll logs periodically
    setInterval(fetchLogs, 10000);

    // Start idle polling if enabled
    window.idle?.fetchIdleStatus().then(() => {
        if (window.idle?.isEnabled()) {
            window.idle.startIdlePolling();
        }
    });

    addLog('Dashboard initialized', 'info');
}

// Export functions
window.app = {
    init,
    addLog,
    formatNumber,
    formatUptime,
    formatDuration,
    updateMinerState,
    controlMiner,
    loadSettings,
    saveSettings
};

// Initialize on load
window.addEventListener('load', init);
