//! Offline authoritative schema, fingerprint and evaluation validator; no DB or .env.
use std::io::Read;
fn main(){if let Err(e)=run(){eprintln!("{e}");std::process::exit(1)}}
fn run()->Result<(),String>{
 let mut bytes=Vec::new();std::io::stdin().take((backend_iot::mlops::MAX_PACKAGE+1) as u64).read_to_end(&mut bytes).map_err(|_|"Cannot read input")?;
 if bytes.len()>backend_iot::mlops::MAX_PACKAGE{return Err("Package exceeds 32 MiB".into());}
 let p:backend_iot::mlops::Package=serde_json::from_slice(&bytes).map_err(|e|format!("Package schema: {e}"))?;
 let summary=backend_iot::mlops::validate_package(&p)?;
 println!("{}",serde_json::to_string(&summary).unwrap());Ok(())
}
