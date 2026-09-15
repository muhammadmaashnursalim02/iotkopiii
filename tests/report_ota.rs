use backend_iot::{
    ota::{self, OtaState},
    report,
};
use serde_json::json;
use sha2::{Digest, Sha256};
fn fixture() -> Vec<u8> {
    let mut b = vec![0; 48];
    b[0] = 0xe9;
    b[1] = 1;
    b[28] = 4;
    b[32..36].copy_from_slice(b"TEST");
    b[47] = b"TEST".iter().fold(0xef, |a, b| a ^ b);
    b
}
#[test]
fn image_rejects_garbage_truncation_checksum_chip_and_digest() {
    let b = fixture();
    assert!(ota::validate_image(&b).is_ok());
    for n in 0..b.len() {
        assert!(ota::validate_image(&b[..n]).is_err());
    }
    for offset in [0, 1, 12, 28, 32, 47] {
        let mut bad = b.clone();
        bad[offset] ^= 0xff;
        assert!(ota::validate_image(&bad).is_err(), "{offset}");
    }
    let mut signed = b.clone();
    signed[23] = 1;
    let digest = Sha256::digest(&signed);
    signed.extend_from_slice(&digest);
    assert!(ota::validate_image(&signed).is_ok());
    signed[50] ^= 1;
    assert!(ota::validate_image(&signed).is_err());
}
#[test]
fn pdf_escapes_strings_paginates_and_preserves_null_semantics() {
    let rows = vec![
        json!({"created_at":"2026-01-01","s1_mq3":2.0,"conf_level_1":0.8,"pred_level_1":"evil) (\\ /OpenAction"}),
        json!({"s1_mq3":4.0,"conf_level_1":2.0}),
    ];
    let bytes = report::render(&rows, "test) /JavaScript", 2);
    let pdf = String::from_utf8(bytes).unwrap();
    assert!(pdf.starts_with("%PDF-1.4"));
    assert!(pdf.contains("test\\) /JavaScript"));
    assert!(pdf.contains("s1_mq3: n=2 min=2.0000 mean=3.0000 max=4.0000"));
    assert!(pdf.contains("L1 mean confidence: 80.0000%"));
    assert!(pdf.contains("s2_mq6: n=0 unavailable"));
    assert!(pdf.contains("Accuracy unavailable"));
    let many: Vec<_> = (0..1000)
        .map(|i| json!({"pred_level_1":format!("class{i}")}))
        .collect();
    let bytes = report::render(&many, "many", 1000);
    assert!(bytes.len() < 1_000_000);
    assert!(String::from_utf8(bytes).unwrap().contains("Page 2 /"));
}
#[tokio::test]
async fn disabled_closed_and_report_db_failure() {
    let root = std::env::temp_dir().join(format!("ota-rust-test-{}", uuid::Uuid::new_v4()));
    let s = OtaState::new(root.clone(), None, None).unwrap();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(50))
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let app = ota::router(s).merge(report::router(pool));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let c = reqwest::Client::new();
    assert_eq!(
        c.get(format!("http://{addr}/api/ota/check"))
            .send()
            .await
            .unwrap()
            .status(),
        503
    );
    assert_eq!(
        c.get(format!("http://{addr}/api/report"))
            .send()
            .await
            .unwrap()
            .status(),
        503
    );
    assert_eq!(
        c.get(format!("http://{addr}/api/report?limit=1001"))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    server.abort();
    std::fs::remove_dir_all(root).unwrap();
}
