-- v0.3.0 Migration: Update push notification configs to support multiple configs per task
-- This migration enhances the push_notification_configs table to support the v0.3.0 spec
-- PostgreSQL version

-- Drop the old table (backing up data if needed in production)
DROP TABLE IF EXISTS push_notification_configs CASCADE;

-- Create new table with support for multiple configs per task
CREATE TABLE IF NOT EXISTS push_notification_configs (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    url TEXT NOT NULL,
    token TEXT,
    authentication JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
);

-- Index for efficient lookups
CREATE INDEX IF NOT EXISTS idx_push_configs_task_id ON push_notification_configs(task_id);

-- Function to automatically update the updated_at timestamp
CREATE OR REPLACE FUNCTION update_push_configs_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Trigger to automatically update the updated_at timestamp
DROP TRIGGER IF EXISTS update_push_configs_updated_at_trigger ON push_notification_configs;
CREATE TRIGGER update_push_configs_updated_at_trigger
    BEFORE UPDATE ON push_notification_configs
    FOR EACH ROW
    EXECUTE FUNCTION update_push_configs_updated_at();
