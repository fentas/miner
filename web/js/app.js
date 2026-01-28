// BTC Miner Dashboard - Main Application

// Application state
let minerState = 'stopped';
let benchmarkInterval = null;
let startTime = Date.now();

// Pool presets configuration
const poolPresets = {
    ckpool: {
        host: 'solo.ckpool.org',
        port: 3333,
        stratum: 'v1',
        description: 'Solo mining pool with Stratum V1. 2% fee on successful block rewards. Reliable and widely used.'
    },
    braiins: {
        host: 'solo.stratum.braiins.com',
        port: 3333,
        stratum: 'v1',
        description: 'Braiins Solo mining pool with Stratum V1. 0.5% fee on found blocks (paid to CKPool authors).'
    },
    ocean: {
        host: 'mine.ocean.xyz',
        port: 3334,
        stratum: 'v1',
        description: 'Ocean Pool with DATUM support. 0% pool fee - miners only pay Bitcoin transaction fees. Supports custom block templates.'
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
    } else {
        const days = Math.floor(seconds / 86400);
        const hours = Math.floor((seconds % 86400) / 3600);
        return days + 'd ' + hours + 'h';
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
    logOutput.scrollTop = logOutput.scrollHeight;

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
            tab.style.display = 'block';
            tab.classList.add('active');
        });
    });
}

// Pool preset handling
function updatePoolPreset() {
    const preset = document.getElementById('solo-pool-preset').value;
    const poolHostInput = document.getElementById('pool-host');
    const stratumBadge = document.getElementById('stratum-badge');
    const presetInfo = document.getElementById('pool-preset-info');

    if (preset === 'custom') {
        poolHostInput.disabled = false;
        poolHostInput.placeholder = 'host:port';
        stratumBadge.className = 'stratum-badge v1';
        stratumBadge.textContent = 'V1';
        presetInfo.classList.remove('visible');
    } else {
        const config = poolPresets[preset];
        poolHostInput.value = `${config.host}:${config.port}`;
        poolHostInput.disabled = true;
        stratumBadge.className = `stratum-badge ${config.stratum}`;
        stratumBadge.textContent = config.stratum.toUpperCase();
        presetInfo.textContent = config.description;
        presetInfo.classList.add('visible');
    }
}

// Address validation
function validateAddress(address) {
    const validationEl = document.getElementById('address-validation');

    if (!address) {
        validationEl.className = 'validation-message error';
        validationEl.textContent = 'Bitcoin address is required';
        return false;
    }

    const legacy = /^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$/;
    const bech32 = /^bc1[a-z0-9]{39,59}$/i;
    const bech32m = /^bc1p[a-z0-9]{58}$/i;

    if (legacy.test(address)) {
        validationEl.className = 'validation-message success';
        validationEl.textContent = 'Valid legacy address';
        return true;
    } else if (bech32.test(address) || bech32m.test(address)) {
        validationEl.className = 'validation-message success';
        validationEl.textContent = 'Valid SegWit address';
        return true;
    } else if (address.length < 26) {
        validationEl.className = 'validation-message error';
        validationEl.textContent = 'Address is too short';
        return false;
    } else {
        validationEl.className = 'validation-message warning';
        validationEl.textContent = 'Address format not recognized - verify before saving';
        return true;
    }
}

// Mining mode validation
function validateMiningMode() {
    const enableCpu = document.getElementById('enable-cpu').checked;
    const enableGpu = document.getElementById('enable-gpu').checked;
    const validationEl = document.getElementById('mining-mode-validation');

    if (!enableCpu && !enableGpu) {
        validationEl.className = 'validation-message error';
        validationEl.textContent = 'At least one mining mode must be enabled (CPU or GPU)';
        return false;
    } else {
        validationEl.className = 'validation-message';
        validationEl.textContent = '';
        return true;
    }
}

