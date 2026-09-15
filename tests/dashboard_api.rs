use backend_iot::dashboard;
use sqlx::postgres::PgPoolOptions;

#[tokio::test]
async fn validation_and_database_failure_are_honest() {
    let pool = PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(100))
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, dashboard::router(pool))
            .await
            .unwrap()
    });
    let client = reqwest::Client::new();
    for query in [
        "limit=0",
        "limit=201",
        "limit=-1",
        "limit=no",
        "device_id=",
        "device_id=%20",
    ] {
        assert_eq!(
            client
                .get(format!("http://{address}/api/dashboard?{query}"))
                .send()
                .await
                .unwrap()
                .status(),
            400,
            "{query}"
        );
    }
    let response = client
        .get(format!("http://{address}/api/dashboard"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 503);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["error"],
        "Telemetry database unavailable"
    );
    server.abort();
}

#[tokio::test]
#[ignore = "real Azure read-only HTTP versus independent SQL readback"]
async fn azure_http_matches_sql() {
    dotenvy::dotenv().ok();
    let pool = PgPoolOptions::new()
        .connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .expect("Azure connection failed; URL suppressed");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = dashboard::router(pool.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = reqwest::Client::new();
    for device in [
        None,
        Some("DAQ01"),
        Some("DAQ02"),
        Some("__absent_dashboard_test__"),
        Some("' OR 1=1 --"),
    ] {
        let mut url = reqwest::Url::parse(&format!("http://{address}/api/dashboard")).unwrap();
        if let Some(value) = device {
            url.query_pairs_mut().append_pair("device_id", value);
        }
        let request = client.get(url);
        let response = request.send().await.unwrap();
        assert_eq!(response.status(), 200);
        let body = response.json::<serde_json::Value>().await.unwrap();
        let expected:Vec<serde_json::Value> = sqlx::query_scalar("SELECT to_jsonb(t) FROM (SELECT * FROM telemetry_data WHERE ($1::text IS NULL OR device_id=$1) ORDER BY created_at DESC NULLS LAST, id DESC LIMIT 20) t ORDER BY created_at ASC NULLS FIRST, id ASC").bind(device).fetch_all(&pool).await.unwrap();
        let rows = body["rows"].as_array().unwrap();
        assert_eq!(rows.len(), expected.len());
        for (actual, expected) in rows.iter().zip(&expected) {
            for (key, value) in actual.as_object().unwrap() {
                if key == "id" {
                    assert_eq!(value.as_str().unwrap(), expected[key].to_string());
                } else {
                    assert_eq!(value, &expected[key], "{key}");
                }
            }
        }
        assert_eq!(
            body["latest_update"],
            rows.last()
                .map(|r| r["created_at"].clone())
                .unwrap_or(serde_json::Value::Null)
        );
        println!(
            "REAL HTTP/SQL device={device:?} count={} latest_id={} timestamp={}",
            rows.len(),
            rows.last().map(|r| r["id"].clone()).unwrap_or_default(),
            body["latest_update"]
        );
    }
    let limited = client
        .get(format!("http://{address}/api/dashboard?limit=1"))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert!(limited["rows"].as_array().unwrap().len() <= 1);
    server.abort();
}
