-- Initial schema for A2A task storage (MySQL version)
-- This creates the core tables needed for persistent task storage

-- Tasks table - stores main task information
CREATE TABLE IF NOT EXISTS tasks (
    id VARCHAR(255) PRIMARY KEY,
    context_id VARCHAR(255) NOT NULL,
    status_state VARCHAR(50) NOT NULL CHECK (status_state IN ('submitted', 'working', 'input-required', 'completed', 'canceled', 'failed', 'rejected', 'auth-required', 'unknown')),
    status_message TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    metadata JSON,
    artifacts JSON,
    INDEX idx_tasks_context_id (context_id),
    INDEX idx_tasks_created_at (created_at),
    INDEX idx_tasks_status_state (status_state)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Task history table - stores chronological task updates
CREATE TABLE IF NOT EXISTS task_history (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    task_id VARCHAR(255) NOT NULL,
    timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    status_state VARCHAR(50) NOT NULL CHECK (status_state IN ('submitted', 'working', 'input-required', 'completed', 'canceled', 'failed', 'rejected', 'auth-required', 'unknown')),
    message JSON,
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
    INDEX idx_task_history_task_id (task_id),
    INDEX idx_task_history_timestamp (timestamp)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Push notification configurations
CREATE TABLE IF NOT EXISTS push_notification_configs (
    task_id VARCHAR(255) PRIMARY KEY,
    webhook_url TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
