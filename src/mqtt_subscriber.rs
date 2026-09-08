use rumqttc::{AsyncClient, MqttOptions, QoS, Event, Incoming};
use std::time::Duration;
use sqlx::PgPool;
use crate::models::TelemetryPayload;

pub async fn start_mqtt_listener(db_pool: PgPool) {
    let token = std::env::var("THINGSBOARD_TOKEN").expect("THINGSBOARD_TOKEN kosong");
    let host = std::env::var("THINGSBOARD_URL").unwrap_or_else(|_| "demo.thingsboard.io".to_string());
    // Hapus "https://" jika ada di .env
    let clean_host = host.replace("https://", "").replace("http://", "");
    
    let mut mqttoptions = MqttOptions::new("rust_backend_subscriber", clean_host, 1883);
    mqttoptions.set_credentials(token, "");
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
    
    // Subscribe ke topik ThingsBoard tempat ESP32 mengirim data
    client.subscribe("v1/devices/me/telemetry", QoS::AtMostOnce).await.unwrap();
    println!("📡 MQTT Listener aktif! Menunggu data dari ESP32 di ThingsBoard...");

    loop {
        if let Ok(Event::Incoming(Incoming::Publish(p))) = eventloop.poll().await {
            let payload_str = String::from_utf8_lossy(&p.payload);
            
            // Coba Parse JSON
            if let Ok(data) = serde_json::from_str::<TelemetryPayload>(&payload_str) {
                println!("📥 Data Baru Masuk: {:?}", data);
                
                // Simpan ke Azure DB (Skema G09)
                let query = "
                    INSERT INTO telemetry_data 
                    (device_id, coffee_id, pred_level_1, conf_level_1, pred_level_2, conf_level_2, pred_level_3, conf_level_3, temperature, humidity) 
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                ";
                
                let result = sqlx::query(query)
                    .bind(&data.device_id)
                    .bind(&data.coffee_id)
                    .bind(&data.pred_level_1)
                    .bind(data.conf_level_1)
                    .bind(&data.pred_level_2)
                    .bind(data.conf_level_2)
                    .bind(&data.pred_level_3)
                    .bind(data.conf_level_3)
                    .bind(data.temperature)
                    .bind(data.humidity)
                    .execute(&db_pool)
                    .await;
                    
                match result {
                    Ok(_) => println!("💾 Tersimpan di Azure DB!"),
                    Err(e) => println!("❌ Gagal simpan DB: {}", e),
                }
            } else {
                println!("⚠️ Data masuk tapi JSON tidak valid: {}", payload_str);
            }
        }
    }
}