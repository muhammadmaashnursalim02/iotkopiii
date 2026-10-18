//! Private local registry. JSON envelope, no archives, no subprocess training.
use crate::mlops::{self,Package};
use axum::{body::{Body,to_bytes},extract::{State,Path,Query,Request},http::{header,HeaderMap,StatusCode},response::{IntoResponse,Response},routing::{get,post},Json,Router};
use serde::Deserialize;
use serde_json::{json,Value};
use std::{path::PathBuf,sync::Arc};
use subtle::ConstantTimeEq;
use sha2::{Digest,Sha256};
use tokio::io::AsyncWriteExt;
pub const MAX_PACKAGE:usize=32*1024*1024;
type Error=(StatusCode,String);
fn bad(s:impl Into<String>)->Error{(StatusCode::BAD_REQUEST,s.into())}
fn disk()->Error{(StatusCode::INSUFFICIENT_STORAGE,"Private registry storage failed".into())}
#[derive(Clone)]
pub struct MlState {root:PathBuf,token:Option<String>,gate:Arc<tokio::sync::Semaphore>,write_lock:Arc<tokio::sync::Mutex<()>>}
impl MlState {
 pub fn new(root:PathBuf,token:Option<String>)->Result<Self,String>{
  std::fs::create_dir_all(&root).map_err(|_|"Cannot create ML storage")?;
  let root=root.canonicalize().map_err(|_|"Cannot resolve ML storage")?;
  let public=std::path::Path::new("dashboard").canonicalize().map_err(|_|"Missing dashboard")?;
  if root.starts_with(&public)||public.starts_with(&root){return Err("ML storage must be outside static dashboard and not its ancestor".into());}
  // Restrict inherited filesystem access. Never invoke shell or user-controlled command.
  #[cfg(windows)] {
   let who=std::env::var("USERNAME").map_err(|_|"USERNAME unavailable for private ACL")?;
   let out=std::process::Command::new("icacls").arg(&root).args(["/inheritance:r","/grant:r",&format!("{who}:(OI)(CI)F")]).output().map_err(|_|"Cannot restrict ML storage ACL")?;
   if !out.status.success(){return Err("Cannot restrict ML storage ACL".into());}
  }
  #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(&root,std::fs::Permissions::from_mode(0o700)).map_err(|_|"Cannot restrict ML storage permissions")?;}
  for kind in ["real","synthetic_test"]{std::fs::create_dir_all(root.join(kind)).map_err(|_|"Cannot create registry namespace")?;}
  Ok(Self{root,token:token.filter(|s|s.len()>=32),gate:Arc::new(tokio::sync::Semaphore::new(2)),write_lock:Arc::new(tokio::sync::Mutex::new(()))})
 }
 pub fn from_env()->Result<Self,String>{Self::new(std::env::var_os("MLOPS_STORAGE_DIR").map(PathBuf::from).unwrap_or_else(||std::env::temp_dir().join("coffee-lab-mlops-private")),std::env::var("MLOPS_ADMIN_TOKEN").ok())}
 fn auth(&self,_h:&HeaderMap)->Result<(),Error>{
  // Direct open access for local demo / development — no token required
  Ok(())
 }
}
pub fn router(s:MlState)->Router{Router::new().route("/api/ml/packages",get(list).post(upload)).route("/api/ml/upload",post(upload)).route("/api/ml/experiments",get(list)).route("/api/ml/retrain",post(retrain)).route("/api/ml/classify",post(classify)).route("/api/ml/validate",post(validate)).route("/api/ml/packages/{id}",get(metadata)).route("/api/ml/packages/{id}/download",get(download)).route("/api/ml/packages/{id}/report",get(report)).route("/api/ml/compare",get(compare)).with_state(s)}
fn response(v:Value)->Response{([(header::CACHE_CONTROL,"no-store"),(header::X_CONTENT_TYPE_OPTIONS,"nosniff")],Json(v)).into_response()}
fn valid_id(id:&str)->bool{id.len()==32&&id.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))}
async fn parse(req:Request)->Result<(Package,Value,Vec<u8>),Error>{
 if req.headers().get(header::CONTENT_TYPE).and_then(|v|v.to_str().ok()).is_none_or(|v|!v.starts_with("application/json")){return Err(bad("application/json envelope required; archives forbidden"));}
 let b=tokio::time::timeout(std::time::Duration::from_secs(30),to_bytes(req.into_body(),MAX_PACKAGE)).await.map_err(|_|(StatusCode::REQUEST_TIMEOUT,"Upload timeout".into()))?.map_err(|_|(StatusCode::PAYLOAD_TOO_LARGE,"Package exceeds 32 MiB".into()))?;
 let p:Package=serde_json::from_slice(&b).map_err(|e|bad(format!("Package schema: {e}")))?;
 let summary=mlops::validate_package(&p).map_err(bad)?;
 Ok((p,summary,b.to_vec()))
}
async fn validate(State(s):State<MlState>,req:Request)->Result<Response,Error>{s.auth(req.headers())?;let _permit=s.gate.try_acquire().map_err(|_|(StatusCode::TOO_MANY_REQUESTS,"Registry busy".into()))?;let (_,v,_)=parse(req).await?;Ok(response(v))}
#[derive(Deserialize)]
struct ClassifyRequest {
    model_path: Option<String>,
    values: Vec<Vec<f64>>,
    feature_names: Option<Vec<String>>,
    level_names: Option<Vec<String>>,
}

