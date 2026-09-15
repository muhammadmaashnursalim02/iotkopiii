pub mod config;
pub mod dashboard;
pub mod models;
pub mod mlops;
mod ml_registry;
pub mod mqtt_subscriber;
pub mod ota;
pub mod report;

/// Deliberately bypass legacy destructive DROP migration. This repair is idempotent.
pub async fn prepare_schema(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(509090001)")
        .execute(&mut *tx)
        .await?;
    sqlx::raw_sql(include_str!(
        "../migrations/20260909000000_add_sensor_columns.sql"
    ))
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}
