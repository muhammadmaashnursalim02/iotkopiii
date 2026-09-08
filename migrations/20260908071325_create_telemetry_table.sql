CREATE TABLE telemetry_data (
    id SERIAL PRIMARY KEY,
    device_id VARCHAR(50) NOT NULL,
    coffee_id VARCHAR(50) NOT NULL,
    prediction VARCHAR(50) NOT NULL,
    confidence REAL NOT NULL,
    temperature REAL NOT NULL,
    humidity REAL NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);