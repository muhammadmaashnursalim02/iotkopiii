use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct TelemetryPayload {
    pub device_id: String,
    pub coffee_id: String,
    
    // Klasifikasi Bertingkat (Tugas G09)
    pub pred_level_1: String, // Arabica/Robusta
    pub conf_level_1: f32,
    
    pub pred_level_2: String, // Green/Roasted
    pub conf_level_2: f32,
    
    pub pred_level_3: String, // Coffee ID (Sample spesifik / Unknown)
    pub conf_level_3: f32,
    
    pub temperature: f32,
    pub humidity: f32,
}