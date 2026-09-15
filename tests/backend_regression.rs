use backend_iot::models::TelemetryPayload;
use serde_json::json;

fn sample() -> serde_json::Value {
    json!({"device_id":"DAQ01","coffee_id":"C09","s1_mq3":1.65,"s2_mq6":1.27,"s3_mq7":1.23,"s4_mq135":1.87,"s5_tgs2600":1.01,"s6_tgs2602":0.95,"s7_tgs2611":1.12,"s8_tgs2620":1.30,"temperature":32.5,"humidity":65.2,"pred_level_1":"Arabica","conf_level_1":0.98,"pred_level_2":"Roasted","conf_level_2":0.95,"pred_level_3":"C09","conf_level_3":0.90})
}
#[test]
fn complete_payload_roundtrip() {
    let payload: TelemetryPayload = serde_json::from_value(sample()).unwrap();
    assert_eq!(payload.pred_level_3, "C09");
    assert_eq!(payload.s8_tgs2620, 1.30);
    assert_eq!(
        serde_json::to_value(payload)
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        18
    );
}
#[test]
fn every_measurement_required() {
    for field in sample().as_object().unwrap().keys() {
        let mut value = sample();
        value.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<TelemetryPayload>(value).is_err(),
            "{field}"
        );
    }
}
#[test]
fn config_missing_has_no_secret() {
    assert_eq!(
        backend_iot::config::required("IOT_TEST_ABSENT_734527").unwrap_err(),
        "IOT_TEST_ABSENT_734527 missing/placeholder; restore locally in .env"
    );
}
#[tokio::test]
async fn resubscribes_after_each_connack() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    async fn packet(stream: &mut tokio::net::TcpStream) -> (u8, Vec<u8>) {
        let header = stream.read_u8().await.unwrap();
        let mut len = 0usize;
        let mut shift = 0;
        loop {
            let b = stream.read_u8().await.unwrap();
            len |= ((b & 127) as usize) << shift;
            if b & 128 == 0 {
                break;
            }
            shift += 7;
        }
        let mut body = vec![0; len];
        stream.read_exact(&mut body).await.unwrap();
        (header, body)
    }
    let server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let options = rumqttc::MqttOptions::new(
        "offline-regression",
        "127.0.0.1",
        server.local_addr().unwrap().port(),
    );
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@localhost/unused")
        .unwrap();
    let task = tokio::spawn(backend_iot::mqtt_subscriber::start_mqtt_listener(
        pool, options,
    ));
    let check = tokio::time::timeout(std::time::Duration::from_secs(8), async {
        for _ in 0..2 {
            let (mut stream, _) = server.accept().await.unwrap();
            assert_eq!(packet(&mut stream).await.0 >> 4, 1);
            stream.write_all(&[0x20, 2, 0, 0]).await.unwrap();
            let (header, body) = packet(&mut stream).await;
            assert_eq!(header, 0x82);
            assert_eq!(&body[4..body.len() - 1], b"iot5a/g09/telemetry");
            assert_eq!(*body.last().unwrap(), 1);
            stream
                .write_all(&[0x90, 3, body[0], body[1], 1])
                .await
                .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await;
    task.abort();
    assert!(check.is_ok(), "reconnect must subscribe again");
}

#[tokio::test]
#[ignore = "requires real Azure DATABASE_URL; read-only readback"]
async fn azure_schema_and_latest_row_readback() {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL").unwrap();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_secs(15))
        .connect(&url)
        .await
        .expect("Azure connection failed (URL suppressed)");
    let cols:Vec<String>=sqlx::query_scalar("SELECT column_name::text FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='telemetry_data'").fetch_all(&pool).await.unwrap();
    for field in sample().as_object().unwrap().keys() {
        assert!(cols.contains(field), "missing {field}");
    }
    let value:serde_json::Value=sqlx::query_scalar("SELECT to_jsonb(t) FROM telemetry_data t WHERE device_id='DAQ01' AND s1_mq3 IS NOT NULL ORDER BY id DESC LIMIT 1").fetch_one(&pool).await.unwrap();
    let data: TelemetryPayload = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(data.device_id, "DAQ01");
    println!("REAL Azure readback (broker-origin, hardware provenance unverified): {value}");
}
