# PDF + OTA server artifacts (local stages 2/3)

## Safe local start

From repository root (Bash):

```bash
PORT=3001 cargo run --locked --bin dashboard_readonly
```

Open http://127.0.0.1:3001. Binary name retained for compatibility: **database remains read-only**, but **OTA may write local artifact files**. No migrations, MQTT subscriber, telemetry publication, or hardware flashing. Existing DATABASE_URL loaded from untouched `.env`. Do not run main backend for read-only verification; main still has existing MQTT/schema startup behavior. Stop only this exact owned executable before rebuilding on Windows; never unrelated Node port 3000.

OTA defaults closed (503) without `OTA_ADMIN_TOKEN` of at least 32 bytes. Configure a cryptographically random token through process environment, not committed configuration. Optional independent `OTA_DEVICE_TOKEN` (at least 32 bytes) permits read endpoints only. Never embed admin token in firmware. `OTA_STORAGE_DIR` selects private local storage, outside dashboard; default OS temporary directory `coffee-lab-ota-private`. Production/operator use should select a persistent private directory with OS ACL restricted to service account. Directory and ancestors must not be publicly served. Do not give untrusted local users write access. Root canonicalization rejects dashboard/junction aliases. No retention quota yet: operators must monitor disk and remove obsolete artifact directories while server stopped.

Enter matching admin token in **OTA & Admin → Token admin (memori tab saja)** password field. Browser holds it only in page memory, clears on navigation, never localStorage/sessionStorage. Upload enabled only on HTTPS or loopback and when token/file/version supplied. No token created for ordinary runner; uploads remain disabled until configured. Do not paste secrets in chat/logs. Use HTTPS with verified certificates off-loopback; this local runner does not provide TLS. Same-origin UI; permissive CORS removed.

## Real PDF

`GET /api/report?device_id=DAQ01&limit=200`

- `application/pdf`, attachment filename `coffee-lab-report.pdf`, no-store/nosniff.
- Exact parameterized optional chamber filter, nonblank max 128 bytes; limit 1..1000, default 1000. Dashboard requests latest 200. Snapshot may differ from displayed latest 20. Never silently export Demo as Live.
- Latest rows, chronological range; count, finite per-channel count/min/mean/max for eight sensors and environment; per-level label counts and valid 0..1 confidence means as percent. Missing/invalid confidence excluded explicitly. No labels means **accuracy unavailable**, not confidence renamed accuracy.
- Query bounded to 1000 rows, label text 96 characters, eight-second request query timeout. Offline native PDF generation: no browser, HTML, scripts, external fonts/URLs, shell, or network renderer. Escapes PDF literals and substitutes unsupported/control characters with `?`; ASCII report labels are fully searchable. Non-ASCII original labels are not faithfully reproduced. Template remains unchanged as historical reference; report reuses its slate heading/editorial hierarchy, not its fabricated sample rows.
- Empty selection generates honest zero-sample PDF. DB failure 503, invalid query 400.

## OTA protocol

All endpoints require `Authorization: Bearer TOKEN`; POST requires admin token, GET permits admin or device read token. No cookies/query-string credentials.

1. `POST /api/ota/upload`, multipart fields exactly `target=ESP32_G09`, `version=major.minor.patch` (optional `v` prefix and alphanumeric `-`/`_` suffix), `firmware=application-image.bin`.
2. Returns 201 manifest: `id` (random 32 lowercase hexadecimal), `target`, `firmware_version`, `size_bytes`, `sha256`, `status=stored_only`, `hardware_status=unconfirmed`, relative `download_url`, creation Unix timestamp, compatibility disclaimer.
3. `GET /api/ota/manifest/{id}` reads stored metadata.
4. `GET /api/ota/check?target=ESP32_G09&artifact_id={id}` optionally reads nominated candidate; always `update_available=false`, `active_firmware=null`. No active/latest pointer exists, so test uploads and newer versions never become deployment instructions.
5. `GET /api/ota/download/{id}` returns attachment bytes with Content-Length from Axum; server checks stored size/SHA-256 before serving. No range/resume support. Client verifies exact size and SHA-256 before any separate approval/flash flow.

Kelas A firmware integration still required: verified TLS, dedicated read credential, explicit nominated artifact, ESP32 target and board/partition/chip revision checks, trusted signed-image verification, anti-rollback policy, bounded download, digest check, then separately authorized update and device acknowledgement. Never flash test fixtures. Server does not publish MQTT, choose chambers, command restart, or claim deployed status. SHA-256 is integrity, **not signature authentication**.

Validation: max 16 MiB image (+8 KiB multipart envelope), max 64-byte text fields, safe .bin filename (no paths), fixed target, literal firmware version, ESP32 magic/chip ID/segment count/bounds/alignment/XOR checksum and optional appended SHA-256. Supports unsigned standard ESP32 app images and appended digest; rejects secure-boot signature trailer formats until explicit support added. Structural validity cannot prove bootability, board compatibility, trusted source, or model validity. Raw TinyML model files are not firmware images.

Writes: unique staging directory, create-new file handles, sync files, atomic directory rename publishes firmware and metadata together. Failed writes clean staging when possible; crash may leave hidden `.pending-*`, never addressable by routes. Storage-read failures and integrity mismatch fail closed. Concurrent upload/download buffering limited to two requests; no hardware activation endpoint.

## Verification

```bash
cargo test --locked --all-targets
cargo build --locked --bins
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --test dashboard_api -- --ignored --nocapture
# Stop owned port-3001 runner before dedicated test harness:
python dashboard/tests/run_report_ota_local.py
# Restart normal read-only runner then:
python dashboard/tests/test_live_dashboard.py
python dashboard/tests/test_dashboard.py
node --check dashboard/js/main.js
```

Python tests require requests, pypdf, playwright and installed Chrome. Dedicated harness generates random token and artifacts in `coffee-ota-test-*` OS temp directory, Windows ACL restricted to current user. Secret never printed. Harness starts exact read-only executable and stops its own child in finally. Test payload visibly contains `TEST ARTIFACT ONLY - NEVER FLASH!!`; no active firmware pointer exists. Test directories preserved for inspection; never reuse them as real deployment storage.

Evidence: `dashboard/tests/evidence/azure-DAQ01-report.pdf`, `browser-report.pdf`, extracted `.txt`, `report-ota-results.json`, `ota-server-upload.png`, existing live/offline regression JSON. Real Azure verification found 18 DAQ01 rows, latest ID 18. All ten numeric summaries and three confidence means match real API; independent Rust HTTP-vs-SQL test confirms actual Azure rows. Browser PDF download and admin upload/manifest/download hash roundtrip pass. Unauthorized, wrong-token, garbage, oversize, filename traversal and invalid version rejected; default no-token runner returns 503. Test fixture never active or flashed.
