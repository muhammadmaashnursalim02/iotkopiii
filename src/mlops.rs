//! Local-only labeled dataset registry. No telemetry labels inferred; no model execution.
pub use crate::ml_registry::{router, MlState, MAX_PACKAGE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance { pub kind:String, pub verified_by:String, pub evidence:String }
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Sample { pub sample_id:String, pub session_id:String, pub group_id:String, pub split:String, pub labels:[String;3], pub values:Vec<Vec<f64>> }
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Dataset { pub schema_version:u32, pub purpose:String, pub label_provenance:Provenance, pub group_ids:Vec<String>, pub feature_names:Vec<String>, pub feature_units:Vec<String>, pub interval_ms:u32, pub samples:Vec<Sample> }
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Model { pub format:String, pub bytes_hex:String }
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Prediction {pub sample_id:String, pub labels:[String;3]}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Experiment {pub name:String, pub engine:String, pub model_sha256:String, pub dataset_fingerprint:String, pub test_fingerprint:String, pub evaluation_protocol:String, pub preprocessing:String, pub provenance:String, pub predictions:Option<Vec<Prediction>>}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Package {pub schema_version:u32, pub dataset:Dataset, pub model:Option<Model>, pub experiment:Option<Experiment>}
fn model_bytes(m:&Model)->Result<Vec<u8>,String>{
 need(matches!(m.format.as_str(),"edge_impulse_opaque"|"sklearn_pickle_opaque"|"onnx_opaque"),"Unsupported model format; models are opaque, never executed")?;
 need(!m.bytes_hex.is_empty() && m.bytes_hex.len()<=16*1024*1024 && m.bytes_hex.len()%2==0 && m.bytes_hex.bytes().all(|b|b.is_ascii_hexdigit()),"Model must be nonempty hex, maximum 8 MiB decoded")?;
 m.bytes_hex.as_bytes().chunks_exact(2).map(|c|u8::from_str_radix(std::str::from_utf8(c).unwrap(),16).map_err(|_|"Invalid hex".into())).collect()
}
pub fn validate_package_value(v:Value)->Result<Value,String>{
 let p:Package=serde_json::from_value(v).map_err(|e|format!("Package schema: {e}"))?;validate_package(&p)
}
pub fn validate_package(p:&Package)->Result<Value,String>{
 need(p.schema_version==1,"Package schema_version must be 1")?;
 let mut summary=validate_dataset(&p.dataset)?;
 let model_hash=p.model.as_ref().map(model_bytes).transpose()?.map(|b|digest(&b));
 summary["model_sha256"]=json!(model_hash); summary["metrics"]=Value::Null;
 summary["experiment"]=Value::Null;
 if let Some(e)=&p.experiment {
  need(text(&e.name)&&text(&e.preprocessing)&&text(&e.provenance)&&matches!(e.engine.as_str(),"edge_impulse"|"python_svm"|"python_lda") && e.evaluation_protocol=="heldout_group_v1","Experiment name, engine, provenance, preprocessing and heldout_group_v1 protocol required")?;
  need(model_hash.as_deref()==Some(&e.model_sha256),"Experiment model SHA-256 mismatch or missing model")?;
  need(summary["dataset_fingerprint"]==e.dataset_fingerprint && summary["test_fingerprint"]==e.test_fingerprint,"Dataset/test fingerprint mismatch")?;
  let mut meta=serde_json::to_value(e).unwrap(); meta.as_object_mut().unwrap().remove("predictions"); summary["experiment"]=meta;
  if let Some(preds)=&e.predictions {
   let expected:BTreeMap<_,_>=p.dataset.samples.iter().filter(|s|s.split!="train").map(|s|(&s.sample_id,s)).collect();
   let mut observed=BTreeMap::new();
   for pred in preds {need(expected.contains_key(&pred.sample_id)&&pred.labels.iter().all(|l|text(l))&&observed.insert(&pred.sample_id,pred).is_none(),"Predictions require unique validation/test sample IDs, three labels; no training rows")?;}
   need(observed.len()==expected.len(),"Predictions must cover exact full validation and fixed test sets")?;
   let mut metrics=json!({});
   for split in ["validation","test"] {
    let rows:Vec<_>=expected.values().filter(|s|s.split==split).collect();
    let mut levels=Vec::new();
    for level in 0..3 {
     let labels:BTreeSet<_>=rows.iter().flat_map(|s|[s.labels[level].clone(),observed[&s.sample_id].labels[level].clone()]).collect();
     let labels:Vec<_>=labels.into_iter().collect();
     let index:BTreeMap<_,_>=labels.iter().enumerate().map(|(i,l)|(l,i)).collect();
     let mut cm=vec![vec![0usize;labels.len()];labels.len()];
     for s in &rows{cm[index[&s.labels[level]]][index[&observed[&s.sample_id].labels[level]]]+=1;}
     let mut f1=0.0;let mut correct=0usize;
     for i in 0..labels.len(){let tp=cm[i][i];correct+=tp;let denom=cm[i].iter().sum::<usize>()+cm.iter().map(|r|r[i]).sum::<usize>();if denom>0{f1+=2.0*tp as f64/denom as f64;}}
     levels.push(json!({"level":level+1,"accuracy":correct as f64/rows.len() as f64,"macro_f1":f1/labels.len() as f64,"support":rows.len(),"labels":labels,"confusion_matrix":cm}));
    }
    metrics[split]=json!(levels);
   }
   summary["metrics"]=metrics;
  }
 }
 summary["evaluation_trust"]=json!("Metrics recomputed from supplied predictions, not proof of model execution or external training isolation");
 Ok(summary)
}
fn need(ok:bool, message:&str)->Result<(),String>{if ok {Ok(())}else{Err(message.into())}}
fn text(s:&str)->bool{!s.trim().is_empty() && s.len()<=256 && !s.chars().any(char::is_control)}
fn ident(s:&str)->bool{!s.is_empty() && s.len()<=96 && s.bytes().all(|b|b.is_ascii_alphanumeric() || b"._-".contains(&b))}
pub fn digest(b:&[u8])->String{format!("{:x}",Sha256::digest(b))}
pub fn validate_dataset_value(v:Value)->Result<Value,String>{let d:Dataset=serde_json::from_value(v).map_err(|e|format!("Dataset schema: {e}"))?;validate_dataset(&d)}
pub fn validate_dataset(d:&Dataset)->Result<Value,String>{
 need(d.schema_version==1,"schema_version must be 1")?;
 need(matches!(d.purpose.as_str(),"real"|"synthetic_test"),"purpose real or synthetic_test required")?;
 need((d.purpose=="real" && d.label_provenance.kind=="human_verified") || (d.purpose=="synthetic_test" && d.label_provenance.kind=="synthetic_test"),"Human-verified labels required; telemetry predictions/confidence are not ground truth")?;
 need(text(&d.label_provenance.verified_by)&&text(&d.label_provenance.evidence),"Label verifier and evidence required (attestation, not independent verification)")?;
 let groups:BTreeSet<_>=d.group_ids.iter().collect();
 need(groups.len()==d.group_ids.len() && groups.len()>=3 && groups.iter().all(|s|ident(s)),"At least 3 unique group identifiers required")?;
 need(d.purpose!="real" || groups.len()==18,"Class A real contract requires datasets from exactly 18 groups, not 18 rows")?;
 need((1..=32).contains(&d.feature_names.len()) && d.feature_names.len()==d.feature_units.len() && d.feature_names.iter().all(|s|ident(s)&&!s.starts_with("pred_")&&!s.starts_with("conf_")) && d.feature_units.iter().all(|s|text(s)) && d.feature_names.iter().collect::<BTreeSet<_>>().len()==d.feature_names.len(),"Unique raw sensor features and units required; predictions forbidden")?;
 need((1..=3600000).contains(&d.interval_ms) && (3..=20000).contains(&d.samples.len()),"Invalid interval or sample count (3..20000)")?;
 let mut ids=BTreeSet::new(); let mut seen_groups=BTreeMap::new(); let mut sessions=BTreeMap::new(); let mut hierarchy=BTreeMap::new(); let mut splits=BTreeMap::new(); let mut points=0usize;
 for s in &d.samples {
  need(ident(&s.sample_id)&&ident(&s.session_id)&&ids.insert(&s.sample_id),"Unique safe sample_id and session_id required")?;
  need(groups.contains(&s.group_id)&&matches!(s.split.as_str(),"train"|"validation"|"test"),"Declared group and explicit train/validation/test split required")?;
  if let Some(old)=seen_groups.insert(&s.group_id,&s.split){need(old==&s.split,"Group leakage across splits")?;}
  if let Some(old)=sessions.insert(&s.session_id,(&s.group_id,&s.split)){need(old==(&s.group_id,&s.split),"Session leakage across groups or splits")?;}
  *splits.entry(s.split.clone()).or_insert(0usize)+=1;
  need(s.labels.iter().all(|l|text(l)),"Three genuine ground-truth labels required")?;
  // L2 names may repeat under different L1 (e.g. roast); L3 identifies one parent path.
  if let Some(old)=hierarchy.insert(&s.labels[2],(&s.labels[0],&s.labels[1])){need(old==(&s.labels[0],&s.labels[1]),"L3 label must have exactly one L1/L2 parent path")?;}
  points+=s.values.len();
  need((2..=20000).contains(&s.values.len()) && points<=500000 && s.values.iter().all(|r|r.len()==d.feature_names.len()&&r.iter().all(|v|v.is_finite()&&v.abs()<=1e12)),"Raw finite fixed-width time series required; bounds exceeded")?;
 }
 need(seen_groups.len()==groups.len()&&splits.len()==3,"Every declared group and all three splits must contain samples")?;
 let mut normalized=d.clone(); normalized.samples.sort_by(|a,b|a.sample_id.cmp(&b.sample_id)); normalized.group_ids.sort();
 let dataset_fp=digest(&serde_json::to_vec(&normalized).unwrap());
 let tests:Vec<_>=normalized.samples.iter().filter(|s|s.split=="test").collect();
 let test_fp=digest(&serde_json::to_vec(&json!({"features":d.feature_names,"units":d.feature_units,"interval_ms":d.interval_ms,"samples":tests})).unwrap());
 Ok(json!({"dataset_fingerprint":dataset_fp,"test_fingerprint":test_fp,"sample_count":d.samples.len(),"group_count":groups.len(),"splits":splits,"purpose":d.purpose,"label_status":"uploader_attestation_not_independently_verified"}))
}
