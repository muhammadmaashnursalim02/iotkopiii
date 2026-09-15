"""Local-only acceptance. OTA fixture stored in dedicated temporary runner storage, never active."""
import os, io, json, hashlib, struct
from pathlib import Path
import requests
from pypdf import PdfReader
from playwright.sync_api import sync_playwright, expect
URL = os.environ.get('DASHBOARD_URL', 'http://127.0.0.1:3001')
OUT = Path(__file__).parent / 'evidence'

def fixture():
    data = b'TEST ARTIFACT ONLY - NEVER FLASH!!'
    data += bytes((-len(data)) % 4)
    header = bytearray(24); header[0] = 0xe9; header[1] = 1
    image = header + struct.pack('<II', 0x3f400020, len(data)) + data
    checksum = 0xef
    for byte in data: checksum ^= byte
    image += bytes((15 - len(image) % 16) % 16) + bytes([checksum])
    return bytes(image)

def run():
    response = requests.get(URL + '/api/report?device_id=DAQ01&limit=200', timeout=20)
    assert response.status_code == 200, (response.status_code, response.text[:100])
    assert response.headers['Content-Type'] == 'application/pdf'
    text = '\n'.join(p.extract_text() for p in PdfReader(io.BytesIO(response.content)).pages)
    real = requests.get(URL + '/api/dashboard?device_id=DAQ01&limit=200', timeout=20).json()['rows']
    assert f'Sample count: {len(real)}' in text
    assert 'Accuracy unavailable' in text
    for field in ['s1_mq3','s2_mq6','s3_mq7','s4_mq135','s5_tgs2600','s6_tgs2602','s7_tgs2611','s8_tgs2620','temperature','humidity']:
        values = [r[field] for r in real if isinstance(r[field], (float,int))]
        if values:
            expected = f'{field}: n={len(values)} min={min(values):.4f} mean={sum(values)/len(values):.4f} max={max(values):.4f}'
            assert expected in text, expected
    if real:
        assert f'Time start: {real[0]["created_at"]}' in text
        assert f'Time end: {real[-1]["created_at"]}' in text
    from collections import Counter
    for level in range(1,4):
        for label, count in Counter(r[f'pred_level_{level}'] for r in real).items():
            assert f'L{level} {label}: {count}' in text
        values = [r[f'conf_level_{level}'] for r in real if isinstance(r[f'conf_level_{level}'], (float,int)) and 0 <= r[f'conf_level_{level}'] <= 1]
        if values: assert f'L{level} mean confidence: {sum(values)/len(values)*100:.4f}%' in text
    OUT.mkdir(exist_ok=True)
    (OUT/'azure-DAQ01-report.pdf').write_bytes(response.content)
    (OUT/'azure-DAQ01-report.txt').write_text(text, encoding='utf-8')
    for query in ['limit=0','limit=1001','device_id=','device_id=%20']:
        assert requests.get(URL+'/api/report?'+query).status_code == 400
    empty = requests.get(URL+'/api/report?device_id=__absent_report_test__')
    assert 'Sample count: 0' in '\n'.join(p.extract_text() for p in PdfReader(io.BytesIO(empty.content)).pages)
    payload = fixture()
    def upload(auth=None, data=payload, name='TEST-ONLY.bin', version='0.0.0-test-only'):
        return requests.post(URL+'/api/ota/upload',headers=auth or {},data={'version':version,'target':'ESP32_G09'},files={'firmware':(name,data,'application/octet-stream')},timeout=30)
    assert upload().status_code == 401
    tokenfile = os.environ.get('OTA_TEST_TOKEN_FILE')
    assert tokenfile, 'Dedicated local test runner/token required for upload tests'
    token = Path(tokenfile).read_text().strip()
    auth = {'Authorization':'Bearer '+token}
    assert upload({'Authorization':'Bearer wrong'}).status_code == 401
    for data,name,version in [(b'garbage','bad.bin','1.0.0'),(payload,'../escape.bin','1.0.0'),(payload,'test.bin','../../bad')]:
        assert upload(auth,data,name,version).status_code == 400
    assert upload(auth, bytes(16*1024*1024+1)).status_code == 413
    stored = upload(auth)
    assert stored.status_code == 201, stored.text
    manifest = stored.json()
    assert manifest['target'] == 'ESP32_G09' and manifest['hardware_status'] == 'unconfirmed'
    assert manifest['sha256'] == hashlib.sha256(payload).hexdigest()
    assert requests.get(URL+manifest['download_url']).status_code == 401
    assert requests.get(URL+manifest['download_url'],headers=auth).content == payload
    assert requests.get(URL+'/api/ota/manifest/'+manifest['id'],headers=auth).json() == manifest
    check = requests.get(URL+'/api/ota/check',headers=auth).json()
    assert check['update_available'] is False and check['active_firmware'] is None
    with sync_playwright() as p:
        browser=p.chromium.launch(executable_path='C:/Program Files/Google/Chrome/Application/chrome.exe',headless=True)
        page=browser.new_page(accept_downloads=True)
        errors=[]; page.on('pageerror',lambda e: errors.append(str(e)))
        page.goto(URL)
        expect(page.locator('#connectionStatus')).to_have_text('Live · riwayat PostgreSQL')
        with page.expect_download() as download: page.click('#reportLink')
        download.value.save_as(OUT/'browser-report.pdf')
        assert PdfReader(str(OUT/'browser-report.pdf')).pages
        page.click('#otaTab')
        expect(page.locator('#deployFirmware')).to_be_disabled()
        page.fill('#otaToken',token)
        page.fill('#fwVersion','0.0.0-test-only')
        page.set_input_files('#fwFile',{'name':'TEST-ONLY.bin','mimeType':'application/octet-stream','buffer':payload})
        page.click('#deployFirmware')
        expect(page.locator('#otaFeedback')).to_contain_text('Upload ke server selesai',timeout=30000)
        expect(page.locator('#otaFeedback')).to_contain_text(hashlib.sha256(payload).hexdigest())
        assert not page.evaluate('Object.values(localStorage).some(x=>x.includes(document.querySelector("#otaToken").value))')
        page.fill('#otaToken','')
        page.screenshot(path=str(OUT/'ota-server-upload.png'),full_page=True)
        page.reload(); expect(page.locator('#otaToken')).to_have_value('')
        assert not errors, errors
        browser.close()
    result={'pdf_rows':len(real),'pdf_metrics':'all 10 sensor/environment summaries and 3 confidence means matched real API; API separately checked against SQL','ota_fixture_sha256':manifest['sha256'],'artifact_id':manifest['id'],'hardware_flashed':False,'active_firmware':None,'javascript_errors':errors}
    (OUT/'report-ota-results.json').write_text(json.dumps(result,indent=2))
    print(json.dumps(result,indent=2))
if __name__ == '__main__': run()