// Update UI based on settings
function updateCpuStatus(enabled) {
    const cpuThreadsRow = document.getElementById('cpu-threads-row');
    if (cpuThreadsRow) {
        cpuThreadsRow.style.display = enabled ? 'grid' : 'none';
    }
    validateMiningMode();
}

function updateGpuStatus(enabled) {
    const gpuStatus = document.getElementById('gpu-status');
    if (gpuStatus) gpuStatus.style.display = enabled ? 'block' : 'none';

    const gpuCard = document.getElementById('gpu-hashrate-card');
    if (gpuCard) gpuCard.style.display = enabled ? 'block' : 'none';

    validateMiningMode();
}

function updateBitcoindStatus(enabled) {
    const bitcoindSettings = document.getElementById('bitcoind-settings');
    if (bitcoindSettings) {
        bitcoindSettings.style.display = enabled ? 'block' : 'none';
    }
}

function updateBitcoindManagedMode(managed) {
    const managedControls = document.getElementById('bitcoind-managed-controls');
    const manualSettings = document.getElementById('bitcoind-manual-settings');
    if (managedControls) managedControls.style.display = managed ? 'block' : 'none';
    if (manualSettings) manualSettings.style.display = managed ? 'none' : 'block';
}

// Miner state management
function updateMinerState(state) {
    minerState = state;
    const indicator = document.getElementById('state-indicator');
    const stateText = document.getElementById('state-text');

    indicator.className = 'state-indicator ' + state;
    stateText.textContent = state.charAt(0).toUpperCase() + state.slice(1);

    const btnStart = document.getElementById('btn-start');
    const btnStop = document.getElementById('btn-stop');
    const btnPause = document.getElementById('btn-pause');
    const btnResume = document.getElementById('btn-resume');
    const btnBenchStart = document.getElementById('btn-benchmark-start');
    const btnBenchStop = document.getElementById('btn-benchmark-stop');

    switch (state) {
        case 'stopped':
            btnStart.disabled = false;
            btnStop.disabled = true;
            btnPause.disabled = true;
            btnResume.disabled = true;
            btnBenchStart.disabled = false;
            btnBenchStop.disabled = true;
            break;
        case 'running':
            btnStart.disabled = true;
            btnStop.disabled = false;
            btnPause.disabled = false;
            btnResume.disabled = true;
            btnBenchStart.disabled = false;
            btnBenchStop.disabled = true;
            break;
        case 'paused':
            btnStart.disabled = true;
            btnStop.disabled = false;
            btnPause.disabled = true;
            btnResume.disabled = false;
            btnBenchStart.disabled = true;
            btnBenchStop.disabled = true;
            break;
        case 'benchmarking':
            btnStart.disabled = true;
            btnStop.disabled = true;
            btnPause.disabled = false;
            btnResume.disabled = true;
            btnBenchStart.disabled = true;
            btnBenchStop.disabled = false;
            break;
    }
}

// Miner control
async function controlMiner(action) {
    try {
        const res = await fetch(`/api/control/${action}`, { method: 'POST' });
        const data = await res.json();
        if (data.state) updateMinerState(data.state);
        addLog(`Miner ${action}: ${data.message || 'OK'}`, data.success ? 'success' : 'error');
    } catch (e) {
        addLog(`Failed to ${action} miner: ${e.message}`, 'error');
    }
}

