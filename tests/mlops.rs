use serde_json::{json, Value};
fn dataset() -> Value {
    json!({"schema_version":1,"purpose":"synthetic_test","label_provenance":{"kind":"synthetic_test","verified_by":"fixture generator","evidence":"TEST ONLY; no coffee performance"},"group_ids":["g1","g2","g3"],"feature_names":["s1"],"feature_units":["V"],"interval_ms":10,"samples":[
        {"sample_id":"s1","session_id":"q1","group_id":"g1","split":"train","labels":["a","a.b","a.b.c"],"values":[[1.0],[2.0]]},
        {"sample_id":"s2","session_id":"q2","group_id":"g2","split":"validation","labels":["a","a.b","a.b.c"],"values":[[1.0],[2.0]]},
        {"sample_id":"s3","session_id":"q3","group_id":"g3","split":"test","labels":["a","a.b","a.b.c"],"values":[[1.0],[2.0]]}
    ]})
}
#[test]
fn evaluation_recomputes_metrics_binds_model_and_test_set() {
    let d=dataset(); let summary=backend_iot::mlops::validate_dataset_value(d.clone()).unwrap();
    let p=json!({"schema_version":1,"dataset":d,"model":{"format":"edge_impulse_opaque","bytes_hex":"54455354"},"experiment":{"name":"V1","engine":"edge_impulse","model_sha256":backend_iot::mlops::digest(b"TEST"),"dataset_fingerprint":summary["dataset_fingerprint"],"test_fingerprint":summary["test_fingerprint"],"evaluation_protocol":"heldout_group_v1","preprocessing":"External V1 details unavailable","provenance":"Test-only predictions; remote training unverified","predictions":[{"sample_id":"s2","labels":["a","a.b","a.b.c"]},{"sample_id":"s3","labels":["wrong","a.b","a.b.c"]}]}});
    let result=backend_iot::mlops::validate_package_value(p.clone()).unwrap();
    assert_eq!(result["metrics"]["test"][0]["accuracy"],0.0);
    assert_eq!(result["metrics"]["test"][1]["macro_f1"],1.0);
    for field in ["model_sha256","dataset_fingerprint","test_fingerprint"] {let mut bad=p.clone();bad["experiment"][field]=json!("wrong");assert!(backend_iot::mlops::validate_package_value(bad).is_err());}
    let mut bad=p.clone();bad["experiment"]["predictions"].as_array_mut().unwrap().pop();assert!(backend_iot::mlops::validate_package_value(bad).is_err());
    let mut absent=p;absent["experiment"]=Value::Null;absent["model"]=Value::Null;
    assert!(backend_iot::mlops::validate_package_value(absent).unwrap()["metrics"].is_null());
}
#[test]
fn dataset_contract_rejects_prediction_labels_and_group_leakage() {
    let d=dataset();
    let good=backend_iot::mlops::validate_dataset_value(d.clone()).unwrap();
    assert_eq!(good["sample_count"],3);
    for mutation in ["prediction","leakage","session","duplicate","hierarchy","width","real","unknown"] {
        let mut bad=d.clone();
        match mutation {
            "prediction"=>bad["label_provenance"]["kind"]=json!("pred_level_1"),
            "leakage"=>bad["samples"][1]["group_id"]=json!("g1"),
            "session"=>bad["samples"][1]["session_id"]=json!("q1"),
            "duplicate"=>bad["samples"][1]["sample_id"]=json!("s1"),
            "hierarchy"=>bad["samples"][1]["labels"][0]=json!("other"),
            "width"=>bad["samples"][1]["values"][0]=json!([1.0,2.0]),
            "real"=>bad["purpose"]=json!("real"),
            _=>bad["samples"][0]["pred_level_1"]=json!("a"),
        }
        assert!(backend_iot::mlops::validate_dataset_value(bad).is_err(),"{mutation}");
    }
    let mut reordered=d.clone(); reordered["samples"].as_array_mut().unwrap().reverse();
    assert_eq!(good,backend_iot::mlops::validate_dataset_value(reordered).unwrap());
}
