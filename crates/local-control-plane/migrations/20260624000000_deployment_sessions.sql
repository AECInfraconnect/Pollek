-- deployment sessions
CREATE TABLE IF NOT EXISTS deployment_sessions (
    deployment_id TEXT PRIMARY KEY,
    policy_id TEXT NOT NULL,
    policy_version TEXT NOT NULL,
    requested_control_level TEXT NOT NULL,
    target_scope_json TEXT NOT NULL,
    status TEXT NOT NULL,
    created_by TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);

-- deployment events
CREATE TABLE IF NOT EXISTS deployment_events (
    event_id TEXT PRIMARY KEY,
    deployment_id TEXT NOT NULL,
    agent_id TEXT,
    entity_id TEXT,
    policy_id TEXT NOT NULL,
    phase TEXT NOT NULL,
    status TEXT NOT NULL,
    title_json TEXT NOT NULL,
    detail_json TEXT NOT NULL,
    technical_detail_json TEXT,
    user_action_json TEXT,
    created_at TIMESTAMP NOT NULL,
    correlation_id TEXT NOT NULL,
    FOREIGN KEY (deployment_id) REFERENCES deployment_sessions(deployment_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_deployment_events_session ON deployment_events(deployment_id);
