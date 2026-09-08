// Inisialisasi Chart.js
const ctx = document.getElementById('sensorChart').getContext('2d');

const labels = Array.from({length: 100}, (_, i) => i * 5); 

const generateCurve = (base, peak, delay) => {
    return labels.map(t => {
        if (t < 60) return base + (Math.random() * 0.05); 
        return base + ((peak - base) * (1 - Math.exp(-(t-60)/delay))) + (Math.random() * 0.02);
    });
};

const chart = new Chart(ctx, {
    type: 'line',
    data: {
        labels: labels,
        datasets: [
            { label: 'S1 (MQ-3)', data: generateCurve(1.6, 3.2, 50), borderColor: '#ffadad', borderWidth: 2, pointRadius: 0 },
            { label: 'S4 (MQ-135)', data: generateCurve(1.8, 2.5, 80), borderColor: '#caffbf', borderWidth: 2, pointRadius: 0 },
            { label: 'S6 (TGS2602)', data: generateCurve(0.9, 4.1, 40), borderColor: '#a0c4ff', borderWidth: 2, pointRadius: 0 },
            { label: 'S8 (TGS2620)', data: generateCurve(1.3, 3.0, 60), borderColor: '#ffc6ff', borderWidth: 2, pointRadius: 0 }
        ]
    },
    options: {
        responsive: true,
        maintainAspectRatio: false,
        animation: false,
        interaction: { mode: 'index', intersect: false },
        plugins: { legend: { labels: { color: '#8c7a6b' } } },
        scales: {
            x: { 
                title: { display: true, text: 'Time (s)', color: '#8c7a6b' },
                grid: { color: '#3d2f27' }, 
                ticks: { color: '#8c7a6b' } 
            },
            y: { 
                title: { display: true, text: 'Voltage (V)', color: '#8c7a6b' },
                grid: { color: '#3d2f27' }, 
                ticks: { color: '#8c7a6b' },
                min: 0, max: 5
            }
        }
    }
});

setInterval(() => {
    const now = new Date();
    document.getElementById('lastUpdate').innerText = `Last Update: ${now.toLocaleTimeString()}`;
    
    document.getElementById('tempVal').innerText = (31.8 + (Math.random() * 0.4 - 0.2)).toFixed(1) + '°C';
    document.getElementById('humVal').innerText = (67.1 + (Math.random() * 0.4 - 0.2)).toFixed(1) + '%';
}, 2000);