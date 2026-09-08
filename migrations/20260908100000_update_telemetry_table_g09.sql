DROP TABLE IF EXISTS telemetry_data;

CREATE TABLE telemetry_data (
    id SERIAL PRIMARY KEY,
    device_id VARCHAR(50) NOT NULL,
    coffee_id VARCHAR(50) NOT NULL,
    
    pred_level_1 VARCHAR(50) NOT NULL,
    conf_level_1 REAL NOT NULL,
    
    pred_level_2 VARCHAR(50) NOT NULL,
    conf_level_2 REAL NOT NULL,
    
    pred_level_3 VARCHAR(50) NOT NULL,
    conf_level_3 REAL NOT NULL,
    
    temperature REAL NOT NULL,
    humidity REAL NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);