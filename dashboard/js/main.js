'use strict';
(() => {
  const $ = id => document.getElementById(id);

  // Theme Toggle (Switch)
  const themeToggle = $('themeToggle');
  if (themeToggle) {
    const prefersLight = window.matchMedia && window.matchMedia('(prefers-color-scheme: light)').matches;
    if (prefersLight) {
      document.documentElement.classList.add('light');
      themeToggle.checked = true;
    }
    themeToggle.addEventListener('change', () => {
      document.documentElement.classList.toggle('light', themeToggle.checked);
    });
  }

  const sensors = [
    ['MQ-3', '#dfb483'], ['MQ-6', '#c1ca9b'], ['MQ-7', '#df9980'], ['MQ-135', '#8ab9aa'],
    ['TGS2600', '#c7bbac'], ['TGS2602', '#a6b5d2'], ['TGS2611', '#ba9ab2'], ['TGS2620', '#d0bd6f']
  ];
  const devices = ['DAQ01', 'DAQ02', 'DAQ03'];
  let rows = [];
  let inspectionToken = 0;
  let generation = 0, pollTimer, activeRequest = null;
  let liveStatus = 'Belum terhubung';
  const sensorFields = ['s1_mq3','s2_mq6','s3_mq7','s4_mq135','s5_tgs2600','s6_tgs2602','s7_tgs2611','s8_tgs2620'];
  const number = (value, digits) => Number.isFinite(value) ? value.toFixed(digits) : '—';
  const escape = value => String(value ?? '—').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const percent = value => Number.isFinite(value) ? `${value}%` : '—';

  async function pollLive(token) {
    if (activeRequest || token !== generation || $('dataMode').value !== 'live') return;
    if (location.protocol === 'file:') { liveStatus = 'Belum terhubung'; render(); return; }
    const controller = new AbortController();
    activeRequest = controller;
    const timeout = setTimeout(() => controller.abort(), 10000);
    try {
      const params = new URLSearchParams({limit:'20'});
      if ($('deviceSelect').value) params.set('device_id', $('deviceSelect').value);
      const response = await fetch(`/api/dashboard?${params}`, {signal:controller.signal, cache:'no-store'});
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const data = await response.json();
      if (!Array.isArray(data.rows)) throw new Error('Invalid telemetry response');
      if (token !== generation) return;
      const start = Date.parse(data.rows[0]?.created_at);
      rows = data.rows.map(row => ({
        id:row.id, created_at:row.created_at, device:row.device_id, coffee_id:row.coffee_id,
        t:Number.isFinite(Date.parse(row.created_at)) && Number.isFinite(start) ? (Date.parse(row.created_at)-start)/1000 : 0,
        temperature:row.temperature, humidity:row.humidity,
        sensors:sensorFields.map(key => row[key]),
        predictions:[1,2,3].map(i => row[`pred_level_${i}`]),
        confidence:[1,2,3].map(i => Number.isFinite(row[`conf_level_${i}`]) ? Math.round(row[`conf_level_${i}`]*1000)/10 : null)
      }));
      liveStatus = rows.length ? 'Live · riwayat PostgreSQL' : 'Live · data kosong';
      render();
    } catch (error) {
      if (token !== generation) return;
      rows = [];
      liveStatus = 'Terputus · gagal membaca API';
      render();
    } finally {
      clearTimeout(timeout);
      activeRequest = null;
      if ($('dataMode').value === 'live') {
        const current = generation;
        pollTimer = setTimeout(() => pollLive(current), token === current ? 5000 : 0);
      }
    }
  }
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
    const duration = Math.max(1, rows.at(-1)?.t || 300);
    const x = t => 32 + t / duration * (right - 32);
    const y = value => 262 - value / 5 * 240;
    let markup = '<title>Respons MOS — tegangan (V) terhadap waktu relatif (s)</title>';
    for (let value = 0; value <= 5; value++) {
      markup += `<line class="gridline" x1="32" x2="${right}" y1="${y(value)}" y2="${y(value)}"/><text x="23" y="${y(value) + 4}" text-anchor="end">${value}</text>`;
    }
    for (let n = 0; n <= 5; n++) {
      const t = duration * n / 5;
      markup += `<text x="${x(t)}" y="287" text-anchor="middle">${Math.round(t)}s</text>`;
    }
    for (const i of visible) {
      if (!rows.length) continue;
      let segment = [];
      const flush = () => {
        if (!segment.length) return;
        markup += `<polyline data-channel="${i}" stroke="${sensors[i][1]}" points="${segment.join(' ')}"><title>S${i + 1} ${sensors[i][0]} — ${$('dataMode').value === 'demo' ? 'simulasi' : 'PostgreSQL'}</title></polyline>`;
        segment = [];
      };
      rows.forEach(row => {
        if (!Number.isFinite(row.sensors[i])) { flush(); return; }
        segment.push(`${x(row.t).toFixed(2)},${y(row.sensors[i]).toFixed(2)}`);
      });
      flush();
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
    $('predictions').innerHTML = labels.map((label, i) => `<div class="prediction-row"><span class="level">L${i + 1}</span><div><div class="prediction-label">${label}</div><div class="prediction-value">${escape(latest?.predictions[i])}</div></div><div class="confidence"><small>Confidence</small><strong>${latest ? percent(latest.confidence[i]) : '—'}</strong></div></div>`).join('');
    $('predictionSource').textContent = latest ? ($('dataMode').value === 'demo' ? 'DEMO' : 'POSTGRESQL') : 'TANPA DATA';
  }

  function render() {
    const demo = $('dataMode').value === 'demo';
    const latest = rows.at(-1);
    $('connectionStatus').textContent = demo ? 'Demo · simulasi' : liveStatus;
    $('connectionStatus').classList.toggle('demo', demo);
    $('modeNotice').textContent = demo
      ? 'DEMO — Semua nilai, kurva, kelas, dan confidence adalah simulasi lokal. Sesi tetap 0–300 detik; bukan stream perangkat.'
      : 'Live membaca riwayat PostgreSQL setiap 5 detik; bukan bukti ESP32 online. Data lama tetap riwayat, nilai kosong berarti tidak tersedia. Kegagalan API tidak memakai simulasi.';
    $('chartDevice').textContent = $('deviceSelect').value;
    $('lastUpdate').textContent = demo ? 'Sesi Demo tetap · interval 5 detik' : latest ? `Data terakhir: ${latest.created_at ?? 'timestamp tidak tersedia'} · ID ${latest.id}` : 'Belum ada data diterima';
    $('tempVal').textContent = latest ? number(latest.temperature, 1) : '—';
    $('humVal').textContent = latest ? number(latest.humidity, 1) : '—';
    sensors.forEach((_, i) => { $(`sensorValue${i}`).textContent = latest ? number(latest.sensors[i], 3) + ' V' : '— V'; });
    $('sampleCount').textContent = `${rows.length} sampel${demo ? ' Demo' : ''}`;
    $('chartCaption').textContent = demo ? `${rows.length} sampel Demo · 0–300 detik · interval 5 s` : `${rows.length} sampel PostgreSQL · waktu relatif dari timestamp asli (detik)`;
    $('exportCsv').disabled = !rows.length;
    $('reportLink').setAttribute('aria-disabled', String(demo || location.protocol === 'file:'));
    $('reportLink').href = `/api/report?${new URLSearchParams({device_id:$('deviceSelect').value,limit:'200'})}`;
    $('exportStatus').textContent = '';
    $('logTableBody').innerHTML = rows.length ? rows.map(row => `<tr><td>${escape(row.id)}</td><td>${escape(row.created_at)}</td><td>${escape(row.device ?? $('deviceSelect').value)}</td><td>${escape(row.coffee_id)}</td><td>${row.t}</td><td>${number(row.temperature,1)}</td><td>${number(row.humidity,1)}</td>${row.sensors.map(v => `<td>${number(v,3)}</td>`).join('')}${row.predictions.map((v, i) => `<td>${escape(v)} / ${percent(row.confidence[i])}</td>`).join('')}</tr>`).join('') : '<tr><td colspan="18" class="empty-row">Belum ada pembacaan. Data Live tidak disimulasikan.</td></tr>';
    renderPredictions(latest);
    renderChart();
  }

  // Deterministic, explicitly labelled demo fixture. Never used in Live mode.
  function loadSource() {
    generation++;
    clearTimeout(pollTimer);
    if (activeRequest) activeRequest.abort();
    liveStatus = 'Menghubungkan API…';
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
    if ($('dataMode').value === 'live') pollLive(generation);
  }
  $('deviceSelect').addEventListener('change', loadSource);
  $('dataMode').addEventListener('change', loadSource);

  const tabs = [$('monitorTab'), $('otaTab'), $('mlTab')];
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
      activateTab(tabs[event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length-1 : (i + (event.key === 'ArrowRight' ? 1 : tabs.length-1)) % tabs.length], true);
    });
  });

  $('exportCsv').addEventListener('click', () => {
    if (!rows.length) return;
    const mode = $('dataMode').value.toUpperCase();
    const header = ['mode', 'device', 'id', 'created_at', 'coffee_id', 'elapsed_s', 'temperature_c', 'humidity_percent', ...sensors.map((s, i) => `S${i + 1}_${s[0]}_V`), 'L1_species', 'L1_confidence_percent', 'L2_roast', 'L2_confidence_percent', 'L3_sample_id', 'L3_confidence_percent'];
    const records = rows.map(row => [mode, row.device ?? $('deviceSelect').value, row.id ?? '', row.created_at ?? '', row.coffee_id ?? '', row.t, number(row.temperature,1), number(row.humidity,1), ...row.sensors.map(v => number(v,3)), ...row.predictions.flatMap((v, i) => [v ?? '—', row.confidence[i] ?? '—'])]);
    const csv = [header, ...records].map(record => record.map(value => `"${String(value).replaceAll('"', '""')}"`).join(',')).join('\r\n');
    const url = URL.createObjectURL(new Blob(['\uFEFF', csv, '\r\n'], { type: 'text/csv;charset=utf-8' }));
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `coffee-lab-${mode}-${$('deviceSelect').value}.csv`;
    document.body.append(anchor);
    anchor.click();
    anchor.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
    $('exportStatus').textContent = `CSV ${mode} disiapkan: ${rows.length} baris, semua 8 sensor; sesuai tabel tampil.`;
  });

  function clearInspection() {
    inspectionToken++;
    $('fileInspection').hidden = true;
    $('fileInspection').replaceChildren();
    $('otaFeedback').textContent = '';
    $('otaFeedback').classList.remove('error');
    $('inspectFirmware').disabled = false;
  }
  ['fwVersion', 'fwFile'].forEach(id => $(id).addEventListener('input', clearInspection));
  $('otaForm').addEventListener('submit', async event => {
    event.preventDefault();
    clearInspection();
    const token = inspectionToken;
    const device = 'ESP32_G09';
    const version = $('fwVersion').value;
    const file = $('fwFile').files[0];
    const errors = [];
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
      $('otaFeedback').textContent = 'Pemeriksaan lokal selesai. Isi firmware belum diverifikasi kompatibel. Tidak ada berkas diunggah. Upload to server adalah langkah terpisah.';
    } catch (error) {
      if (token !== inspectionToken) return;
      $('otaFeedback').classList.add('error');
      $('otaFeedback').textContent = 'Berkas tidak dapat dibaca. Pilih ulang berkas lokal dan coba lagi. Tidak ada unggahan.';
    } finally {
      if (token === inspectionToken) $('inspectFirmware').disabled = false;
    }
  });
  $('reportLink').addEventListener('click', async event => {
    event.preventDefault();
    if ($('dataMode').value !== 'live' || location.protocol === 'file:') {
      $('exportStatus').textContent = 'PDF hanya untuk data Live dari server; bukan laporan Demo.'; return;
    }
    const url = $('reportLink').href;
    $('exportStatus').textContent = 'Menyiapkan PDF dari PostgreSQL…';
    try {
      const response = await fetch(url, {cache:'no-store', signal:AbortSignal.timeout(20000)});
      if (!response.ok || !response.headers.get('content-type')?.includes('application/pdf')) throw new Error(`HTTP ${response.status}`);
      const blob = await response.blob();
      const link = document.createElement('a'); link.href = URL.createObjectURL(blob); link.download = 'coffee-lab-report.pdf';
      link.click(); setTimeout(() => URL.revokeObjectURL(link.href),1000);
      $('exportStatus').textContent = 'PDF Live diunduh; maksimal 200 sampel terbaru sesuai filter chamber saat klik.';
    } catch (error) { $('exportStatus').textContent = `PDF gagal: ${error.message}. Tidak ada laporan simulasi pengganti.`; }
  });
  let uploading = false;
  const uploadEnabled = () => {
    const secure = location.protocol === 'https:' || ['127.0.0.1','localhost','[::1]'].includes(location.hostname);
    $('deployFirmware').disabled = uploading || !secure || !$('otaToken').value || !$('fwFile').files[0] || !$('fwVersion').value;
  };
  ['otaToken','fwVersion','fwFile'].forEach(id => $(id).addEventListener('input', uploadEnabled));
  $('deployFirmware').addEventListener('click', async () => {
    if (uploading || $('deployFirmware').disabled) return;
    const file = $('fwFile').files[0];
    if (file.size > 16*1024*1024) { $('otaFeedback').textContent = 'Berkas melebihi 16 MiB.'; return; }
    uploading = true; uploadEnabled();
    $('otaFeedback').textContent = 'Mengunggah ke server; tidak mem-flash perangkat…';
    const form = new FormData();
    form.set('target','ESP32_G09'); form.set('version',$('fwVersion').value); form.set('firmware',file);
    try {
      const response = await fetch('/api/ota/upload', {method:'POST', headers:{Authorization:`Bearer ${$('otaToken').value}`}, body:form, signal:AbortSignal.timeout(60000)});
      if (!response.ok) throw new Error(`HTTP ${response.status}: ${await response.text()}`);
      const result = await response.json();
      const verify = await fetch(`/api/ota/manifest/${encodeURIComponent(result.id)}`,{headers:{Authorization:`Bearer ${$('otaToken').value}`},cache:'no-store',signal:AbortSignal.timeout(10000)});
      if (!verify.ok || (await verify.json()).sha256 !== result.sha256) throw new Error('Readback manifest gagal');
      $('otaFeedback').textContent = `Upload ke server selesai. ID ${result.id}; firmware ${result.firmware_version}; ${result.size_bytes} byte; SHA-256 ${result.sha256}. Hardware belum dikonfirmasi; tidak ada flashing atau aktivasi. Kompatibilitas belum terbukti.`;
    } catch (error) { $('otaFeedback').textContent = `Upload/readback gagal: ${error.message}. Hardware tidak diubah.`; }
    finally { uploading=false; uploadEnabled(); }
  });
  window.addEventListener('pagehide', () => { $('otaToken').value=''; });
  uploadEnabled();
  loadSource();
  new ResizeObserver(() => {
    if (!$('monitorPanel').hidden) renderChart();
  }).observe($('sensorChart'));
})();
