//! Offline PDF writer: no HTML, JavaScript, subprocess, fonts or network resources.
use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use serde_json::Value;
use sqlx::PgPool;
use std::collections::BTreeMap;

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/api/report", get(report))
        .with_state(pool)
}
async fn report(
    State(pool): State<PgPool>,
    Query(q): Query<crate::dashboard::DashboardQuery>,
) -> Response {
    let limit = q.limit.unwrap_or(1000);
    if !(1..=1000).contains(&limit)
        || q.device_id
            .as_ref()
            .is_some_and(|s| s.trim().is_empty() || s.len() > 128)
    {
        return (
            StatusCode::BAD_REQUEST,
            "limit must be 1..1000; device_id 1..128 nonblank bytes",
        )
            .into_response();
    }
    let rows = tokio::time::timeout(std::time::Duration::from_secs(8), sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(t) FROM (SELECT id::text AS id, created_at, left(device_id,128) AS device_id, s1_mq3,s2_mq6,s3_mq7,s4_mq135,s5_tgs2600,s6_tgs2602,s7_tgs2611,s8_tgs2620,temperature,humidity,left(pred_level_1,96) AS pred_level_1,left(pred_level_2,96) AS pred_level_2,left(pred_level_3,96) AS pred_level_3,conf_level_1,conf_level_2,conf_level_3 FROM telemetry_data WHERE ($1::text IS NULL OR device_id=$1) ORDER BY created_at DESC NULLS LAST,id DESC LIMIT $2) t ORDER BY created_at ASC NULLS FIRST,id::bigint ASC"
    ).bind(&q.device_id).bind(limit).fetch_all(&pool)).await;
    match rows {
        Ok(Ok(rows)) => (
            [
                (header::CONTENT_TYPE, "application/pdf"),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=coffee-lab-report.pdf",
                ),
                (header::CACHE_CONTROL, "no-store"),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            ],
            render(
                &rows,
                q.device_id.as_deref().unwrap_or("All chambers"),
                limit,
            ),
        )
            .into_response(),
        _ => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Telemetry database unavailable",
        )
            .into_response(),
    }
}
fn numeric(rows: &[Value], key: &str) -> Vec<f64> {
    rows.iter()
        .filter_map(|r| r[key].as_f64())
        .filter(|n| n.is_finite())
        .collect()
}
pub fn render(rows: &[Value], device: &str, limit: i64) -> Vec<u8> {
    let mut lines = vec![format!("Chamber filter: {device}"),"Source: Azure PostgreSQL telemetry history (not hardware connectivity)".into(),format!("Sample count: {}",rows.len()),format!("Window: latest {limit} samples maximum; not entire database"),format!("Time start: {}",rows.first().and_then(|r|r["created_at"].as_str()).unwrap_or("Unavailable")),format!("Time end: {}",rows.last().and_then(|r|r["created_at"].as_str()).unwrap_or("Unavailable")),"".into(),"SENSOR / ENVIRONMENT STATISTICS".into(),"Finite readings only; missing values excluded. Sensor units V; temperature C; humidity % RH.".into()];
    for key in [
        "s1_mq3",
        "s2_mq6",
        "s3_mq7",
        "s4_mq135",
        "s5_tgs2600",
        "s6_tgs2602",
        "s7_tgs2611",
        "s8_tgs2620",
        "temperature",
        "humidity",
    ] {
        let v = numeric(rows, key);
        lines.push(if v.is_empty() {
            format!("{key}: n=0 unavailable")
        } else {
            format!(
                "{key}: n={} min={:.4} mean={:.4} max={:.4}",
                v.len(),
                v.iter().copied().fold(f64::INFINITY, f64::min),
                v.iter().sum::<f64>() / v.len() as f64,
                v.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            )
        });
    }
    lines.extend([
        "".into(),
        "PREDICTION DISTRIBUTIONS / MODEL CONFIDENCE".into(),
        "Accuracy unavailable: no ground-truth labels. Confidence is not accuracy.".into(),
        "Labels truncated to 96 characters; counts group by displayed label.".into(),
    ]);
    for level in 1..=3 {
        let v: Vec<_> = numeric(rows, &format!("conf_level_{level}"))
            .into_iter()
            .filter(|v| (0.0..=1.0).contains(v))
            .collect();
        lines.push(if v.is_empty() {
            format!("L{level} mean confidence: unavailable (n=0)")
        } else {
            format!(
                "L{level} mean confidence: {:.4}% (n={}; missing/invalid={})",
                100.0 * v.iter().sum::<f64>() / v.len() as f64,
                v.len(),
                rows.len() - v.len()
            )
        });
        let mut counts = BTreeMap::new();
        for row in rows {
            *counts
                .entry(
                    row[format!("pred_level_{level}")]
                        .as_str()
                        .unwrap_or("[missing]")
                        .to_owned(),
                )
                .or_insert(0usize) += 1;
        }
        for (label, count) in counts {
            lines.push(format!("L{level} {label}: {count}"));
        }
    }
    let wrapped: Vec<String> = lines
        .into_iter()
        .flat_map(|s| {
            let chars: Vec<_> = s.chars().collect();
            if chars.is_empty() {
                vec![String::new()]
            } else {
                chars.chunks(100).map(|c| c.iter().collect()).collect()
            }
        })
        .collect();
    pdf(&wrapped)
}
fn literal(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_graphic() || c == ' ' {
                c
            } else {
                '?'
            }
        })
        .collect::<String>()
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}
pub fn experiment_pdf(lines: &[String]) -> Vec<u8> {
    pdf_titled(lines, "EVALUATED EXPERIMENT REPORT", "Local labeled dataset registry")
}
fn pdf(lines: &[String]) -> Vec<u8> {
    pdf_titled(lines, "TELEMETRY ANALYTICS REPORT", "PostgreSQL history")
}
fn pdf_titled(lines: &[String], title: &str, source: &str) -> Vec<u8> {
    let pages = lines.chunks(43).collect::<Vec<_>>();
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".into(),
        String::new(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>".into(),
    ];
    let mut kids = Vec::new();
    for (i, page) in pages.iter().enumerate() {
        let id = objects.len() + 1;
        kids.push(format!("{id} 0 R"));
        let mut stream="0.17 0.22 0.27 rg 40 739 515 65 re f\nBT /F2 19 Tf 1 1 1 rg 56 774 Td (SMART COFFEE / E-NOSE) Tj ET\nBT /F1 10 Tf 1 1 1 rg 56 752 Td (TELEMETRY ANALYTICS REPORT) Tj ET\n".to_string();
        stream = stream.replace("TELEMETRY ANALYTICS REPORT", &literal(title));
        for (line, s) in page.iter().enumerate() {
            stream.push_str(&format!(
                "BT /F1 9 Tf 0.17 0.22 0.27 rg 48 {} Td ({}) Tj ET\n",
                714 - line * 15,
                literal(s)
            ));
        }
        stream.push_str(&format!("0.7 G 40 48 m 555 48 l S BT /F1 8 Tf 0.4 g 48 33 Td (COFFEE LAB / G09 - {source} - Page {} / {}) Tj ET",i+1,pages.len()));
        objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {} 0 R >>",id+1));
        objects.push(format!(
            "<< /Length {} >>\nstream\n{}\nendstream",
            stream.len(),
            stream
        ));
    }
    objects[1] = format!(
        "<< /Type /Pages /Kids [{}] /Count {} >>",
        kids.join(" "),
        pages.len()
    );
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![0];
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{}\nendobj\n", i + 1, obj).as_bytes());
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).as_bytes());
    for offset in offsets.iter().skip(1) {
        out.extend(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len()
        )
        .as_bytes(),
    );
    out
}
