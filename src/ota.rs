//! Staged server artifacts only. No activation, MQTT publish, or device flashing.
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::Arc};
use subtle::ConstantTimeEq;
use tokio::io::AsyncWriteExt;

pub const MAX_SIZE: usize = 16 * 1024 * 1024;
type Error = (StatusCode, &'static str);
#[derive(Clone)]
pub struct OtaState {
    root: PathBuf,
    admin: Option<String>,
    reader: Option<String>,
    gate: Arc<tokio::sync::Semaphore>,
}
impl OtaState {
    pub fn new(
        root: PathBuf,
        admin: Option<String>,
        reader: Option<String>,
    ) -> Result<Self, String> {
        let root = std::path::absolute(root).map_err(|_| "Invalid OTA_STORAGE_DIR")?;
        // Canonicalize existing ancestors too, preventing symlink/junction paths into public files.
        std::fs::create_dir_all(&root).map_err(|_| "Cannot create OTA storage")?;
        let root = root
            .canonicalize()
            .map_err(|_| "Cannot resolve OTA storage")?;
        let public = std::path::Path::new("dashboard")
            .canonicalize()
            .map_err(|_| "dashboard directory missing")?;
        if root.starts_with(&public) || public.starts_with(&root) {
            return Err("OTA storage must be outside public dashboard and not its ancestor".into());
        }
        Ok(Self {
            root,
            admin: admin.filter(|v| v.len() >= 32),
            reader: reader.filter(|v| v.len() >= 32),
            gate: Arc::new(tokio::sync::Semaphore::new(2)),
        })
    }
    pub fn from_env() -> Result<Self, String> {
        let default = std::env::temp_dir().join("coffee-lab-ota-private");
        Self::new(
            std::env::var_os("OTA_STORAGE_DIR")
                .map(PathBuf::from)
                .unwrap_or(default),
            std::env::var("OTA_ADMIN_TOKEN").ok(),
            std::env::var("OTA_DEVICE_TOKEN").ok(),
        )
    }
    fn auth(&self, _h: &HeaderMap, _write: bool) -> Result<(), Error> {
        // Direct open access for local demo / development — no token required
        Ok(())
    }
}
pub fn router(state: OtaState) -> Router {
    Router::new()
        .route("/api/ota/upload", post(upload))
        .route("/api/ota/check", get(check))
        .route("/api/ota/manifest/{id}", get(manifest))
        .route("/api/ota/download/{id}", get(download))
        .layer(DefaultBodyLimit::max(MAX_SIZE + 8192))
        .with_state(state)
}
fn valid_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn version_ok(v: &str) -> bool {
    if v.is_empty()
        || v.len() > 64
        || !v
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return false;
    }
    let v = v.strip_prefix('v').unwrap_or(v);
    let core = v.split(['-', '_']).next().unwrap_or("");
    let parts: Vec<_> = core.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.bytes().all(|b| b.is_ascii_digit())
                && (p.len() == 1 || !p.starts_with('0'))
        })
        && (v.len() == core.len()
            || v.as_bytes()
                .get(core.len() + 1)
                .is_some_and(u8::is_ascii_alphanumeric))
}
/// Structural ESP32 image validation, not signature trust or board compatibility.
pub fn validate_image(b: &[u8]) -> Result<(), Error> {
    let bad = (
        StatusCode::BAD_REQUEST,
        "Invalid ESP32 image: header, segments, checksum or appended digest",
    );
    if b.len() < 48
        || b[0] != 0xe9
        || !(1..=16).contains(&b[1])
        || b[2] > 3
        || b[23] > 1
    {
        return Err(bad);
    }
    let mut offset = 24usize;
    let mut checksum = 0xef;
    for _ in 0..b[1] {
        let h = b.get(offset..offset + 8).ok_or(bad)?;
        let length = u32::from_le_bytes(h[4..8].try_into().unwrap()) as usize;
        if length == 0 || length > MAX_SIZE || !length.is_multiple_of(4) {
            return Err(bad);
        }
        offset += 8;
        let end = offset.checked_add(length).ok_or(bad)?;
        for value in b.get(offset..end).ok_or(bad)? {
            checksum ^= value;
        }
        offset = end;
    }
    let checksum_at = offset + (15 - offset % 16) % 16;
    if b.get(checksum_at) != Some(&checksum)
        || b.get(offset..checksum_at)
            .ok_or(bad)?
            .iter()
            .any(|v| *v != 0)
    {
        return Err(bad);
    }
    let end = checksum_at + 1;
    if b[23] == 1 {
        if b.len() != end + 32 || Sha256::digest(&b[..end])[..] != b[end..] {
            return Err(bad);
        }
    } else if b.len() != end {
        return Err(bad);
    }
    Ok(())
}
async fn upload(
    State(s): State<OtaState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Response, Error> {
    s.auth(&headers, true)?;
    let _permit = s
        .gate
        .try_acquire()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "OTA busy; retry later"))?;
    let mut version = None;
    let mut target = None;
    let mut firmware = None;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| (e.status(), "Invalid or oversized multipart"))?
    {
        let name = field.name().unwrap_or("").to_owned();
        if name == "firmware" && firmware.is_none() {
            let filename = field.file_name().unwrap_or("");
            if filename.len() > 128
                || !filename.to_ascii_lowercase().ends_with(".bin")
                || !filename
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                || filename.contains("..")
            {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Safe .bin filename required; paths forbidden",
                ));
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = field
                .chunk()
                .await
                .map_err(|e| (e.status(), "Invalid or oversized multipart"))?
            {
                if bytes.len() + chunk.len() > MAX_SIZE {
                    return Err((StatusCode::PAYLOAD_TOO_LARGE, "Firmware exceeds 16 MiB"));
                }
                bytes.extend_from_slice(&chunk);
            }
            firmware = Some(bytes);
        } else if (name == "version" && version.is_none()) || (name == "target" && target.is_none())
        {
            let mut bytes = Vec::new();
            while let Some(chunk) = field
                .chunk()
                .await
                .map_err(|e| (e.status(), "Invalid multipart"))?
            {
                if bytes.len() + chunk.len() > 64 {
                    return Err((StatusCode::BAD_REQUEST, "Metadata too long"));
                }
                bytes.extend_from_slice(&chunk);
            }
            let value = String::from_utf8(bytes)
                .map_err(|_| (StatusCode::BAD_REQUEST, "Metadata must be UTF-8"))?;
            if name == "version" {
                version = Some(value);
            } else {
                target = Some(value);
            }
        } else {
            return Err((
                StatusCode::BAD_REQUEST,
                "Unknown or duplicate multipart field",
            ));
        }
    }
    let version = version.filter(|v| version_ok(v)).ok_or((
        StatusCode::BAD_REQUEST,
        "Firmware version must be major.minor.patch with optional v prefix and suffix",
    ))?;
    if target.as_deref() != Some("ESP32_G09") {
        return Err((StatusCode::BAD_REQUEST, "Only ESP32_G09 target permitted"));
    }
    let bytes = firmware.ok_or((StatusCode::BAD_REQUEST, "firmware field required"))?;
    validate_image(&bytes)?;
    let id = uuid::Uuid::new_v4().simple().to_string();
    let record = json!({"id":id,"target":"ESP32_G09","firmware_version":version,"size_bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(&bytes)),"status":"stored_only","hardware_status":"unconfirmed","compatibility":"ESP32 image structure only; board compatibility and signature trust unverified","download_url":format!("/api/ota/download/{id}"),"created_unix_seconds":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()});
    let staging = s.root.join(format!(".pending-{id}"));
    let final_dir = s.root.join(&id);
    let result = async {
        tokio::fs::create_dir(&staging).await?;
        for (name, data) in [
            ("firmware.bin", bytes),
            ("manifest.json", serde_json::to_vec(&record).unwrap()),
        ] {
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(staging.join(name))
                .await?;
            file.write_all(&data).await?;
            file.sync_all().await?;
        }
        tokio::fs::rename(&staging, &final_dir).await
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err((StatusCode::INSUFFICIENT_STORAGE, "Artifact storage failed"));
    }
    // Read exact published target before acknowledging successful storage.
    let readback = read_record(&s, &id).await?;
    Ok((
        StatusCode::CREATED,
        [(header::CACHE_CONTROL, "no-store")],
        Json(readback),
    )
        .into_response())
}
async fn read_record(s: &OtaState, id: &str) -> Result<Value, Error> {
    if !valid_id(id) {
        return Err((StatusCode::BAD_REQUEST, "Invalid artifact ID"));
    }
    let path = s.root.join(id).join("manifest.json");
    let meta = tokio::fs::metadata(&path)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "Artifact not found"))?;
    if meta.len() > 8192 {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Artifact metadata invalid",
        ));
    }
    let data = tokio::fs::read(path)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "Artifact not found"))?;
    serde_json::from_slice(&data).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Artifact metadata invalid",
        )
    })
}
async fn manifest(
    State(s): State<OtaState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, Error> {
    s.auth(&headers, false)?;
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(read_record(&s, &id).await?),
    )
        .into_response())
}
async fn download(
    State(s): State<OtaState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, Error> {
    s.auth(&headers, false)?;
    let _permit = s
        .gate
        .try_acquire()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "OTA busy; retry later"))?;
    let record = read_record(&s, &id).await?;
    let path = s.root.join(id).join("firmware.bin");
    let meta = tokio::fs::metadata(&path)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "Artifact missing"))?;
    if meta.len() > MAX_SIZE as u64 {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "Artifact size invalid"));
    }
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "Artifact missing"))?;
    if record["size_bytes"].as_u64() != Some(bytes.len() as u64)
        || record["sha256"].as_str() != Some(&format!("{:x}", Sha256::digest(&bytes)))
    {
        return Err((StatusCode::CONFLICT, "Artifact integrity failure"));
    }
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=firmware.bin",
            ),
            (header::CACHE_CONTROL, "no-store"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        bytes,
    )
        .into_response())
}
#[derive(Deserialize)]
struct CheckQuery {
    target: Option<String>,
    artifact_id: Option<String>,
}
async fn check(
    State(s): State<OtaState>,
    headers: HeaderMap,
    Query(q): Query<CheckQuery>,
) -> Result<Response, Error> {
    s.auth(&headers, false)?;
    if q.target.as_ref().is_some_and(|t| t != "ESP32_G09") {
        return Err((StatusCode::BAD_REQUEST, "Only ESP32_G09 target permitted"));
    }
    let candidate = if let Some(id) = q.artifact_id {
        Some(read_record(&s, &id).await?)
    } else {
        let mut latest = None;
        let mut max_time = 0;
        if let Ok(mut entries) = tokio::fs::read_dir(&s.root).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Ok(ft) = entry.file_type().await {
                    if ft.is_dir() && !entry.file_name().to_string_lossy().starts_with('.') {
                        let id = entry.file_name().to_string_lossy().to_string();
                        if let Ok(record) = read_record(&s, &id).await {
                            if let Some(time) = record["created_unix_seconds"].as_u64() {
                                if time >= max_time {
                                    max_time = time;
                                    latest = Some(record);
                                }
                            }
                        }
                    }
                }
            }
        }
        latest
    };
    
    let is_update = candidate.is_some();
    Ok(([(header::CACHE_CONTROL,"no-store")],Json(json!({"target":"ESP32_G09","update_available":is_update,"active_firmware":null,"hardware_status":"unconfirmed","candidate":candidate,"policy":"Auto-latest enabled for single device."}))).into_response())
}
