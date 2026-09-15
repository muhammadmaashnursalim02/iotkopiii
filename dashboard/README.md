# Coffee Lab dashboard

**Update:** Live PostgreSQL, PDF nyata, dan upload/download OTA server sudah terhubung. Ikuti [LIVE_API.md](LIVE_API.md) dan [REPORT_OTA.md](REPORT_OTA.md). Catatan di bawah adalah baseline offline awal; klaim template-only/OTA belum terhubung bukan status saat ini.

Buka `index.html` langsung di browser modern. HTML, CSS, JS terpisah; tanpa CDN, API, atau cloud.

- Live default kosong: sumber telemetri belum terhubung.
- Demo harus dipilih eksplisit. Sesi deterministik 61 sampel, 0–300 detik, interval 5 detik. Perangkat memilih fixture berbeda; bukan streaming.
- Checkbox sensor memfilter kurva SVG, bukan log atau CSV. CSV berisi semua baris dan delapan sensor, dengan penanda DEMO, perangkat, waktu relatif, lingkungan, prediksi dan confidence tiap level.
- Confidence bukan akurasi model.
- OTA hanya inspeksi lokal: validasi perangkat, versi literal, ekstensi, ukuran nonkosong sampai 16 MiB; pembacaan ukuran, byte awal, SHA-256 jika Web Crypto tersedia. Tidak memverifikasi kompatibilitas atau tanda tangan. Tidak mengunggah atau deploy.
- Link laporan membuka template statis asli. Template berisi placeholder/contoh lama, bukan laporan sesi ini.

## Verifikasi

Chrome terpasang, Playwright Python. Jalankan dari direktori dashboard:

```bash
python tests/test_dashboard.py
node --check js/main.js
```

Test browser nyata lewat `file://`: 12 kelompok pemeriksaan lulus, nol error JavaScript/console, nol request HTTP/HTTPS/WebSocket. Viewport 1440, 1024, 768, 390, 320 px tanpa overflow halaman. CSV unduhan dibandingkan seluruh sel log. OTA diuji invalid/valid lokal. Screenshot desktop/mobile dan hasil JSON tersimpan di `tests/evidence/`.

Backup empat berkas asli: `backups/20260908-205303/`, beserta manifest SHA-256. `report_template.html` tidak berubah: `cc3d40357e3a542e3255dbec0aee601584aaeb983e5f898ced29db8902eab493`.

Audit desain: Monitor utama, Configure sekunder. Tidak ada hero marketing, gradien, emoji, blur, atau kartu fitur dekoratif. Tipografi Segoe UI dipertahankan dari sumber untuk keterbacaan angka; serif dipakai hanya judul. Audit screenshot menemukan sumbu grafik mobile terlalu kecil; diperbaiki dengan viewBox mengikuti lebar aktual dan grid sensor dua kolom. Skor anti-slop 0/10 pada daftar skill; penilaian desain subjektif.

Backend, environment, database, otorisasi admin, telemetri, dan OTA end-to-end tidak diubah maupun diverifikasi.