// Benchmark functions
async function startBenchmark() {
    const duration = parseInt(document.getElementById('benchmark-duration').value) || 0;
    try {
        const res = await fetch(`/api/control/benchmark?duration=${duration}`, { method: 'POST' });
        const data = await res.json();
        if (data.state) updateMinerState(data.state);
        addLog(`Benchmark started${duration > 0 ? ' for ' + duration + 's' : ''}`, 'success');

        document.getElementById('benchmark-results').classList.add('active');

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

        document.getElementById('benchmark-hashrate').textContent = window.chart.formatHashrate(data.hashrate || 0);
        document.getElementById('benchmark-hashes').textContent = formatNumber(data.total_hashes || 0);
        document.getElementById('benchmark-time').textContent = (data.duration || 0).toFixed(1) + 's';

        const cpuHashrate = (data.hashrate || 0) - (data.gpu_hashrate || 0);
        document.getElementById('benchmark-cpu-hashrate').textContent = window.chart.formatHashrate(cpuHashrate > 0 ? cpuHashrate : data.hashrate || 0);

        const gpuRow = document.getElementById('benchmark-gpu-row');
        if (data.gpu_hashrate !== undefined && data.gpu_hashrate > 0) {
            gpuRow.style.display = 'flex';
            document.getElementById('benchmark-gpu-hashrate').textContent = window.chart.formatHashrate(data.gpu_hashrate);
        } else if (document.getElementById('enable-gpu').checked) {
            gpuRow.style.display = 'flex';
            document.getElementById('benchmark-gpu-hashrate').textContent = 'Initializing...';
        } else {
            gpuRow.style.display = 'none';
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

// Bitcoin Core functions
async function testBitcoindConnection() {
    const statusEl = document.getElementById('bitcoind-status-text');
    const blockEl = document.getElementById('bitcoind-block-height');
    const syncEl = document.getElementById('bitcoind-sync-progress');

    statusEl.textContent = 'Testing...';
    statusEl.style.color = 'var(--text-secondary)';

    try {
        const res = await fetch('/api/bitcoind/test', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                host: document.getElementById('bitcoind-host').value,
                port: parseInt(document.getElementById('bitcoind-rpc-port').value) || 8332,
                user: document.getElementById('bitcoind-rpc-user').value,
                password: document.getElementById('bitcoind-rpc-password').value
            })
        });

        const data = await res.json();

        if (data.success) {
            statusEl.textContent = 'Connected';
            statusEl.style.color = 'var(--success)';
            blockEl.textContent = data.block_height || '-';
            syncEl.textContent = data.sync_progress ? (data.sync_progress * 100).toFixed(1) + '%' : '100%';
            addLog('Bitcoin Core connection successful', 'success');
        } else {
            statusEl.textContent = 'Failed';
            statusEl.style.color = 'var(--danger)';
            blockEl.textContent = '-';
            syncEl.textContent = '-';
            addLog('Bitcoin Core connection failed: ' + (data.error || 'Unknown error'), 'error');
        }
    } catch (e) {
        statusEl.textContent = 'Error';
        statusEl.style.color = 'var(--danger)';
        blockEl.textContent = '-';
        syncEl.textContent = '-';
        addLog('Bitcoin Core test failed: ' + e.message, 'error');
    }
}

async function bitcoindAction(action) {
    const statusEl = document.getElementById('bitcoind-status-text');
    statusEl.textContent = action === 'install' ? 'Installing...' :
                           action === 'start' ? 'Starting...' : 'Stopping...';
    statusEl.style.color = 'var(--text-secondary)';

    try {
        const res = await fetch('/api/bitcoind/' + action, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                prune: parseInt(document.getElementById('bitcoind-prune').value) || 10000
            })
        });

        const data = await res.json();

        if (data.success) {
            statusEl.textContent = data.status || 'Success';
            statusEl.style.color = 'var(--success)';
            addLog('Bitcoin Core ' + action + ' successful', 'success');

            if (data.block_height) {
                document.getElementById('bitcoind-block-height').textContent = data.block_height;
            }
            if (data.sync_progress !== undefined) {
                document.getElementById('bitcoind-sync-progress').textContent =
                    (data.sync_progress * 100).toFixed(1) + '%';
            }
        } else {
            statusEl.textContent = 'Failed';
            statusEl.style.color = 'var(--danger)';
            addLog('Bitcoin Core ' + action + ' failed: ' + (data.error || 'Unknown error'), 'error');
        }
    } catch (e) {
        statusEl.textContent = 'Error';
        statusEl.style.color = 'var(--danger)';
        addLog('Bitcoin Core ' + action + ' failed: ' + e.message, 'error');
    }
}

