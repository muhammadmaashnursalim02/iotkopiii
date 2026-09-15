# Backend IoT G09

Alur aktif: EMQX MQTTS port **8883**, topic **`iot5a/g09/telemetry`**, subscriber Rust, Azure PostgreSQL. Dashboard Live, PDF asli, dan penyimpanan OTA lokal tersedia. Lihat [kontrak PDF/OTA](dashboard/REPORT_OTA.md). Upload bukan flashing perangkat.

## Konfigurasi lokal

Simpan rahasia hanya dalam `.env` yang diabaikan Git. Jangan tempel URL DB atau password dalam log/chat.

```dotenv
PORT=3001
DATABASE_URL=postgresql://USER:URL_ENCODED_PASSWORD@HOST:5432/DATABASE?sslmode=require
EMQX_HOST=YOUR_EMQX_HOST
EMQX_USERNAME=YOUR_SUBSCRIBER_USERNAME
EMQX_PASSWORD=YOUR_SUBSCRIBER_PASSWORD
```

Gunakan akun subscriber yang memang diberi ACL topic tersebut. Jangan menggantinya dengan akun publisher. URL PostgreSQL harus menggunakan TLS; percent-encode password bila perlu. Program tidak mencetak URL DB. SQLSTATE `28P01` berarti autentikasi DB ditolak: pulihkan `DATABASE_URL` asli lokal, bukan mengganti password dengan `***`.

Port 3000 saat verifikasi dipakai proses Node lain. Jangan hentikan proses itu. Jalankan port bebas, misalnya Bash:

```bash
PORT=3001 cargo run --locked --bin backend-iot
```

Program mengikat port lebih dulu untuk mencegah subscriber ganda bila port bentrok. TLS MQTT memakai public CA roots dan verifikasi hostname. ConnAck memicu subscribe QoS 1 setiap reconnect. Log SubAck menandakan broker menerima langganan, **bukan** bukti data masuk DB.

## Skema aman

`migrations/20260909000000_add_sensor_columns.sql` menambah 8 sensor, dan melengkapi prediksi untuk skema lama tanpa menghapus baris. Startup menjalankan SQL idempotent ini dalam transaksi dengan advisory lock.

**Jangan jalankan `sqlx migrate run` terhadap seluruh folder ini.** Migrasi historis `20260908100000_update_telemetry_table_g09.sql` berisi `DROP TABLE`. File historis sengaja tidak ditulis ulang agar checksum instalasi lama tidak berubah. Startup tidak menjalankan migrasi destruktif tersebut. Kolom baru nullable untuk baris historis: nilai yang tidak pernah diukur tidak direkayasa. Payload baru wajib memuat semua 18 field.

Field: `device_id`, `coffee_id`, `s1_mq3`, `s2_mq6`, `s3_mq7`, `s4_mq135`, `s5_tgs2600`, `s6_tgs2602`, `s7_tgs2611`, `s8_tgs2620`, `temperature`, `humidity`, `pred_level_1`, `conf_level_1`, `pred_level_2`, `conf_level_2`, `pred_level_3`, `conf_level_3`.

## Delivery dan batasan

- Polling MQTT terpisah dari worker DB melalui antrean bounded 128 pesan.
- Error koneksi terlihat, reconnect dengan backoff 1–30 detik.
- Insert gagal dicoba ulang 1–30 detik; ACK MQTT diberikan setelah commit. JSON tidak valid dicatat dan ditolak tanpa mencetak payload mentah.
- Antrean penuh mematikan listener dengan error jelas, bukan membuang pesan diam-diam.
- QoS 1 membutuhkan publisher QoS 1 juga; publisher QoS 0 tetap tidak memiliki jaminan redelivery.
- Belum ada measurement ID dalam kontrak payload. Duplicate MQTT/retry ambigu dapat menghasilkan baris ganda. Tidak mengklaim exactly-once atau durability ketika proses mati; sesi saat ini clean dan antrean dalam memori.

## Verifikasi

```bash
cargo build --locked --bins
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked --bin mqtt_test
cargo test --locked --test backend_regression azure_schema_and_latest_row_readback -- --ignored --nocapture
```

`mqtt_test` default read-only: menunggu ConnAck dan SubAck nyata dengan timeout 25 detik. Tidak mempublikasikan data dan tidak memiliki credential hardcoded.

Publikasi simulasi opt-in: set `EMQX_PUBLISHER_USERNAME`, `EMQX_PUBLISHER_PASSWORD`, dan `MQTT_TEST_PAYLOAD_FILE` lokal, lalu `cargo run --locked --bin mqtt_test -- --publish`. Gunakan device_id unik berawalan `SIMTEST-`, payload lengkap, dan akun publisher asli milik pengguna. Program menunggu PubAck, lalu **masih perlu** SELECT baris sesuai ID unik untuk membuktikan end-to-end. Jangan menyebut data simulasi sebagai hardware.

Verifikasi lokal berhasil: ConnAck Success, SubAck QoS 1, commit DAQ01, dan SELECT Azure baris id 12 (`2026-09-09T08:15:36.403458+00:00`) berisi semua sensor dan prediksi. Tidak ada telemetry sintetis dikirim saat verifikasi. Asal fisik perangkat DAQ01 belum diverifikasi; hanya penerimaan broker dan penyimpanan DB yang terbukti.

Docker memakai Rust 1.98 dan `cargo build --locked`; Cargo.lock tidak diabaikan. Build container/deployment belum diuji. Tidak ada cloud deployment atau git push.
