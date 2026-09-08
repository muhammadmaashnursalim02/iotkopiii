use axum::http::StatusCode;

// Endpoint OTA Firmware (Sementara Dummy)
pub async fn check_ota() -> (StatusCode, &'static str) {
    (StatusCode::OK, "OTA v1.1.0 siap")
}