// Settings functions
async function loadSettings() {
    try {
        const res = await fetch('/api/settings');
        const data = await res.json();

        const poolHost = data.pool || '';
        let detectedPreset = 'custom';
        for (const [key, config] of Object.entries(poolPresets)) {
            if (poolHost.includes(config.host)) {
                detectedPreset = key;
                break;
            }
        }

        document.getElementById('solo-pool-preset').value = detectedPreset;
        document.getElementById('pool-host').value = poolHost;
        document.getElementById('payout-address').value = data.address || '';
        document.getElementById('pool-password').value = data.password || 'x';
        document.getElementById('threads-input').value = data.threads || 0;
        document.getElementById('stats-interval').value = data.stats_interval || 10;
        document.getElementById('enable-cpu').checked = data.enable_cpu !== false;
        document.getElementById('enable-gpu').checked = data.enable_gpu || false;

        document.getElementById('bitcoind-enabled').checked = data.bitcoind_enabled || false;
        document.getElementById('bitcoind-managed').checked = data.bitcoind_managed || false;
        document.getElementById('bitcoind-host').value = data.bitcoind_host || '127.0.0.1';
        document.getElementById('bitcoind-rpc-port').value = data.bitcoind_rpc_port || 8332;
        document.getElementById('bitcoind-rpc-user').value = data.bitcoind_rpc_user || '';
        document.getElementById('bitcoind-rpc-password').value = data.bitcoind_rpc_password || '';
        document.getElementById('bitcoind-prune').value = data.bitcoind_prune || 10000;
        document.getElementById('bitcoind-sv2').checked = data.bitcoind_sv2 || false;

        updatePoolPreset();
        updateCpuStatus(data.enable_cpu !== false);
        updateGpuStatus(data.enable_gpu || false);
        updateBitcoindStatus(data.bitcoind_enabled || false);
        updateBitcoindManagedMode(data.bitcoind_managed || false);

        if (data.address) validateAddress(data.address);
    } catch (e) {
        addLog('Failed to load settings', 'error');
    }
}

