// BTC Miner Dashboard - Chart Module

// Chart state
let hashrateHistory = [];
const maxHistoryPoints = 60;

// Export for use by other modules
window.chartState = {
    hashrateHistory,
    maxHistoryPoints
};

// Format hashrate for display
function formatHashrate(hr) {
    if (hr >= 1e15) return (hr / 1e15).toFixed(2) + ' PH/s';
    if (hr >= 1e12) return (hr / 1e12).toFixed(2) + ' TH/s';
    if (hr >= 1e9) return (hr / 1e9).toFixed(2) + ' GH/s';
    if (hr >= 1e6) return (hr / 1e6).toFixed(2) + ' MH/s';
    if (hr >= 1e3) return (hr / 1e3).toFixed(2) + ' KH/s';
    return hr.toFixed(0) + ' H/s';
}

// Draw hashrate chart
function drawChart() {
    const canvas = document.getElementById('hashrate-chart');
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    const rect = canvas.parentElement.getBoundingClientRect();

    canvas.width = rect.width - 48;
    canvas.height = 250;

    const w = canvas.width;
    const h = canvas.height;
    const padding = { top: 20, right: 20, bottom: 30, left: 70 };
    const chartW = w - padding.left - padding.right;
    const chartH = h - padding.top - padding.bottom;

    // Clear
    ctx.fillStyle = '#242424';
    ctx.fillRect(0, 0, w, h);

    if (hashrateHistory.length < 2) {
        ctx.fillStyle = '#888';
        ctx.font = '14px sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText('Collecting data...', w / 2, h / 2);
        return;
    }

    // Find max value
    const maxVal = Math.max(...hashrateHistory, 1);

    // Draw grid
    ctx.strokeStyle = '#333';
    ctx.lineWidth = 1;
    for (let i = 0; i <= 4; i++) {
        const y = padding.top + (chartH * i / 4);
        ctx.beginPath();
        ctx.moveTo(padding.left, y);
        ctx.lineTo(w - padding.right, y);
        ctx.stroke();

        // Y labels
        const val = maxVal * (1 - i / 4);
        ctx.fillStyle = '#888';
        ctx.font = '12px sans-serif';
        ctx.textAlign = 'right';
        ctx.fillText(formatHashrate(val), padding.left - 10, y + 4);
    }

    // Draw line
    ctx.beginPath();
    ctx.strokeStyle = '#f7931a';
    ctx.lineWidth = 2;

    hashrateHistory.forEach((val, i) => {
        const x = padding.left + (i / (maxHistoryPoints - 1)) * chartW;
        const y = padding.top + chartH - (val / maxVal) * chartH;

        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
    });
    ctx.stroke();

    // Draw gradient fill
    const gradient = ctx.createLinearGradient(0, padding.top, 0, h - padding.bottom);
    gradient.addColorStop(0, 'rgba(247, 147, 26, 0.3)');
    gradient.addColorStop(1, 'rgba(247, 147, 26, 0)');

    ctx.beginPath();
    hashrateHistory.forEach((val, i) => {
        const x = padding.left + (i / (maxHistoryPoints - 1)) * chartW;
        const y = padding.top + chartH - (val / maxVal) * chartH;

        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
    });
    ctx.lineTo(padding.left + chartW, h - padding.bottom);
    ctx.lineTo(padding.left, h - padding.bottom);
    ctx.closePath();
    ctx.fillStyle = gradient;
    ctx.fill();
}

// Add hashrate data point
function addHashrateData(hashrate) {
    hashrateHistory.push(hashrate);
    if (hashrateHistory.length > maxHistoryPoints) {
        hashrateHistory.shift();
    }
    drawChart();
}

// Export functions
window.chart = {
    formatHashrate,
    drawChart,
    addHashrateData,
    getHistory: () => hashrateHistory,
    clearHistory: () => { hashrateHistory = []; }
};
