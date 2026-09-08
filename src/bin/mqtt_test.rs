use rumqttc::{AsyncClient, MqttOptions, QoS, Transport};
use std::time::Duration;
use tokio::time;
use rustls::ClientConfig;

#[tokio::main]
async fn main() {
    let host = "yb413bb1.ala.asia-southeast1.emqxsl.com";
    let port = 8883;
    let topic = "iot5a/g09/telemetry";

    // GANTI DENGAN USERNAME & PASSWORD YANG KAMU BUAT DI MENU AUTHENTICATION EMQX
    let emqx_username = "esp32_kelas_a"; 
    let emqx_password = "PasswordA123!"; 

    // Payload dengan 8 sensor dan klasifikasi bertingkat
    let payload = r#"{
        "device_id": "DAQ01",
        "coffee_id": "C09",
        "s1_mq3": 1.65,
        "s2_mq6": 1.27,
        "s3_mq7": 1.23,
        "s4_mq135": 1.87,
        "s5_tgs2600": 1.01,
        "s6_tgs2602": 0.95,
        "s7_tgs2611": 1.12,
        "s8_tgs2620": 1.30,
        "temperature": 32.5,
        "humidity": 65.2,
        "pred_level_1": "Arabica",
        "conf_level_1": 0.98,
        "pred_level_2": "Roasted",
        "conf_level_2": 0.95,
        "pred_level_3": "Unknown",
        "conf_level_3": 0.45
    }"#;

    // Setup koneksi aman (TLS/SSL) untuk port 8883
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let client_config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let mut mqttoptions = MqttOptions::new("rust_tester_emqx", host, port);
    mqttoptions.set_credentials(emqx_username, emqx_password);
    mqttoptions.set_keep_alive(Duration::from_secs(5));
    mqttoptions.set_transport(Transport::tls_with_config(client_config.into()));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    println!("Menghubungkan ke EMQX ({}:{}) dengan user: {}...", host, port, emqx_username);

    // Kirim data
    match client.publish(topic, QoS::AtLeastOnce, false, payload.as_bytes()).await {
        Ok(_) => println!("✅ Berhasil Request Publish!"),
        Err(e) => println!("❌ Gagal Publish: {:?}", e),
    }

    // Biarkan eventloop jalan sebentar agar data terkirim beneran
    for _ in 0..3 {
        if let Ok(event) = eventloop.poll().await {
            println!("Status Network EMQX: {:?}", event);
        }
        time::sleep(Duration::from_secs(1)).await;
    }
}