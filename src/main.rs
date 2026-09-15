use axum::Router;
use backend_iot::{config, mqtt_subscriber};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Startup/runtime error: {e}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), String> {
    match dotenvy::dotenv() {
        Ok(_) => {}
        Err(dotenvy::Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("Invalid .env syntax; check locally (values suppressed)".into()),
    }
    let options = config::mqtt_options(false)?;
    let url = config::required("DATABASE_URL")?;
    if url.contains("***") || url.contains("%2A%2A%2A") {
        return Err("DATABASE_URL password placeholder; restore real URL locally".into());
    }
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".into())
        .parse()
        .map_err(|_| "PORT must be integer 1..65535")?;
    if port == 0 {
        return Err("PORT must be integer 1..65535".into());
    }
    // Claim HTTP port before starting MQTT: duplicate starts cannot create extra consumers.
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::UNSPECIFIED, port))
        .await
        .map_err(|e| format!("HTTP bind failed: {e}"))?;
    let pool = PgPoolOptions::new().max_connections(5).acquire_timeout(Duration::from_secs(15)).connect(&url).await.map_err(|e| {
        let code = e.as_database_error().and_then(|d| d.code()).map(|c| c.to_string()).unwrap_or_else(|| "network/TLS/config".into());
        format!("PostgreSQL connection failed ({code}); check DATABASE_URL locally, Azure TLS/firewall. Credentials not logged")
    })?;
    backend_iot::prepare_schema(&pool)
        .await
        .map_err(|e| format!("Additive schema upgrade failed: {e}"))?;
    println!("PostgreSQL connected; additive schema ready");

    let app = Router::new()
        .fallback_service(ServeDir::new("dashboard"))
        .merge(backend_iot::ota::router(
            backend_iot::ota::OtaState::from_env()?,
        ))
        .merge(backend_iot::mlops::router(backend_iot::mlops::MlState::from_env()?))
        .merge(backend_iot::report::router(pool.clone()))
        .merge(backend_iot::dashboard::router(pool.clone()));
    println!("HTTP bound port={port}; MQTT connection pending");
    tokio::select! {
        r = mqtt_subscriber::start_mqtt_listener(pool, options) => r,
        r = axum::serve(listener, app) => r.map_err(|e| format!("HTTP failed: {e}")),
        r = tokio::signal::ctrl_c() => r.map_err(|e| e.to_string()),
    }
}
