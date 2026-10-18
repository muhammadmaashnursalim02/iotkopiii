# Panduan MLOps Lokal (Class C)

Dashboard MLOps lokal ini mendukung manajemen model Machine Learning untuk proyek E-Nose. Fokusnya pada validasi, import, dan retrain model secara lokal menggunakan Python SVM/LDA, serta perbandingan eksperimen.

## 1. Koneksi ke MLOps Registry (Backend Rust)

Untuk mengakses fungsionalitas MLOps, Anda perlu menyediakan token administrator. Token ini melindungi endpoint API `/api/ml/*` dari akses tidak sah.

1.  **Mendapatkan Token:** Token (`MLOPS_ADMIN_TOKEN`) disimpan di file `.env` di root direktori backend Rust (`C:/Users/User/Documents/IoT_Project/backend-iot/.env`). Salin nilai token ini.
    *   Contoh `.env`:
        ```
        MLOPS_ADMIN_TOKEN=local-dev-token-change-in-production-32chars
        ```
    *   **Penting:** Token ini hanya untuk pengembangan lokal. Di produksi, gunakan token yang lebih kuat dan jaga kerahasiaannya.

2.  **Masukkan Token di Dashboard:** Pada tab **Dataset / Experiments**, masukkan token yang sudah disalin ke kolom input `Token MLOPS admin`.

3.  **Klik 'Connect / Baca registry':** Setelah token dimasukkan, klik tombol `Connect / Baca registry`. Dashboard akan melakukan panggilan ke API backend (`/api/ml/packages`) untuk mengambil daftar paket dataset dan eksperimen yang sudah terdaftar.

    *   **Status:** Pesan status di bawah tombol akan menunjukkan apakah koneksi berhasil dan berapa banyak paket yang ditemukan. Token tidak disimpan di browser (memori tab saja).

## 2. Struktur Paket Dataset & Eksperimen (JSON)

Setiap paket MLOps adalah file JSON tunggal yang berisi metadata dataset, model (opsional), dan detail eksperimen (opsional).

*   **Skema:** Gunakan `C:/Users/User/Documents/IoT_Project/backend-iot/src/mlops.rs` untuk detail skema JSON lengkap (`struct Package`).
*   **Ukuran Maksimal:** File paket JSON tidak boleh melebihi 32 MiB.

## 3. Validasi dan Import Paket

### Validasi Tanpa Simpan

1.  **Pilih Paket JSON:** Klik `Pilih file` di samping `Package JSON` dan pilih file `.json` yang berisi paket Anda.
2.  **Klik 'Validasi tanpa simpan':** Ini akan mengirimkan file JSON ke backend (`/api/ml/validate`) untuk memvalidasi skemanya dan memeriksa konsistensi data, tetapi tidak akan menyimpannya ke registry.
3.  **Hasil Validasi:** Hasil validasi akan ditampilkan di area `Registry dan evaluasi`, termasuk sidik jari dataset, jumlah sampel, dan metrik (jika ada prediksi dalam paket).

### Import Paket ke Registry

1.  **Pilih Paket JSON:** Sama seperti validasi, pilih file `.json` paket Anda.
2.  **Klik 'Import package':** Ini akan mengirim file JSON ke backend (`/api/ml/packages` via POST) untuk disimpan secara permanen di registry lokal.
3.  **Verifikasi Integritas:** Setelah import, backend akan membaca ulang paket yang baru disimpan dan membandingkan hash SHA-256 untuk memastikan integritas data. Jika berhasil, paket akan muncul di dropdown `Paket terpilih`.

## 4. Retraining Model (Python SVM/LDA)

Dashboard memungkinkan Anda untuk melakukan retraining model yang sudah ada dengan protokol yang ditentukan.

1.  **Pilih Paket untuk Retraining:** Dari dropdown `Pilih paket untuk retraining`, pilih ID paket yang ingin Anda latih ulang.
2.  **Klik 'Retrain (Python SVM)':** Ini akan memicu backend untuk menjalankan skrip Python retraining lokal (`mlops/retrain.py`) menggunakan dataset dari paket yang dipilih. Hasil retraining (model dan metrik baru) akan disimpan sebagai paket baru di registry.
    *   **Alur:** Backend tidak menjalankan web shell. Ini memanggil skrip Python yang sudah didefinisikan secara statis di lingkungan `mlops/.venv`.
    *   **Output:** Paket baru dengan ID yang berbeda akan dibuat, dan dropdown akan diperbarui dengan paket yang baru saja dilatih.

## 5. Eksplorasi Registry dan Evaluasi

Bagian ini memungkinkan Anda melihat detail paket yang telah diimpor atau divalidasi.

1.  **Pilih Paket:** Dari dropdown `Paket terpilih`, pilih paket yang ingin Anda inspeksi.
2.  **Detail Paket:** Area di bawah dropdown akan menampilkan metadata paket, ringkasan dataset, detail eksperimen, dan metrik evaluasi (akurasi, F1-score, confusion matrix) untuk split `validation` dan `test`.

### Aksi Tambahan:

*   **Unduh package:** Mengunduh file JSON paket yang dipilih.
*   **PDF eksperimen:** Mengunduh laporan PDF singkat dari hasil eksperimen.

## 6. Perbandingan Eksperimen

Anda dapat membandingkan metrik dari dua eksperimen yang berbeda, misalnya antara model V1 (baseline) dan V2 (kandidat).

1.  **Pilih Baseline dan Kandidat:** Gunakan dropdown `Baseline V1` dan `Kandidat V2` untuk memilih dua paket eksperimen yang ingin dibandingkan.
2.  **Klik 'Bandingkan fixed test':** Dashboard akan menampilkan delta (perbedaan) metrik akurasi dan macro-F1 antara kandidat dan baseline.
    *   **Catatan:** Perbandingan hanya valid jika kedua paket memiliki sidik jari dataset/test yang sama dan protokol evaluasi yang kompatibel.

## Batasan Penting

*   **Keamanan:** Backend tidak menjalankan model yang diupload sebagai kode atau firmware. Model dianggap `opaque` (data biner saja).
*   **Attestation vs. Verifikasi Independen:** Metrik evaluasi dihitung ulang dari prediksi yang Anda berikan. Ini adalah *attestation* dari Anda, bukan *verifikasi independen* bahwa model remote benar-benar dijalankan dengan integritas.
*   **`synthetic_test`:** Dataset dengan `purpose: "synthetic_test"` hanya untuk pengujian dan tidak boleh diinterpretasikan sebagai performa model pada kopi sungguhan.
*   **Tidak ada Flashing OTA:** Dashboard ini **tidak** melakukan flashing firmware OTA ke perangkat ESP32. Manajemen firmware OTA berada di tab terpisah. Model machine learning dan firmware perangkat adalah dua artefak yang berbeda.