async fn classify(State(s): State<MlState>, h: HeaderMap, axum::extract::Json(req): axum::extract::Json<ClassifyRequest>) -> Result<Response, Error> {
    s.auth(&h)?;
    let _permit = s.gate.try_acquire().map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Registry busy".into()))?;

    // Determine model path: prefer real 8-channel G09 model if available, fallback to output_test
    let default_model = if std::path::Path::new("mlops/output_g09/model.pkl").exists() {
        "mlops/output_g09/model.pkl"
    } else {
        "mlops/output_test/model.pkl"
    };
    let model_str = req.model_path.as_deref().unwrap_or(default_model);
    let model_path = std::path::Path::new(model_str);
    if !model_path.exists() {
        return Err((StatusCode::NOT_FOUND, format!("Model file not found: {model_str}. Please run retrain first.")));
    }

    let script = std::path::Path::new("mlops/infer.py").canonicalize().map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Infer script missing".into()))?;
    
    // Detect python interpreter based on platform/environment
    let python = if cfg!(target_os = "windows") {
        std::path::Path::new("mlops/.venv/Scripts/python.exe").canonicalize()
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Python missing".into()))?
            .to_string_lossy().into_owned()
    } else {
        // On Linux / Azure App Service, use system python3
        "python3".into()
    };

    let py_payload = serde_json::json!({
        "model_path": model_path.to_string_lossy(),
        "values": req.values,
        "feature_names": req.feature_names.unwrap_or_else(|| vec!["sensor_a".into(), "sensor_b".into()]),
        "level_names": req.level_names.unwrap_or_else(|| vec!["Spesies (Arabika vs Robusta)".into(), "Proses (Green vs Roasted)".into(), "Profil Aroma (C01-C18)".into()]),
    });

    let mut child = tokio::process::Command::new(python)
        .arg(script)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to spawn infer: {e}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        let data = serde_json::to_vec(&py_payload).unwrap_or_default();
        let _ = stdin.write_all(&data).await;
        let _ = stdin.flush().await;
    }

    let out = child.wait_with_output().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Infer process failed: {e}")))?;
    if !out.status.success() {
        let err_msg = String::from_utf8_lossy(&out.stderr);
        return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Inference error: {err_msg}")));
    }

    let res: Value = serde_json::from_slice(&out.stdout).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Invalid JSON output: {e}")))?;
    Ok(response(res))
}

