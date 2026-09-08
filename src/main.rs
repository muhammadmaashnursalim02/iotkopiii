use axum::{routing::get, Router};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use dotenvy::dotenv;
use sqlx::postgres::{PgPoolOptions, PgPool};
use std::sync::Arc;
use tokio::task;

mod handlers;
mod models;
mod mqtt_subscriber;

#[derive(Clone)]  
pub struct AppState {
    pub db: Arc<PgPool>,
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL harus ada di .env");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .expect("Gagal konek ke Azure DB");
        
    println!("✅ BERHASIL konek ke Azure Database!");
    
    let app_state = AppState {
        db: Arc::new(pool.clone()),
    };

    let mqtt_pool = pool.clone();
    task::spawn(async move {
        mqtt_subscriber::start_mqtt_listener(mqtt_pool).await;
    });

    let app = Router::new()
        .nest_service("/", ServeDir::new("dashboard"))
        .route("/api/ota/check", get(handlers::check_ota))
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);
    
    println!("🚀 Server Rust nyala di http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}