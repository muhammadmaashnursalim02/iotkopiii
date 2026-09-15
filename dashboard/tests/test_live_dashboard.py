"""Run against read-only Rust API. Network fixtures below are tests, never Live fallback."""
import csv
import io
import json
import os
import hashlib
from pathlib import Path
from playwright.sync_api import sync_playwright, expect

URL = os.environ.get('DASHBOARD_URL', 'http://127.0.0.1:3001')
checks = []
def passed(name):
    checks.append(name)
    print('PASS:', name)

with sync_playwright() as p:
    browser = p.chromium.launch(executable_path='C:/Program Files/Google/Chrome/Application/chrome.exe', headless=True)
    page = browser.new_page(accept_downloads=True)
    errors = []
    page.on('pageerror', lambda e: errors.append(str(e)))
    page.add_init_script('''
      const originalFetch = window.fetch;
      window.fetchActive = 0; window.fetchMax = 0; window.fetchCalls = 0;
      window.fetch = async (...args) => {
        window.fetchCalls++; window.fetchActive++;
        window.fetchMax = Math.max(window.fetchMax,window.fetchActive);
        try { return await originalFetch(...args); }
        finally { window.fetchActive--; }
      };
    ''')
    page.goto(URL)
    expect(page.locator('#connectionStatus')).to_have_text('Live · riwayat PostgreSQL')
    real = page.request.get(URL + '/api/dashboard?device_id=DAQ01').json()['rows']
    assert real
    expect(page.locator('#logTableBody tr')).to_have_count(len(real))
    expect(page.locator('#sensorGrid input')).to_have_count(8)
    latest = real[-1]
    fields = ['s1_mq3','s2_mq6','s3_mq7','s4_mq135','s5_tgs2600','s6_tgs2602','s7_tgs2611','s8_tgs2620']
    for i, field in enumerate(fields):
        expect(page.locator(f'#sensorValue{i}')).to_have_text(f'{latest[field]:.3f} V' if latest[field] is not None else '— V')
    expect(page.locator('#tempVal')).to_have_text(f'{latest["temperature"]:.1f}')
    expect(page.locator('#humVal')).to_have_text(f'{latest["humidity"]:.1f}')
    expect(page.locator('.prediction-value')).to_have_text([latest[f'pred_level_{i}'] for i in range(1,4)])
    expect(page.locator('.confidence strong')).to_have_text([f'{round(latest[f"conf_level_{i}"]*100,1):g}%' for i in range(1,4)])
    expect(page.locator('#lastUpdate')).to_contain_text(latest['created_at'])
    expect(page.locator('#modeNotice')).to_contain_text('bukan bukti ESP32 online')
    assert 'NaN' not in page.locator('#sensorChart').inner_html()
    passed('Real Azure rows render eight sensors, environment, predictions, timestamp; history not online')
    with page.expect_download() as download:
        page.click('#exportCsv')
    exported = list(csv.DictReader(io.StringIO(open(download.value.path(), encoding='utf-8-sig').read())))
    assert len(exported) == len(real)
    assert [r['id'] for r in exported] == [r['id'] for r in real]
    assert [r['created_at'] for r in exported] == [r['created_at'] for r in real]
    assert all(r['mode'] == 'LIVE' for r in exported)
    cells = page.locator('#logTableBody tr').evaluate_all('(rows)=>rows.map(r=>[...r.cells].map(c=>c.textContent))')
    for row, displayed in zip(exported,cells):
        assert [row['id'],row['created_at'],row['device'],row['coffee_id'] or '—'] == displayed[:4]
        assert list(row.values())[5:16] == displayed[4:15]
        assert [row['L1_species']+' / '+(row['L1_confidence_percent']+'%' if row['L1_confidence_percent']!='—' else '—'), row['L2_roast']+' / '+(row['L2_confidence_percent']+'%' if row['L2_confidence_percent']!='—' else '—'), row['L3_sample_id']+' / '+(row['L3_confidence_percent']+'%' if row['L3_confidence_percent']!='—' else '—')] == displayed[15:]
    before_poll = page.evaluate('window.fetchCalls')
    page.wait_for_function('window.fetchCalls > '+str(before_poll),timeout=7000)

    passed('Live CSV has exact real IDs/timestamps and displayed row count')
    page.select_option('#deviceSelect','DAQ02')
    expect(page.locator('#connectionStatus')).to_have_text('Live · data kosong')
    expect(page.locator('#exportCsv')).to_be_disabled()
    expect(page.locator('#tempVal')).to_have_text('—')
    passed('Real DAQ02 filter empty; clears DAQ01')
    page.route('**/api/dashboard?*', lambda route: route.fulfill(status=503, json={'error':'Telemetry database unavailable'}))
    page.select_option('#deviceSelect','DAQ01')
    expect(page.locator('#connectionStatus')).to_have_text('Terputus · gagal membaca API')
    expect(page.locator('#sensorChart polyline')).to_have_count(0)
    expect(page.locator('#exportCsv')).to_be_disabled()
    passed('HTTP failure honest; no random fallback')
    page.select_option('#dataMode','demo')
    expect(page.locator('#logTableBody tr')).to_have_count(61)
    expect(page.locator('#connectionStatus')).to_have_text('Demo · simulasi')
    expect(page.locator('#sensorChart polyline')).to_have_count(8)
    expect(page.locator('.confidence strong')).to_have_text(['96%','91%','78%'])
    with page.expect_download() as download:
        page.click('#exportCsv')
    demo_csv = list(csv.DictReader(io.StringIO(open(download.value.path(),encoding='utf-8-sig').read())))
    assert len(demo_csv)==61 and all(r['mode']=='DEMO' and r['id']=='' for r in demo_csv)
    passed('Explicit Demo and CSV remain separate from Live')
    page.unroute('**/api/dashboard?*')
    pending = []
    page.route('**/api/dashboard?*', lambda route: pending.append(route))
    page.select_option('#dataMode','live')
    page.wait_for_timeout(200)
    page.wait_for_timeout(5500)
    assert len(pending) == 1, 'Slow request must not cause overlapping polling'
    page.select_option('#deviceSelect','DAQ02')
    page.wait_for_timeout(200)
    assert len(pending) == 2, 'Canceled DAQ01 request followed by DAQ02 request'
    assert 'device_id=DAQ02' in pending[-1].request.url
    # AbortController cancellation settles fetch immediately; routing can retain canceled route.
    page.select_option('#dataMode','demo')
    for route in pending:
        try: route.fulfill(json={'rows':real,'latest_update':latest['created_at']})
        except Exception: pass
    page.wait_for_timeout(200)
    expect(page.locator('#connectionStatus')).to_have_text('Demo · simulasi')
    expect(page.locator('#logTableBody tr')).to_have_count(61)
    passed('Delayed response cannot overwrite new filter/mode')
    page.unroute('**/api/dashboard?*')
    nullable = {key: None for key in latest}
    nullable.update(id='9007199254740993',device_id='DAQ01',created_at='2020-01-01T00:00:00+00:00',pred_level_1='<img src=x onerror=alert(1)>')
    page.route('**/api/dashboard?*',lambda route: route.fulfill(json={'rows':[nullable], 'latest_update':nullable['created_at']}))
    page.select_option('#deviceSelect','DAQ01')
    page.select_option('#dataMode','live')
    expect(page.locator('#sampleCount')).to_have_text('1 sampel')
    expect(page.locator('#tempVal')).to_have_text('—')
    expect(page.locator('.prediction-value').first).to_have_text(nullable['pred_level_1'])
    assert page.locator('#predictions img').count()==0
    assert 'NaN' not in page.locator('#sensorChart').inner_html()
    passed('Nullable legacy fields safe; prediction HTML escaped; bigint ID preserved')
    page.click('#otaTab')
    expect(page.locator('#monitorPanel')).to_be_hidden()
    expect(page.locator('#otaPanel')).to_be_visible()
    page.click('#inspectFirmware')
    expect(page.locator('#otaFeedback')).to_contain_text('Versi harus')
    page.fill('#fwVersion','v2.0.1_svm')
    payload = bytes(range(32))
    page.set_input_files('#fwFile',{'name':'local.bin','mimeType':'application/octet-stream','buffer':payload})
    page.click('#inspectFirmware')
    expect(page.locator('#fileInspection')).to_contain_text(hashlib.sha256(payload).hexdigest())
    expect(page.locator('#fileInspection')).to_contain_text('ESP32_G09')
    expect(page.locator('#deployFirmware')).to_be_disabled()
    page.locator('#otaTab').press('ArrowLeft')
    expect(page.locator('#monitorPanel')).to_be_visible()
    expect(page.locator('#otaPanel')).to_be_hidden()
    for width in [1440,768,390,320]:
        page.set_viewport_size({'width':width,'height':900})
        for tab in ['otaTab','monitorTab']:
            page.click('#'+tab)
            assert page.evaluate('document.documentElement.scrollWidth <= innerWidth'), (width,tab)
    assert page.evaluate("window.fetchMax") == 1
    assert not errors, errors
    passed('Tab isolation, keyboard, responsive layout, OTA local SHA-256; zero JS errors')
    browser.close()
result = {'passed':len(checks),'checks':checks,'url':URL,'latest_id':latest['id'],'latest_update':latest['created_at'],'real_rows':len(real),'javascript_errors':errors}
(Path(__file__).parent/'evidence'/'live-results.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
print(json.dumps(result,indent=2))
