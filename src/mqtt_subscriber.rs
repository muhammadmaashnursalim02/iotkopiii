use crate::{config::TOPIC, models::TelemetryPayload};
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS, SubscribeReasonCode};
use sqlx::PgPool;
use std::time::Duration;

pub async fn persist(db_pool: &PgPool, data: &TelemetryPayload) -> Result<(), sqlx::Error> {
    let query = "
                    INSERT INTO telemetry_data 
                    (device_id, coffee_id, s1_mq3, s2_mq6, s3_mq7, s4_mq135, s5_tgs2600, s6_tgs2602, s7_tgs2611, s8_tgs2620, temperature, humidity, pred_level_1, conf_level_1, pred_level_2, conf_level_2, pred_level_3, conf_level_3) 
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
                ";

    let result = sqlx::query(query)
        .bind(&data.device_id)
        .bind(&data.coffee_id)
        .bind(data.s1_mq3)
        .bind(data.s2_mq6)
        .bind(data.s3_mq7)
        .bind(data.s4_mq135)
        .bind(data.s5_tgs2600)
        .bind(data.s6_tgs2602)
        .bind(data.s7_tgs2611)
        .bind(data.s8_tgs2620)
        .bind(data.temperature)
        .bind(data.humidity)
        .bind(&data.pred_level_1)
        .bind(data.conf_level_1)
        .bind(&data.pred_level_2)
        .bind(data.conf_level_2)
        .bind(&data.pred_level_3)
        .bind(data.conf_level_3)
        .execute(db_pool)
        .await;

    result.map(|_| ())
}

pub async fn start_mqtt_listener(db_pool: PgPool, options: MqttOptions) -> Result<(), String> {
    // mqtt_options supplies verified public TLS roots. Manual ACK follows DB commit.
    let mut options = options;
    options.set_manual_acks(true);
    let (client, mut eventloop) = AsyncClient::new(options, 64);
    let (tx, mut rx) = tokio::sync::mpsc::channel::<rumqttc::Publish>(128);
    let ack_client = client.clone();
    let mut worker = tokio::spawn(async move {
        while let Some(p) = rx.recv().await {
            let data: TelemetryPayload = match serde_json::from_slice(&p.payload) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("MQTT invalid telemetry JSON: {e}; rejected");
                    ack_client.ack(&p).await.map_err(|e| e.to_string())?;
                    continue;
                }
            };
            let mut delay = 1;
            loop {
                match persist(&db_pool, &data).await {
                    Ok(()) => {
                        println!("DB committed telemetry device_id={}", data.device_id);
                        break;
                    }
                    Err(e) => {
                        eprintln!("DB insert failed: {e}; retry in {delay}s (MQTT ACK withheld)");
                        tokio::time::sleep(Duration::from_secs(delay)).await;
                        delay = (delay * 2).min(30);
                    }
                }
            }
            ack_client.ack(&p).await.map_err(|e| e.to_string())?;
        }
        Ok::<(), String>(())
    });
    let mut delay = 1;
    loop {
        tokio::select! {
            outcome = &mut worker => return Err(format!("MQTT DB worker stopped: {outcome:?}")),
            event = eventloop.poll() => match event {
                Ok(Event::Incoming(Incoming::ConnAck(ack))) => {
                    println!("MQTT ConnAck {:?}", ack.code);
                    delay = 1;
                    if let Err(e) = client.try_subscribe(TOPIC, QoS::AtLeastOnce) {
                        worker.abort(); return Err(format!("MQTT subscribe enqueue failed: {e}"));
                    }
                }
                Ok(Event::Incoming(Incoming::SubAck(ack))) => {
                    if ack.return_codes.is_empty() || ack.return_codes.contains(&SubscribeReasonCode::Failure) {
                        worker.abort(); return Err("MQTT SubAck rejected topic authorization".into());
                    }
                    println!("MQTT SubAck topic={TOPIC} qos={:?}", ack.return_codes);
                }
                Ok(Event::Incoming(Incoming::Publish(p))) => {
                    if p.topic != TOPIC { continue; }
                    if let Err(e) = tx.try_send(p) {
                        worker.abort();
                        return Err(format!("MQTT telemetry queue unavailable: {e}; uncommitted message NOT acknowledged"));
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("MQTT connection error: {e}; reconnect in {delay}s");
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                    delay = (delay * 2).min(30);
                }
            }
        }
    }
}
