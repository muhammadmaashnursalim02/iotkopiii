//! Read-only subscriber probe by default. Publishing requires explicit separate env credentials.
use backend_iot::config::{mqtt_options, TOPIC};
use rumqttc::{AsyncClient, Event, Incoming, QoS, SubscribeReasonCode};
use std::time::Duration;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let publish = std::env::args().any(|a| a == "--publish");
    let result = tokio::time::timeout(Duration::from_secs(25), probe(publish)).await;
    match result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            eprintln!("MQTT probe failed: {e}");
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!("MQTT probe timeout: no confirmed acknowledgement");
            std::process::exit(1);
        }
    }
}
async fn probe(publish: bool) -> Result<(), String> {
    let (client, mut events) = AsyncClient::new(mqtt_options(publish)?, 10);
    loop {
        match events.poll().await.map_err(|e| e.to_string())? {
            Event::Incoming(Incoming::ConnAck(ack)) => {
                println!("MQTT ConnAck {:?}", ack.code);
                if publish {
                    let file = backend_iot::config::required("MQTT_TEST_PAYLOAD_FILE")?;
                    let bytes = std::fs::read(file).map_err(|e| e.to_string())?;
                    let data: backend_iot::models::TelemetryPayload =
                        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                    if !data.device_id.starts_with("SIMTEST-") {
                        return Err("Simulated payload device_id must start SIMTEST-".into());
                    }
                    client
                        .publish(TOPIC, QoS::AtLeastOnce, false, bytes)
                        .await
                        .map_err(|e| e.to_string())?;
                } else {
                    client
                        .subscribe(TOPIC, QoS::AtLeastOnce)
                        .await
                        .map_err(|e| e.to_string())?;
                }
            }
            Event::Incoming(Incoming::SubAck(ack)) if !publish => {
                if ack.return_codes.is_empty()
                    || ack.return_codes.contains(&SubscribeReasonCode::Failure)
                {
                    return Err("SubAck authorization denied".into());
                }
                println!(
                    "MQTT SubAck topic={TOPIC} qos={:?}; probe read-only, no DB delivery claimed",
                    ack.return_codes
                );
                return Ok(());
            }
            Event::Incoming(Incoming::PubAck(ack)) if publish => {
                println!("MQTT PubAck pkid={}; simulated publish broker-confirmed, DB readback still required", ack.pkid);
                return Ok(());
            }
            _ => {}
        }
    }
}
