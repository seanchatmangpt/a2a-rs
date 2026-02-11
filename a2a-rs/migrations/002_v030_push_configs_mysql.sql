-- v0.3.0 Migration: Update push notification configs to support multiple configs per task
-- This migration enhances the push_notification_configs table to support the v0.3.0 spec
-- MySQL version

-- Drop the old table (backing up data if needed in production)
DROP TABLE IF EXISTS push_notification_configs;

-- Create new table with support for multiple configs per task
CREATE TABLE IF NOT EXISTS push_notification_configs (
    id VARCHAR(255) PRIMARY KEY,
    task_id VARCHAR(255) NOT NULL,
    url TEXT NOT NULL,
    token TEXT,
    authentication JSON,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
    INDEX idx_push_configs_task_id (task_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
