use rumqttc::{MqttOptions, Transport};
use std::time::Duration;

pub const TOPIC: &str = "iot5a/g09/telemetry";

pub fn required(name: &str) -> Result<String, String> {
    std::env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty() && v != "***")
        .ok_or_else(|| format!("{name} missing/placeholder; restore locally in .env"))
}

pub fn mqtt_options(publisher: bool) -> Result<MqttOptions, String> {
    let host = required("EMQX_HOST")?;
    let prefix = if publisher { "EMQX_PUBLISHER" } else { "EMQX" };
    let username = required(&format!("{prefix}_USERNAME"))?;
    let password = required(&format!("{prefix}_PASSWORD"))?;
    let id = format!(
        "rust_g09_{}_{}",
        if publisher { "test" } else { "subscriber" },
        std::process::id()
    );
    let mut options = MqttOptions::new(id, host, 8883);
    options.set_credentials(username, password);
    options.set_keep_alive(Duration::from_secs(30));
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let tls = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    options.set_transport(Transport::tls_with_config(tls.into()));
    Ok(options)
}
