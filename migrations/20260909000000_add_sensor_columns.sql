-- Idempotent additive repair. Never replay destructive historical migration 20260908100000.
CREATE TABLE IF NOT EXISTS telemetry_data (
    id SERIAL PRIMARY KEY, device_id VARCHAR(50) NOT NULL, coffee_id VARCHAR(50) NOT NULL,
    temperature REAL NOT NULL, humidity REAL NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
ALTER TABLE telemetry_data
    ADD COLUMN IF NOT EXISTS s1_mq3 REAL,
    ADD COLUMN IF NOT EXISTS s2_mq6 REAL,
    ADD COLUMN IF NOT EXISTS s3_mq7 REAL,
    ADD COLUMN IF NOT EXISTS s4_mq135 REAL,
    ADD COLUMN IF NOT EXISTS s5_tgs2600 REAL,
    ADD COLUMN IF NOT EXISTS s6_tgs2602 REAL,
    ADD COLUMN IF NOT EXISTS s7_tgs2611 REAL,
    ADD COLUMN IF NOT EXISTS s8_tgs2620 REAL,
    ADD COLUMN IF NOT EXISTS pred_level_1 VARCHAR(50),
    ADD COLUMN IF NOT EXISTS conf_level_1 REAL,
    ADD COLUMN IF NOT EXISTS pred_level_2 VARCHAR(50),
    ADD COLUMN IF NOT EXISTS conf_level_2 REAL,
    ADD COLUMN IF NOT EXISTS pred_level_3 VARCHAR(50),
    ADD COLUMN IF NOT EXISTS conf_level_3 REAL;
-- Legacy rows keep NULL for measurements/predictions that were never observed.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='telemetry_data' AND column_name='prediction') THEN
        ALTER TABLE telemetry_data ALTER COLUMN prediction DROP NOT NULL;
        ALTER TABLE telemetry_data ALTER COLUMN confidence DROP NOT NULL;
        UPDATE telemetry_data SET pred_level_1=prediction, conf_level_1=confidence WHERE pred_level_1 IS NULL;
    END IF;
END $$;
