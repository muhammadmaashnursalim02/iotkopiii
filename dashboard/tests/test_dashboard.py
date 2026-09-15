from pathlib import Path
import csv
import hashlib
import json
from playwright.sync_api import sync_playwright, expect

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'tests' / 'evidence'
OUT.mkdir(exist_ok=True)
checks, errors, network = [], [], []

def passed(name):
    checks.append(name)
    print('PASS:', name)

with sync_playwright() as p:
    browser = p.chromium.launch(executable_path='C:/Program Files/Google/Chrome/Application/chrome.exe', headless=True)
    page = browser.new_page(viewport={'width':1440,'height':1000}, accept_downloads=True)
    page.on('pageerror', lambda e: errors.append(str(e)))
    page.on('console', lambda msg: errors.append(msg.text) if msg.type == 'error' else None)
    page.on('request', lambda request: network.append(request.url) if request.url.startswith(('http:', 'https:', 'ws:', 'wss:')) else None)
    page.goto((ROOT / 'index.html').as_uri())
    expect(page.locator('#connectionStatus')).to_have_text('Belum terhubung')
    expect(page.locator('#tempVal')).to_have_text('—')
    expect(page.locator('#humVal')).to_have_text('—')
    expect(page.locator('#exportCsv')).to_be_disabled()
    expect(page.locator('#sensorGrid input')).to_have_count(8)
    expect(page.locator('.prediction-value')).to_have_text(['—'] * 3)
    expect(page.locator('#sensorChart polyline')).to_have_count(0)
    page.screenshot(path=str(OUT/'desktop-live.png'), full_page=True)
    passed('Live default: disconnected, eight empty sensors, empty environment/predictions, no chart lines, CSV disabled')
    page.select_option('#dataMode', 'demo')
    expect(page.locator('#connectionStatus')).to_have_text('Demo · simulasi')
    expect(page.locator('#logTableBody tr')).to_have_count(61)
    expect(page.locator('#sensorChart polyline')).to_have_count(8)
    expect(page.locator('.confidence strong')).to_have_text(['96%', '91%', '78%'])
    passed('Demo: 61 displayed samples, eight SVG curves, separate three confidence scores')
    first_value = page.locator('#sensorValue0').inner_text()
    page.select_option('#deviceSelect', 'DAQ02')
    assert page.locator('#sensorValue0').inner_text() != first_value
    expect(page.locator('#chartDevice')).to_have_text('DAQ02')
    expect(page.locator('.prediction-value')).to_have_text(['Robusta','Roasted','C02'])
    passed('Device selection updates chart identity, sensor readings and hierarchical predictions')
    for checkbox in page.locator('#sensorGrid input').all():
        checkbox.uncheck()
    expect(page.locator('#sensorChart polyline')).to_have_count(0)
    expect(page.locator('#chartEmpty')).to_contain_text('Tidak ada kanal dipilih')
    page.locator('#sensorGrid input').first.check()
    expect(page.locator('#sensorChart polyline')).to_have_count(1)
    passed('Sensor toggles: all off empty state, one on exactly one curve')
    with page.expect_download() as download_info:
        page.click('#exportCsv')
    download = download_info.value
    csv_path = OUT/download.suggested_filename
    download.save_as(csv_path)
    with csv_path.open(encoding='utf-8-sig', newline='') as f:
        exported = list(csv.DictReader(f))
    assert len(exported) == 61
    assert all(row['mode'] == 'DEMO' and row['device'] == 'DAQ02' for row in exported)
    displayed = page.locator('#logTableBody tr').evaluate_all('(rows) => rows.map(r => [...r.cells].map(c => c.textContent))')
    for row, cells in zip(exported, displayed):
        assert list(row.values())[5:16] == cells[4:15]
        assert cells[:4] == ['—','—',row['device'],'—']
        assert [row['L1_species']+' / '+row['L1_confidence_percent']+'%', row['L2_roast']+' / '+row['L2_confidence_percent']+'%', row['L3_sample_id']+' / '+row['L3_confidence_percent']+'%'] == cells[15:]
    passed('Downloaded CSV: 61 rows exactly match every displayed cell, all eight sensors retained despite toggle filter')
    for checkbox in page.locator('#sensorGrid input').all():
        checkbox.check()
    page.screenshot(path=str(OUT/'desktop-demo.png'), full_page=True)
    expect(page.locator('#reportLink')).to_have_attribute('aria-disabled','true')
    page.click('#reportLink', force=True)
    expect(page.locator('#exportStatus')).to_contain_text('PDF hanya untuk data Live')
    original_hash = json.loads((ROOT/'backups/20260908-205303/manifest.json').read_text())['report_template.html']
    assert hashlib.sha256((ROOT/'report_template.html').read_bytes()).hexdigest() == original_hash
    passed('Offline/Demo PDF blocked honestly; original template SHA-256 unchanged')
    page.click('#otaTab')
    expect(page.locator('#otaPanel')).to_be_visible()
    expect(page.locator('#monitorPanel')).to_be_hidden()
    page.locator('#otaTab').press('ArrowLeft')
    expect(page.locator('#monitorTab')).to_be_focused()
    expect(page.locator('#monitorPanel')).to_be_visible()
    page.locator('#monitorTab').press('End')
    expect(page.locator('#otaTab')).to_be_focused()
    passed('Tabs: mouse navigation, arrow and End keyboard navigation, focus and panel visibility')
    page.click('#inspectFirmware')
    expect(page.locator('#otaFeedback')).to_contain_text('Versi harus')
    expect(page.locator('#otaFeedback')).to_contain_text('Pilih berkas')
    page.fill('#fwVersion',' v2.0.1 ')
    page.set_input_files('#fwFile',{'name':'wrong.txt','mimeType':'text/plain','buffer':b'test'})
    page.click('#inspectFirmware')
    expect(page.locator('#otaFeedback')).to_contain_text('Versi harus')
    expect(page.locator('#otaFeedback')).to_contain_text('Ekstensi')
    page.fill('#fwVersion','v2.0.1_svm')
    page.set_input_files('#fwFile',{'name':'empty.bin','mimeType':'application/octet-stream','buffer':b''})
    page.click('#inspectFirmware')
    expect(page.locator('#otaFeedback')).to_contain_text('Berkas kosong')
    page.set_input_files('#fwFile',{'name':'large.bin','mimeType':'application/octet-stream','buffer':bytes(16*1024*1024+1)})
    page.click('#inspectFirmware')
    expect(page.locator('#otaFeedback')).to_contain_text('16 MiB')
    passed('OTA fixed ESP32_G09 target; rejects missing version/file, malformed literal version, wrong extension, empty and oversized files')
    payload = bytes(range(32))
    page.set_input_files('#fwFile',{'name':'inspection-only.bin','mimeType':'application/octet-stream','buffer':payload})
    page.click('#inspectFirmware')
    expect(page.locator('#fileInspection')).to_be_visible()
    expect(page.locator('#fileInspection')).to_contain_text(hashlib.sha256(payload).hexdigest())
    expect(page.locator('#fileInspection')).to_contain_text('32 byte')
    expect(page.locator('#fileInspection')).to_contain_text('00 01 02 03 04 05 06 07 08 09 0a 0b 0c 0d 0e 0f')
    expect(page.locator('#otaFeedback')).to_contain_text('Tidak ada berkas diunggah')
    expect(page.locator('#deployFirmware')).to_be_disabled()
    page.screenshot(path=str(OUT/'desktop-ota.png'), full_page=True)
    page.fill('#fwVersion','v2.0.2')
    expect(page.locator('#fileInspection')).to_be_hidden()
    passed('OTA local inspection: exact bytes/size/SHA-256; no upload claim; deploy disabled; edits invalidate inspection')
    for width in [1440,1024,768,390,320]:
        page.set_viewport_size({'width':width,'height':900})
        for tab in ['monitorTab','otaTab']:
            page.click('#'+tab)
            assert page.evaluate('document.documentElement.scrollWidth <= innerWidth'), (width,tab,'overflow')
        if width == 390:
            page.screenshot(path=str(OUT/'mobile-ota.png'),full_page=True)
            page.click('#monitorTab')
            page.screenshot(path=str(OUT/'mobile-demo.png'),full_page=True)
    passed('Responsive: Monitor and OTA no page overflow at 1440, 1024, 768, 390, 320 px')
    page.click('#monitorTab')
    page.select_option('#dataMode','live')
    expect(page.locator('#tempVal')).to_have_text('—')
    expect(page.locator('#sensorChart polyline')).to_have_count(0)
    expect(page.locator('#exportCsv')).to_be_disabled()
    expect(page.locator('#sampleCount')).to_have_text('0 sampel')
    page.select_option('#deviceSelect','DAQ03')
    expect(page.locator('#connectionStatus')).to_have_text('Belum terhubung')
    page.reload()
    expect(page.locator('#dataMode')).to_have_value('live')
    passed('Returning Live clears Demo rows/values/export; Live device selection and reload never invent telemetry')
    assert not errors, errors
    assert not network, network
    passed('Zero browser JS/console errors; zero HTTP/HTTPS/WebSocket requests')
    browser.close()

result = {'passed':len(checks),'checks':checks,'browser':'Installed Chrome via Playwright, headless, file://','console_errors':errors,'external_requests':network,'report_sha256':original_hash}
(OUT/'results.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
print(json.dumps(result,indent=2))
