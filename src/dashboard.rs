use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::PgPool;

#[derive(Deserialize)]
pub struct DashboardQuery {
    pub device_id: Option<String>,
    pub limit: Option<i64>,
}

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/api/dashboard", get(read_dashboard))
        .with_state(pool)
}

async fn read_dashboard(
    State(pool): State<PgPool>,
    Query(query): Query<DashboardQuery>,
) -> Result<([(header::HeaderName, &'static str); 1], Json<Value>), (StatusCode, Json<Value>)> {
    let limit = query.limit.unwrap_or(20);
    if !(1..=200).contains(&limit)
        || query
            .device_id
            .as_ref()
            .is_some_and(|s| s.trim().is_empty() || s.len() > 128)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                json!({"error":"limit must be 1..200; device_id must be 1..128 bytes and nonblank"}),
            ),
        ));
    }
    // Read only. Explicit public columns, nullable legacy values preserved as JSON null.
    // String IDs prevent loss of PostgreSQL bigint precision in browsers.
    let rows: Vec<Value> = sqlx::query_scalar(
        r#"
        SELECT to_jsonb(t) FROM (
          SELECT id::text AS id, created_at, device_id, coffee_id,
            s1_mq3, s2_mq6, s3_mq7, s4_mq135, s5_tgs2600, s6_tgs2602, s7_tgs2611, s8_tgs2620,
            temperature, humidity, pred_level_1, pred_level_2, pred_level_3,
            conf_level_1, conf_level_2, conf_level_3
          FROM telemetry_data WHERE ($1::text IS NULL OR device_id = $1)
          ORDER BY created_at DESC NULLS LAST, telemetry_data.id DESC LIMIT $2
        ) t ORDER BY created_at ASC NULLS FIRST, id::bigint ASC
    "#,
    )
    .bind(&query.device_id)
    .bind(limit)
    .fetch_all(&pool)
    .await
    .map_err(|_| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":"Telemetry database unavailable"})),
        )
    })?;
    let latest_update = rows
        .last()
        .map(|r| r["created_at"].clone())
        .unwrap_or(Value::Null);
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(json!({
            "source":"postgresql", "device_id":query.device_id, "limit":limit,
            "count":rows.len(), "latest_update":latest_update, "rows":rows
        })),
    ))
}
