# Local Live dashboard

PDF dan OTA server kini tersedia: [REPORT_OTA.md](REPORT_OTA.md). Runner tetap read-only untuk DB; OTA boleh menulis disk lokal dengan token admin. Default OTA tertutup.

Run from backend-iot in bash:

```bash
PORT=3001 cargo run --bin dashboard_readonly
```

Open http://127.0.0.1:3001 . `.env` supplies existing DATABASE_URL; never commit secrets.
Read-only runner binds loopback, sets PostgreSQL sessions read-only, uses 5-second SQL timeout, and never runs migrations or MQTT. Port 3000 was already occupied by node.exe; leave it alone. Stop this runner before rebuilding on Windows (running .exe cannot be replaced).
Normal backend also registers same dashboard router; no second MQTT subscriber needed.

`GET /api/dashboard?device_id=DAQ01&limit=20`

- Optional exact parameterized device_id; nonblank, at most 128 bytes.
- limit defaults 20, accepted 1..200; invalid input HTTP 400.
- Latest N ordered by created_at then id, returned oldest-to-newest.
- JSON: source, device_id, limit, count, latest_update, rows. IDs strings preserve bigint precision. Missing legacy fields remain null.
- Database errors HTTP 503 with generic message, no credentials. Success Cache-Control: no-store.
- UI polls five seconds after previous request settles; ten-second request timeout. Mode/filter generation guards reject stale replies. Demo never substitutes for API failures.
- Timestamp indicates stored telemetry history, not ESP32 online or verified hardware provenance. Confidence not accuracy.
- CSV exports currently displayed bounded window, not entire database. Includes real ID, timestamp, chamber, coffee ID, eight sensors, environment and three predictions/confidences. Null readings display em dash; Demo IDs/timestamps blank.

Verification (real Azure SELECT only; no publish):

```bash
cargo test
cargo test --test dashboard_api -- --ignored --nocapture
cargo fmt -- --check
# Start read-only runner, then:
python dashboard/tests/test_live_dashboard.py
python dashboard/tests/test_dashboard.py
```

Live browser suite includes real HTTP/data/CSV comparisons, confidence, repeated polling, real empty DAQ02 filter, test-only 503 and nullable fixtures, delayed requests and mode/filter races, HTML escaping, tab isolation, responsive layout and OTA local SHA-256. Offline suite verifies file:// disconnected mode, Demo, original report hash, sensor toggles, OTA invalid files and no uploads. Fixtures exist only inside tests.
