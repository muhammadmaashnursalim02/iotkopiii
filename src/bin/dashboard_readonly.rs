// Database read-only; local OTA artifact disk writes allowed. No MQTT or migrations.
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tower_http::services::ServeDir;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3001".into())
        .parse()?;
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    let url = std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL missing; restore locally")?;
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .acquire_timeout(Duration::from_secs(8))
        .after_connect(|connection, _| {
            Box::pin(async move {
                sqlx::query("SET default_transaction_read_only = on")
                    .execute(&mut *connection)
                    .await?;
                sqlx::query("SET statement_timeout = '5s'")
                    .execute(&mut *connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .map_err(|_| "PostgreSQL connection failed; credentials suppressed")?;
    let app = backend_iot::dashboard::router(pool.clone())
        .merge(backend_iot::report::router(pool))
        .merge(backend_iot::ota::router(
            backend_iot::ota::OtaState::from_env()?,
        ))
        .merge(backend_iot::mlops::router(backend_iot::mlops::MlState::from_env()?))
        .fallback_service(ServeDir::new("dashboard"));
    println!(
        "Read-only dashboard http://127.0.0.1:{port}; pid={}; MQTT disabled; migrations disabled",
        std::process::id()
    );
    axum::serve(listener, app).await?;
    Ok(())
}
