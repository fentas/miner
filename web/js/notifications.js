// BTC Miner Dashboard - Web Notifications Module

// Notification settings
let notificationSettings = {
    enabled: false,
    newBlock: true,
    foundBlock: true,
    shareAccepted: false,
    shareRejected: true,
    error: true,
    connectionLost: true,
    idleMiningStart: false
};

// Check if notifications are supported
const notificationsSupported = 'Notification' in window;

// Request notification permission
async function requestPermission() {
    if (!notificationsSupported) {
        console.warn('Web notifications not supported');
        return false;
    }

    if (Notification.permission === 'granted') {
        notificationSettings.enabled = true;
        return true;
    }

    if (Notification.permission !== 'denied') {
        const permission = await Notification.requestPermission();
        notificationSettings.enabled = permission === 'granted';
        return notificationSettings.enabled;
    }

    return false;
}

// Show notification
function showNotification(title, options = {}) {
    if (!notificationSettings.enabled) return;
    if (!notificationsSupported) return;
    if (Notification.permission !== 'granted') return;

    const defaults = {
        icon: 'data:image/svg+xml,<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><text y=".9em" font-size="90">₿</text></svg>',
        badge: 'data:image/svg+xml,<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><text y=".9em" font-size="90">₿</text></svg>',
        silent: false,
        requireInteraction: false
    };

    const notification = new Notification(title, { ...defaults, ...options });

    // Auto-close after 5 seconds unless it requires interaction
    if (!options.requireInteraction) {
        setTimeout(() => notification.close(), 5000);
    }

    return notification;
}

// Notification helpers for different events
function notifyNewBlock(height) {
    if (!notificationSettings.newBlock) return;
    showNotification('New Block Detected', {
        body: `Block ${height} found on network. Mining job updated.`,
        tag: 'new-block'
    });
}

function notifyFoundBlock(height, hash) {
    if (!notificationSettings.foundBlock) return;
    // Play system bell sound
    const audio = new Audio('data:audio/wav;base64,UklGRl9vT19XQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YU');
    audio.play().catch(() => {});

    showNotification('BLOCK FOUND!', {
        body: `You found block ${height}!\nHash: ${hash?.substring(0, 16)}...`,
        tag: 'found-block',
        requireInteraction: true,
        silent: false
    });
}

function notifyShareAccepted(count) {
    if (!notificationSettings.shareAccepted) return;
    showNotification('Share Accepted', {
        body: `Share #${count} accepted by pool`,
        tag: 'share-accepted'
    });
}

function notifyShareRejected(count, reason) {
    if (!notificationSettings.shareRejected) return;
    showNotification('Share Rejected', {
        body: `Share #${count} rejected${reason ? ': ' + reason : ''}`,
        tag: 'share-rejected'
    });
}

function notifyError(message) {
    if (!notificationSettings.error) return;
    showNotification('Mining Error', {
        body: message,
        tag: 'error'
    });
}

function notifyConnectionLost() {
    if (!notificationSettings.connectionLost) return;
    showNotification('Connection Lost', {
        body: 'Lost connection to miner. Attempting to reconnect...',
        tag: 'connection-lost'
    });
}

function notifyIdleMiningStart() {
    if (!notificationSettings.idleMiningStart) return;
    showNotification('Idle Mining Started', {
        body: 'User is idle - mining has started automatically',
        tag: 'idle-mining'
    });
}

// Load settings from localStorage
function loadNotificationSettings() {
    const saved = localStorage.getItem('btcminer-notifications');
    if (saved) {
        try {
            const parsed = JSON.parse(saved);
            notificationSettings = { ...notificationSettings, ...parsed };
        } catch (e) {
            console.error('Failed to load notification settings:', e);
        }
    }

    // Check permission status
    if (notificationsSupported && Notification.permission === 'granted') {
        notificationSettings.enabled = true;
    }

    updateNotificationUI();
}

// Save settings to localStorage
function saveNotificationSettings() {
    localStorage.setItem('btcminer-notifications', JSON.stringify(notificationSettings));
}

// Update UI to reflect settings
function updateNotificationUI() {
    const toggles = {
        'notify-enabled': 'enabled',
        'notify-new-block': 'newBlock',
        'notify-found-block': 'foundBlock',
        'notify-share-accepted': 'shareAccepted',
        'notify-share-rejected': 'shareRejected',
        'notify-error': 'error',
        'notify-connection-lost': 'connectionLost',
        'notify-idle-mining': 'idleMiningStart'
    };

    for (const [id, key] of Object.entries(toggles)) {
        const el = document.getElementById(id);
        if (el) {
            el.checked = notificationSettings[key];
        }
    }

    // Update permission status display
    const statusEl = document.getElementById('notification-status');
    if (statusEl) {
        if (!notificationsSupported) {
            statusEl.textContent = 'Not supported';
            statusEl.style.color = 'var(--danger)';
        } else if (Notification.permission === 'granted') {
            statusEl.textContent = 'Enabled';
            statusEl.style.color = 'var(--success)';
        } else if (Notification.permission === 'denied') {
            statusEl.textContent = 'Blocked';
            statusEl.style.color = 'var(--danger)';
        } else {
            statusEl.textContent = 'Not enabled';
            statusEl.style.color = 'var(--text-secondary)';
        }
    }
}

// Initialize notification event listeners
function initNotificationListeners() {
    // Main toggle
    const enabledToggle = document.getElementById('notify-enabled');
    if (enabledToggle) {
        enabledToggle.addEventListener('change', async (e) => {
            if (e.target.checked) {
                const granted = await requestPermission();
                if (!granted) {
                    e.target.checked = false;
                    window.app?.addLog('Notification permission denied', 'error');
                }
            } else {
                notificationSettings.enabled = false;
            }
            saveNotificationSettings();
            updateNotificationUI();
        });
    }

    // Individual toggles
    const toggleMap = {
        'notify-new-block': 'newBlock',
        'notify-found-block': 'foundBlock',
        'notify-share-accepted': 'shareAccepted',
        'notify-share-rejected': 'shareRejected',
        'notify-error': 'error',
        'notify-connection-lost': 'connectionLost',
        'notify-idle-mining': 'idleMiningStart'
    };

    for (const [id, key] of Object.entries(toggleMap)) {
        const el = document.getElementById(id);
        if (el) {
            el.addEventListener('change', (e) => {
                notificationSettings[key] = e.target.checked;
                saveNotificationSettings();
            });
        }
    }

    // Test notification button
    const testBtn = document.getElementById('test-notification');
    if (testBtn) {
        testBtn.addEventListener('click', () => {
            if (notificationSettings.enabled) {
                showNotification('Test Notification', {
                    body: 'Notifications are working correctly!',
                    tag: 'test'
                });
            } else {
                window.app?.addLog('Please enable notifications first', 'error');
            }
        });
    }
}

// Export functions
window.notifications = {
    init: () => {
        loadNotificationSettings();
        initNotificationListeners();
    },
    requestPermission,
    showNotification,
    notifyNewBlock,
    notifyFoundBlock,
    notifyShareAccepted,
    notifyShareRejected,
    notifyError,
    notifyConnectionLost,
    notifyIdleMiningStart,
    getSettings: () => notificationSettings,
    setSettings: (settings) => {
        notificationSettings = { ...notificationSettings, ...settings };
        saveNotificationSettings();
        updateNotificationUI();
    }
};
