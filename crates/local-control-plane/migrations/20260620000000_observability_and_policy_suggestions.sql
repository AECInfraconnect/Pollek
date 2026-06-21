-- Observation Aggregator Tables
CREATE TABLE observation_events (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    trace_id TEXT NOT NULL,
    agent_id TEXT,
    shadow_candidate_id TEXT,
    tool_id TEXT,
    resource_id TEXT,
    surface TEXT NOT NULL,
    action TEXT NOT NULL,
    pep_type TEXT,
    risk_level TEXT,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    payload_json TEXT NOT NULL
);
CREATE INDEX idx_observation_agent ON observation_events(agent_id);
CREATE INDEX idx_observation_target ON observation_events(resource_id);

CREATE TABLE cost_ledger (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT,
    input_tokens INTEGER,
    output_tokens INTEGER,
    total_tokens INTEGER,
    input_cost REAL,
    output_cost REAL,
    total_cost REAL,
    currency TEXT,
    estimated BOOLEAN,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_cost_ledger_agent ON cost_ledger(agent_id);

-- Policy Suggestion Tables
CREATE TABLE policy_suggestions (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    target_agent_id TEXT,
    target_resource_id TEXT,
    suggestion_type TEXT,
    status TEXT DEFAULT 'pending',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    data_json TEXT NOT NULL
);
CREATE INDEX idx_policy_sug_agent ON policy_suggestions(target_agent_id);
