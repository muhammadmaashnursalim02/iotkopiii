'use strict';
(() => {
  const $ = id => document.getElementById(id);
  const sensors = [
    ['MQ-3', '#dfb483'], ['MQ-6', '#c1ca9b'], ['MQ-7', '#df9980'], ['MQ-135', '#8ab9aa'],
    ['TGS2600', '#c7bbac'], ['TGS2602', '#a6b5d2'], ['TGS2611', '#ba9ab2'], ['TGS2620', '#d0bd6f']
  ];
  const devices = ['DAQ01', 'DAQ02', 'DAQ03'];
  let rows = [];
  let inspectionToken = 0;
  const visible = new Set(sensors.map((_, index) => index));

  sensors.forEach(([name, color], i) => {
    const label = document.createElement('label');
    label.className = 'sensor-toggle';
    label.style.setProperty('--channel', color);
    label.innerHTML = `<span class="sensor-name"><b>S${i + 1}</b> ${name}</span><input type="checkbox" checked aria-label="Tampilkan S${i + 1} ${name}" data-sensor="${i}"><span class="sensor-value"><i class="swatch" aria-hidden="true"></i><span id="sensorValue${i}">— V</span></span>`;
    label.querySelector('input').addEventListener('change', event => {
      event.target.checked ? visible.add(i) : visible.delete(i);
      renderChart();
    });
    $('sensorGrid').append(label);
  });

  function renderChart() {
    const chartWidth = Math.max(280, Math.round($('sensorChart').getBoundingClientRect().width));
    const right = chartWidth - 14;
    $('sensorChart').setAttribute('viewBox', `0 0 ${chartWidth} 300`);
    const x = t => 32 + t / 300 * (right - 32);
    const y = value => 262 - value / 5 * 240;
    let markup = '<title>Respons MOS — tegangan (V) terhadap waktu relatif (s)</title>';
    for (let value = 0; value <= 5; value++) {
      markup += `<line class="gridline" x1="32" x2="${right}" y1="${y(value)}" y2="${y(value)}"/><text x="23" y="${y(value) + 4}" text-anchor="end">${value}</text>`;
    }
    for (let t = 0; t <= 300; t += 60) {
      markup += `<text x="${x(t)}" y="287" text-anchor="middle">${t}s</text>`;
    }
    for (const i of visible) {
      if (!rows.length) continue;
      const points = rows.map(row => `${x(row.t).toFixed(2)},${y(row.sensors[i]).toFixed(2)}`).join(' ');
      markup += `<polyline data-channel="${i}" stroke="${sensors[i][1]}" points="${points}"><title>S${i + 1} ${sensors[i][0]} — simulasi</title></polyline>`;
    }
    $('sensorChart').innerHTML = markup;
    $('chartEmpty').hidden = Boolean(rows.length && visible.size);
    if (rows.length) {
      $('chartEmpty').innerHTML = '<strong>Tidak ada kanal dipilih</strong><span>Centang sensor di bawah untuk menampilkan kurva.</span>';
    } else {
      $('chartEmpty').innerHTML = '<strong>Menunggu sumber data</strong><span>Pilih Demo untuk menjelajahi contoh respons sensor.</span>';
    }
  }

  function renderPredictions(latest) {
    const labels = ['Spesies', 'Status sangrai', 'ID sampel'];
    $('predictions').innerHTML = labels.map((label, i) => `<div class="prediction-row"><span class="level">L${i + 1}</span><div><div class="prediction-label">${label}</div><div class="prediction-value">${latest ? latest.predictions[i] : '—'}</div></div><div class="confidence"><small>Confidence</small><strong>${latest ? latest.confidence[i] + '%' : '—'}</strong></div></div>`).join('');
    $('predictionSource').textContent = latest ? 'DEMO' : 'TANPA DATA';
  }

  function render() {
    const demo = $('dataMode').value === 'demo';
    const latest = rows.at(-1);
    $('connectionStatus').textContent = demo ? 'Demo · simulasi' : 'Belum terhubung';
    $('connectionStatus').classList.toggle('demo', demo);
    $('modeNotice').textContent = demo
      ? 'DEMO — Semua nilai, kurva, kelas, dan confidence adalah simulasi lokal. Sesi tetap 0–300 detik; bukan stream perangkat.'
      : 'Live belum terhubung ke sumber telemetri. Tidak ada pembacaan atau prediksi aktual.';
    $('chartDevice').textContent = $('deviceSelect').value;
    $('lastUpdate').textContent = demo ? 'Sesi Demo tetap · interval 5 detik' : 'Belum ada data diterima';
    $('tempVal').textContent = latest ? latest.temperature.toFixed(1) : '—';
    $('humVal').textContent = latest ? latest.humidity.toFixed(1) : '—';
    sensors.forEach((_, i) => { $(`sensorValue${i}`).textContent = latest ? latest.sensors[i].toFixed(3) + ' V' : '— V'; });
    $('sampleCount').textContent = `${rows.length} sampel${demo ? ' Demo' : ''}`;
    $('chartCaption').textContent = demo ? `${rows.length} sampel Demo · 0–300 detik · interval 5 s` : 'Tidak ada sampel · sumbu waktu relatif (detik)';
    $('exportCsv').disabled = !rows.length;
    $('exportStatus').textContent = '';
    $('logTableBody').innerHTML = rows.length ? rows.map(row => `<tr><td>${row.t}</td><td>${row.temperature.toFixed(1)}</td><td>${row.humidity.toFixed(1)}</td>${row.sensors.map(v => `<td>${v.toFixed(3)}</td>`).join('')}${row.predictions.map((v, i) => `<td>${v} / ${row.confidence[i]}%</td>`).join('')}</tr>`).join('') : '<tr><td colspan="14" class="empty-row">Belum ada pembacaan. Data Live tidak disimulasikan.</td></tr>';
    renderPredictions(latest);
    renderChart();
  }

  // Deterministic, explicitly labelled demo fixture. Never used in Live mode.
  function loadSource() {
    const deviceIndex = devices.indexOf($('deviceSelect').value);
    rows = $('dataMode').value === 'demo' ? Array.from({ length: 61 }, (_, n) => {
      const t = n * 5;
      return {
        t,
        temperature: Number((28.4 + deviceIndex * 0.7 + Math.sin(n / 9) * 0.3).toFixed(1)),
        humidity: Number((61.2 + deviceIndex * 2 + Math.cos(n / 11) * 0.6).toFixed(1)),
        sensors: sensors.map((_, i) => Number((0.7 + i * 0.14 + deviceIndex * 0.08 + (t < 45 ? 0 : (1.0 + (i % 3) * 0.55) * (1 - Math.exp(-(t - 45) / (35 + i * 8)))) + Math.sin(n / 5 + i) * 0.018).toFixed(3))),
        predictions: [['Arabica', 'Roasted', 'C09'], ['Robusta', 'Roasted', 'C02'], ['Arabica', 'Green Beans', 'C01']][deviceIndex],
        confidence: [[96, 91, 78], [93, 87, 74], [94, 90, 81]][deviceIndex]
      };
    }) : [];
    render();
  }
  $('deviceSelect').addEventListener('change', loadSource);
  $('dataMode').addEventListener('change', loadSource);

  const tabs = [$('monitorTab'), $('otaTab')];
  function activateTab(tab, focus = false) {
    tabs.forEach(item => {
      const selected = item === tab;
      item.setAttribute('aria-selected', String(selected));
      item.tabIndex = selected ? 0 : -1;
      $(item.getAttribute('aria-controls')).hidden = !selected;
    });
    if (focus) tab.focus();
  }
  tabs.forEach((tab, i) => {
    tab.addEventListener('click', () => activateTab(tab));
    tab.addEventListener('keydown', event => {
      if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
      event.preventDefault();
      activateTab(tabs[event.key === 'Home' ? 0 : event.key === 'End' ? 1 : 1 - i], true);
    });
  });

  $('exportCsv').addEventListener('click', () => {
    if (!rows.length || $('dataMode').value !== 'demo') return;
    const header = ['mode', 'device', 'elapsed_s', 'temperature_c', 'humidity_percent', ...sensors.map((s, i) => `S${i + 1}_${s[0]}_V`), 'L1_species', 'L1_confidence_percent', 'L2_roast', 'L2_confidence_percent', 'L3_sample_id', 'L3_confidence_percent'];
    const records = rows.map(row => ['DEMO', $('deviceSelect').value, row.t, row.temperature.toFixed(1), row.humidity.toFixed(1), ...row.sensors.map(v => v.toFixed(3)), ...row.predictions.flatMap((v, i) => [v, row.confidence[i]])]);
    const csv = [header, ...records].map(record => record.map(value => `"${String(value).replaceAll('"', '""')}"`).join(',')).join('\r\n');
    const url = URL.createObjectURL(new Blob(['\uFEFF', csv, '\r\n'], { type: 'text/csv;charset=utf-8' }));
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `coffee-lab-DEMO-${$('deviceSelect').value}.csv`;
    document.body.append(anchor);
    anchor.click();
    anchor.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
    $('exportStatus').textContent = `CSV Demo disiapkan: ${rows.length} baris, semua 8 sensor. Tidak berisi data Live.`;
  });

  function clearInspection() {
    inspectionToken++;
    $('fileInspection').hidden = true;
    $('fileInspection').replaceChildren();
    $('otaFeedback').textContent = '';
    $('otaFeedback').classList.remove('error');
    $('inspectFirmware').disabled = false;
  }
  ['otaDevice', 'fwVersion', 'fwFile'].forEach(id => $(id).addEventListener('input', clearInspection));
  $('otaForm').addEventListener('submit', async event => {
    event.preventDefault();
    clearInspection();
    const token = inspectionToken;
    const device = $('otaDevice').value;
    const version = $('fwVersion').value;
    const file = $('fwFile').files[0];
    const errors = [];
    if (!devices.includes(device)) errors.push('Pilih perangkat tujuan yang valid.');
    if (!/^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:[-_][A-Za-z0-9][A-Za-z0-9._-]*)?$/.test(version) || version.length > 64) errors.push('Versi harus memakai format major.minor.patch, misalnya v2.0.1_svm.');
    if (!file) errors.push('Pilih berkas firmware .bin.');
    else {
      if (!/\.bin$/i.test(file.name)) errors.push('Ekstensi berkas harus .bin.');
      if (!file.size) errors.push('Berkas kosong tidak dapat diperiksa.');
      if (file.size > 16 * 1024 * 1024) errors.push('Berkas melebihi batas pemeriksaan lokal 16 MiB.');
    }
    if (errors.length) {
      $('otaFeedback').classList.add('error');
      $('otaFeedback').textContent = errors.join(' ');
      return;
    }
    $('inspectFirmware').disabled = true;
    $('otaFeedback').textContent = 'Membaca berkas lokal…';
    try {
      const buffer = await file.arrayBuffer();
      const firstBytes = Array.from(new Uint8Array(buffer, 0, Math.min(16, buffer.byteLength)), byte => byte.toString(16).padStart(2, '0')).join(' ');
      let hash = 'Tidak tersedia: browser memerlukan secure context untuk SHA-256.';
      if (globalThis.crypto?.subtle) {
        hash = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', buffer)), byte => byte.toString(16).padStart(2, '0')).join('');
      }
      if (token !== inspectionToken) return;
      const metadata = [['Perangkat', device], ['Versi', version], ['Nama berkas', file.name], ['Ukuran', `${file.size} byte`], ['16 byte awal', firstBytes], ['SHA-256', hash]];
      metadata.forEach(([key, value]) => {
        const term = document.createElement('dt');
        const description = document.createElement('dd');
        term.textContent = key;
        description.textContent = value;
        $('fileInspection').append(term, description);
      });
      $('fileInspection').hidden = false;
      $('otaFeedback').textContent = 'Pemeriksaan lokal selesai. Isi firmware belum diverifikasi kompatibel. Tidak ada berkas diunggah; backend OTA belum terhubung.';
    } catch (error) {
      if (token !== inspectionToken) return;
      $('otaFeedback').classList.add('error');
      $('otaFeedback').textContent = 'Berkas tidak dapat dibaca. Pilih ulang berkas lokal dan coba lagi. Tidak ada unggahan.';
    } finally {
      if (token === inspectionToken) $('inspectFirmware').disabled = false;
    }
  });
  loadSource();
  new ResizeObserver(() => {
    if (!$('monitorPanel').hidden) renderChart();
  }).observe($('sensorChart'));
})();