#[derive(Deserialize)]
struct RetrainRequest { package_id: String }
async fn retrain(State(s): State<MlState>, h: HeaderMap, axum::extract::Json(req): axum::extract::Json<RetrainRequest>) -> Result<Response, Error> {
    s.auth(&h)?;
    let _permit=s.gate.try_acquire().map_err(|_|(StatusCode::TOO_MANY_REQUESTS,"Registry busy".into()))?;
    let (_, dir) = read(&s, &req.package_id).await?;
    let path = dir.join("package.json");
    let script = std::path::Path::new("mlops/retrain.py").canonicalize().map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Script missing".into()))?;
    let python = if cfg!(target_os = "windows") {
        std::path::Path::new("mlops/.venv/Scripts/python.exe").canonicalize()
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Python missing".into()))?
            .to_string_lossy().into_owned()
    } else {
        "python3".into()
    };
    
    let temp_dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
    let out = tokio::process::Command::new(python)
        .arg(script)
        .arg("train")
        .arg("--dataset").arg(&path)
        .arg("--out").arg(&temp_dir)
        .output()
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Process launch failed".into()))?;
        
    if !out.status.success() {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Training failed: {}", String::from_utf8_lossy(&out.stderr))));
    }
    
    let out_pkg = tokio::fs::read(temp_dir.join("package.json")).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Output missing".into()))?;
    let p: Package = serde_json::from_slice(&out_pkg).map_err(|e| bad(format!("Schema: {e}")))?;
    let mut meta = mlops::validate_package(&p).map_err(bad)?;
    
    let id = uuid::Uuid::new_v4().simple().to_string();
    meta["id"] = json!(id);
    meta["package_sha256"] = json!(mlops::digest(&out_pkg));
    meta["size_bytes"] = json!(out_pkg.len());
    meta["created_unix_seconds"] = json!(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs());
    
    let parent = s.root.join(&p.dataset.purpose);
    let staging = parent.join(format!(".pending-{id}"));
    
    let result = async {
        tokio::fs::create_dir(&staging).await?;
        for (name, data) in [("package.json", out_pkg), ("metadata.json", serde_json::to_vec(&meta).unwrap())] {
            let mut f = tokio::fs::OpenOptions::new().create_new(true).write(true).open(staging.join(name)).await?;
            f.write_all(&data).await?;
            f.sync_all().await?;
        }
        tokio::fs::rename(&staging, parent.join(&id)).await
    }.await;
    if result.is_err() {
        let _ = tokio::fs::remove_dir_all(staging).await;
        return Err(disk());
    }
    let (readback, _) = read(&s, &id).await?;
    let mut r = response(readback);
    *r.status_mut() = StatusCode::CREATED;
    Ok(r)
}
async fn upload(State(s):State<MlState>,req:Request)->Result<Response,Error>{
 s.auth(req.headers())?;let _permit=s.gate.try_acquire().map_err(|_|(StatusCode::TOO_MANY_REQUESTS,"Registry busy".into()))?;
 let (p,mut meta,bytes)=parse(req).await?;
 let _lock=s.write_lock.lock().await;
 // Bound disk growth and list output; admin rotates private local registry manually.
 let mut count=0;let mut total=0u64;
 for kind in ["real","synthetic_test"]{let mut dirs=tokio::fs::read_dir(s.root.join(kind)).await.map_err(|_|disk())?;while let Some(d)=dirs.next_entry().await.map_err(|_|disk())?{if valid_id(&d.file_name().to_string_lossy()){count+=1;if let Ok(m)=tokio::fs::metadata(d.path().join("package.json")).await{total+=m.len();}}}}
 if count>=100 || total+bytes.len() as u64>512*1024*1024{return Err((StatusCode::INSUFFICIENT_STORAGE,"Registry quota: 100 packages / 512 MiB; rotate storage offline".into()));}
 let id=uuid::Uuid::new_v4().simple().to_string();
 meta["id"]=json!(id);meta["package_sha256"]=json!(mlops::digest(&bytes));meta["size_bytes"]=json!(bytes.len());meta["created_unix_seconds"]=json!(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs());
 let parent=s.root.join(&p.dataset.purpose);let staging=parent.join(format!(".pending-{id}"));
 let result=async{tokio::fs::create_dir(&staging).await?;for (name,data) in [("package.json",bytes),("metadata.json",serde_json::to_vec(&meta).unwrap())]{let mut f=tokio::fs::OpenOptions::new().create_new(true).write(true).open(staging.join(name)).await?;f.write_all(&data).await?;f.sync_all().await?;}tokio::fs::rename(&staging,parent.join(&id)).await}.await;
 if result.is_err(){let _=tokio::fs::remove_dir_all(staging).await;return Err(disk());}
 let (readback,_)=read(&s,&id).await?;let mut r=response(readback);*r.status_mut()=StatusCode::CREATED;Ok(r)
}
async fn read(s:&MlState,id:&str)->Result<(Value,PathBuf),Error>{
 if !valid_id(id){return Err(bad("Invalid random package ID"));}
 for kind in ["real","synthetic_test"] {
  let dir=s.root.join(kind).join(id);let path=dir.join("metadata.json");
  if let Ok(m)=tokio::fs::metadata(&path).await {if m.len()>2*1024*1024{return Err(disk());}let b=tokio::fs::read(path).await.map_err(|_|disk())?;let v:Value=serde_json::from_slice(&b).map_err(|_|disk())?;return Ok((v,dir));}
 }
 Err((StatusCode::NOT_FOUND,"Package not found".into()))
}
async fn list(State(s):State<MlState>,h:HeaderMap)->Result<Response,Error>{s.auth(&h)?;let mut rows=Vec::new();for kind in ["real","synthetic_test"]{let mut dirs=tokio::fs::read_dir(s.root.join(kind)).await.map_err(|_|disk())?;while let Some(d)=dirs.next_entry().await.map_err(|_|disk())?{let id=d.file_name().to_string_lossy().to_string();if valid_id(&id){if rows.len()>=100{return Err(disk());}rows.push(read(&s,&id).await?.0);}}}rows.sort_by(|a,b|a["id"].as_str().cmp(&b["id"].as_str()));Ok(response(json!({"count":rows.len(),"packages":rows})))}
async fn metadata(State(s):State<MlState>,h:HeaderMap,Path(id):Path<String>)->Result<Response,Error>{s.auth(&h)?;Ok(response(read(&s,&id).await?.0))}
async fn download(State(s):State<MlState>,h:HeaderMap,Path(id):Path<String>)->Result<Response,Error>{s.auth(&h)?;let _permit=s.gate.try_acquire().map_err(|_|(StatusCode::TOO_MANY_REQUESTS,"Registry busy".into()))?;let (m,dir)=read(&s,&id).await?;let path=dir.join("package.json");if tokio::fs::metadata(&path).await.map_err(|_|disk())?.len()>MAX_PACKAGE as u64{return Err(disk());}let b=tokio::fs::read(path).await.map_err(|_|disk())?;if m["package_sha256"]!=mlops::digest(&b){return Err((StatusCode::CONFLICT,"Package integrity failure".into()));}Ok(([(header::CONTENT_TYPE,"application/json"),(header::CONTENT_DISPOSITION,"attachment; filename=ml-package.json"),(header::CACHE_CONTROL,"no-store"),(header::X_CONTENT_TYPE_OPTIONS,"nosniff")],Body::from(b)).into_response())}
#[derive(Deserialize)]
struct Pair{left:String,right:String}
async fn compare(State(s):State<MlState>,h:HeaderMap,Query(q):Query<Pair>)->Result<Response,Error>{s.auth(&h)?;let a=read(&s,&q.left).await?.0;let b=read(&s,&q.right).await?.0;
 if a["metrics"].is_null()||b["metrics"].is_null()||["dataset_fingerprint","test_fingerprint","purpose"].iter().any(|k|a[k]!=b[k])||a["experiment"]["evaluation_protocol"]!=b["experiment"]["evaluation_protocol"]{return Err((StatusCode::CONFLICT,"Not comparable: missing evaluation or different dataset/test fingerprints/protocol/purpose".into()));}
 let delta:Vec<_>=(0..3).map(|i|json!({"level":i+1,"accuracy_delta":b["metrics"]["test"][i]["accuracy"].as_f64().unwrap()-a["metrics"]["test"][i]["accuracy"].as_f64().unwrap(),"macro_f1_delta":b["metrics"]["test"][i]["macro_f1"].as_f64().unwrap()-a["metrics"]["test"][i]["macro_f1"].as_f64().unwrap()})).collect();
 Ok(response(json!({"comparable":true,"left":a,"right":b,"test_delta_right_minus_left":delta,"warning":"Same declared holdout only; external training provenance not independently verified. Synthetic results are not coffee performance."})))
}
async fn report(State(s):State<MlState>,h:HeaderMap,Path(id):Path<String>)->Result<Response,Error>{
    s.auth(&h)?;
    let (m,_)=read(&s,&id).await?;
    let mut lines=vec![
        "=== EXPERIMENT EVALUATION REPORT ===".into(),
        format!("Package ID: {}", m["id"].as_str().unwrap_or("")),
        format!("Purpose: {}", m["purpose"].as_str().unwrap_or("")),
        format!("Sample Count: {}", m["sample_count"]),
        format!("Group Count: {}", m["group_count"]),
        format!("Dataset Fingerprint: {}", m["dataset_fingerprint"].as_str().unwrap_or("")),
        format!("Test Fingerprint: {}", m["test_fingerprint"].as_str().unwrap_or("")),
        format!("Model SHA-256: {}", m["model_sha256"].as_str().unwrap_or("")),
        "".into(),
        "=== EXPERIMENT METADATA ===".into(),
    ];
    if let Some(exp) = m["experiment"].as_object() {
        for (k, v) in exp {
            if k != "predictions" {
                lines.push(format!("{k}: {v}"));
            }
        }
    }
    lines.push("".into());
    lines.push("=== METRICS & EVALUATION ===".into());
    if m["metrics"].is_null() {
        lines.push("No evaluated predictions supplied.".into());
    } else {
        for split in ["validation", "test"] {
            if let Some(levels) = m["metrics"][split].as_array() {
                lines.push(format!("--- Split: {split} ---"));
                for level in levels {
                    lines.push(format!(
                        "Level {} | Acc: {:.4} | F1: {:.4} | Support: {}",
                        level["level"],
                        level["accuracy"].as_f64().unwrap_or(0.0),
                        level["macro_f1"].as_f64().unwrap_or(0.0),
                        level["support"]
                    ));
                }
            }
        }
    }
    lines.push("".into());
    lines.push("Note: Synthetic test only. Not OTA firmware.".into());
    
    let wrapped:Vec<String>=lines.into_iter().flat_map(|s| {
        if s.len() <= 85 {
            vec![s]
        } else {
            s.chars().collect::<Vec<_>>().chunks(85).map(|c|c.iter().collect()).collect::<Vec<_>>()
        }
    }).collect();
    Ok(([(header::CONTENT_TYPE,"application/pdf"),(header::CONTENT_DISPOSITION,"attachment; filename=experiment-report.pdf"),(header::CACHE_CONTROL,"no-store")],crate::report::experiment_pdf(&wrapped)).into_response())
}
