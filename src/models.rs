use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct TelemetryPayload {
    pub device_id: String,
    pub coffee_id: String,
    pub s1_mq3: f32,
    pub s2_mq6: f32,
    pub s3_mq7: f32,
    pub s4_mq135: f32,
    pub s5_tgs2600: f32,
    pub s6_tgs2602: f32,
    pub s7_tgs2611: f32,
    pub s8_tgs2620: f32,
    pub temperature: f32,
    pub humidity: f32,
    pub pred_level_1: String,
    pub conf_level_1: f32,
    pub pred_level_2: String,
    pub conf_level_2: f32,
    pub pred_level_3: String,
    pub conf_level_3: f32,
}
