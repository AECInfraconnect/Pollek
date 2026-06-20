-- Observation Aggregator Tables
CREATE TABLE observation_events (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    action_type TEXT NOT NULL,
    target_resource TEXT,
    tool_used TEXT,
    tokens_consumed INTEGER,
    cost_incurred REAL,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    context_json TEXT
);
CREATE INDEX idx_observation_agent ON observation_events(agent_id);
CREATE INDEX idx_observation_target ON observation_events(target_resource);

CREATE TABLE cost_ledger (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT,
    period_start DATETIME,
    period_end DATETIME,
    total_tokens INTEGER,
    total_cost REAL
);
CREATE INDEX idx_cost_ledger_agent ON cost_ledger(agent_id);

-- Policy Suggestion Tables
CREATE TABLE policy_suggestions (
    id TEXT PRIMARY KEY,
    target_agent_id TEXT,
    target_resource_id TEXT,
    suggestion_type TEXT,
    reasoning TEXT,
    proposed_policy_json TEXT,
    status TEXT DEFAULT 'pending',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_policy_sug_agent ON policy_suggestions(target_agent_id);