async function saveSettings() {
    const address = document.getElementById('payout-address').value;

    if (!validateAddress(address)) {
        addLog('Please fix validation errors before saving', 'error');
        return;
    }

    if (!validateMiningMode()) {
        addLog('Please fix mining mode errors before saving', 'error');
        return;
    }

    const preset = document.getElementById('solo-pool-preset').value;
    let pool = document.getElementById('pool-host').value;
    let stratumVersion = 'v1';

    if (preset !== 'custom' && poolPresets[preset]) {
        const config = poolPresets[preset];
        pool = `${config.host}:${config.port}`;
        stratumVersion = config.stratum;
    }

    const settings = {
        pool: pool,
        address: address,
        password: document.getElementById('pool-password').value,
        threads: parseInt(document.getElementById('threads-input').value) || 0,
        stats_interval: parseInt(document.getElementById('stats-interval').value) || 10,
        enable_cpu: document.getElementById('enable-cpu').checked,
        enable_gpu: document.getElementById('enable-gpu').checked,
        stratum_version: stratumVersion,
        bitcoind_enabled: document.getElementById('bitcoind-enabled').checked,
        bitcoind_managed: document.getElementById('bitcoind-managed').checked,
        bitcoind_host: document.getElementById('bitcoind-host').value,
        bitcoind_rpc_port: parseInt(document.getElementById('bitcoind-rpc-port').value) || 8332,
        bitcoind_rpc_user: document.getElementById('bitcoind-rpc-user').value,
        bitcoind_rpc_password: document.getElementById('bitcoind-rpc-password').value,
        bitcoind_prune: parseInt(document.getElementById('bitcoind-prune').value) || 10000,
        bitcoind_sv2: document.getElementById('bitcoind-sv2').checked
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

async function saveAndRestart() {
    const settings = {
        pool: document.getElementById('pool-host').value,
        address: document.getElementById('payout-address').value,
        password: document.getElementById('pool-password').value,
        threads: parseInt(document.getElementById('threads-input').value) || 0,
        stats_interval: parseInt(document.getElementById('stats-interval').value) || 10
    };

    try {
        const res = await fetch('/api/control/restart', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(settings)
        });

        const data = await res.json();
        if (data.success) {
            addLog('Settings saved, miner restarting...', 'success');
        } else {
            addLog('Failed to restart: ' + (data.message || 'Unknown error'), 'error');
        }
    } catch (e) {
        addLog('Failed to save and restart: ' + e.message, 'error');
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

// Initialize event listeners
function initEventListeners() {
    // Settings buttons
    document.getElementById('save-settings').addEventListener('click', saveSettings);
    document.getElementById('save-restart').addEventListener('click', saveAndRestart);
    document.getElementById('reload-settings').addEventListener('click', loadSettings);
    document.getElementById('clear-logs').addEventListener('click', () => {
        if (logOutput) logOutput.innerHTML = '';
    });

    // Pool preset
    document.getElementById('solo-pool-preset').addEventListener('change', updatePoolPreset);

    // CPU/GPU toggles
    document.getElementById('enable-cpu').addEventListener('change', (e) => updateCpuStatus(e.target.checked));
    document.getElementById('enable-gpu').addEventListener('change', (e) => updateGpuStatus(e.target.checked));

    // Bitcoin Core
    document.getElementById('bitcoind-enabled').addEventListener('change', (e) => updateBitcoindStatus(e.target.checked));
    document.getElementById('btn-test-bitcoind').addEventListener('click', testBitcoindConnection);
    document.getElementById('bitcoind-managed').addEventListener('change', (e) => updateBitcoindManagedMode(e.target.checked));
    document.getElementById('btn-bitcoind-install').addEventListener('click', () => bitcoindAction('install'));
    document.getElementById('btn-bitcoind-start').addEventListener('click', () => bitcoindAction('start'));
    document.getElementById('btn-bitcoind-stop').addEventListener('click', () => bitcoindAction('stop'));

    // Address validation
    document.getElementById('payout-address').addEventListener('input', (e) => validateAddress(e.target.value));

    // Control buttons
    document.getElementById('btn-start').addEventListener('click', () => controlMiner('start'));
    document.getElementById('btn-stop').addEventListener('click', () => controlMiner('stop'));
    document.getElementById('btn-pause').addEventListener('click', () => controlMiner('pause'));
    document.getElementById('btn-resume').addEventListener('click', () => controlMiner('resume'));
    document.getElementById('btn-benchmark-start').addEventListener('click', startBenchmark);
    document.getElementById('btn-benchmark-stop').addEventListener('click', stopBenchmark);

    // Idle mining
    document.getElementById('idle-toggle').addEventListener('change', (e) => window.idle?.toggleIdleMining(e.target.checked));
    document.getElementById('idle-timeout').addEventListener('change', () => {
        if (window.idle?.isEnabled()) window.idle.updateIdleTimeout();
    });
}

// Initialize application
function init() {
    initDom();
    initNavigation();
    initEventListeners();

    // Initialize notifications
    window.notifications?.init();

    // Connect WebSocket
    window.websocket.connect();

    // Load initial data
    loadSettings();
    fetchMinerState();
    window.idle?.fetchIdleStatus();
    window.chart.drawChart();

    // Resize handler
    window.addEventListener('resize', window.chart.drawChart);

    // Poll miner state periodically
    setInterval(() => {
        if (window.websocket.isConnected()) {
            fetchMinerState();
        }
    }, 5000);

    // Start idle polling if enabled
    window.idle?.fetchIdleStatus().then(() => {
        if (window.idle?.isEnabled()) {
            window.idle.startIdlePolling();
        }
    });
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
