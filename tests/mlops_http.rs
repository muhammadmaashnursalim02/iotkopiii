use backend_iot::mlops::{self,MlState};
use serde_json::json;
#[tokio::test]
async fn authenticated_private_registry_roundtrip_and_bounds(){
 let root=std::env::temp_dir().join(format!("mlops-test-{}",uuid::Uuid::new_v4()));
 let token="test-only-012345678901234567890123456789";
 let state=MlState::new(root.clone(),Some(token.into())).unwrap();
 let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let url=format!("http://{}/api/ml",listener.local_addr().unwrap());
 let task=tokio::spawn(async move{axum::serve(listener,mlops::router(state)).await.unwrap()});let c=reqwest::Client::new();
 assert_eq!(c.get(format!("{url}/packages")).send().await.unwrap().status(),401);
 assert_eq!(c.post(format!("{url}/packages")).bearer_auth(token).json(&json!({"pred_level_1":"not truth"})).send().await.unwrap().status(),400);
 let samples:Vec<_>=(0..3).map(|i|json!({"sample_id":format!("s{i}"),"session_id":format!("q{i}"),"group_id":format!("g{i}"),"split":(["train","validation","test"][i]),"labels":["a","b","c"],"values":[[1.0],[2.0]]})).collect();
 let p=json!({"schema_version":1,"dataset":{"schema_version":1,"purpose":"synthetic_test","label_provenance":{"kind":"synthetic_test","verified_by":"test","evidence":"TEST ONLY"},"group_ids":["g0","g1","g2"],"feature_names":["s1"],"feature_units":["V"],"interval_ms":10,"samples":samples},"model":null,"experiment":null});
 let r=c.post(format!("{url}/packages")).bearer_auth(token).json(&p).send().await.unwrap();assert_eq!(r.status(),201);let meta:serde_json::Value=r.json().await.unwrap();let id=meta["id"].as_str().unwrap();assert_eq!(id.len(),32);
 let read:serde_json::Value=c.get(format!("{url}/packages/{id}")).bearer_auth(token).send().await.unwrap().json().await.unwrap();assert_eq!(read,meta);
 let download:serde_json::Value=c.get(format!("{url}/packages/{id}/download")).bearer_auth(token).send().await.unwrap().json().await.unwrap();assert_eq!(download,p);
 assert_eq!(c.get(format!("{url}/packages/{id}/download")).send().await.unwrap().status(),401);
 let report=c.get(format!("{url}/packages/{id}/report")).bearer_auth(token).send().await.unwrap().bytes().await.unwrap();assert!(report.starts_with(b"%PDF"));assert!(String::from_utf8_lossy(&report).contains("unavailable"));
 assert_eq!(c.get(format!("{url}/compare?left={id}&right={id}")).bearer_auth(token).send().await.unwrap().status(),409);
 assert_eq!(c.post(format!("{url}/packages")).bearer_auth(token).header("content-type","application/json").body(" ".repeat(mlops::MAX_PACKAGE+1)).send().await.unwrap().status(),413);
 tokio::fs::write(root.join("synthetic_test").join(id).join("package.json"),b"tamper").await.unwrap();
 assert_eq!(c.get(format!("{url}/packages/{id}/download")).bearer_auth(token).send().await.unwrap().status(),409);
 task.abort();std::fs::remove_dir_all(root).unwrap();
}
