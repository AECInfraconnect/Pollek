-- Create pdp_runtimes table
CREATE TABLE IF NOT EXISTS pdp_runtimes (
  tenant_id TEXT NOT NULL,
  id TEXT NOT NULL,
  name TEXT NOT NULL,
  category TEXT NOT NULL,
  kind TEXT NOT NULL,
  enabled BOOLEAN NOT NULL DEFAULT 1,
  status TEXT NOT NULL,
  endpoint TEXT,
  auth_ref TEXT,
  capabilities_json TEXT NOT NULL DEFAULT '[]',
  health_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_id, id)
);

-- Create pdp_routes table
CREATE TABLE IF NOT EXISTS pdp_routes (
  tenant_id TEXT NOT NULL,
  id TEXT NOT NULL,
  name TEXT NOT NULL,
  enabled BOOLEAN NOT NULL DEFAULT 1,
  priority INTEGER NOT NULL DEFAULT 0,
  match_cond_json TEXT NOT NULL DEFAULT '{}',
  mode TEXT NOT NULL,
  primary_pdp_id TEXT NOT NULL,
  fallback_pdp_ids_json TEXT NOT NULL DEFAULT '[]',
  shadow_pdp_ids_json TEXT NOT NULL DEFAULT '[]',
  merge_strategy TEXT NOT NULL,
  failure_behavior TEXT NOT NULL,
  timeout_ms INTEGER NOT NULL DEFAULT 1000,
  max_retries INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_id, id)
);

-- Migrate data from registry_objects
INSERT INTO pdp_runtimes (
  tenant_id, id, name, category, kind, enabled, status, endpoint, auth_ref, capabilities_json, health_json, created_at, updated_at
)
SELECT 
  tenant_id,
  object_id as id,
  json_extract(data_json, '$.name') as name,
  json_extract(data_json, '$.category') as category,
  json_extract(data_json, '$.kind') as kind,
  COALESCE(json_extract(data_json, '$.enabled'), 1) as enabled,
  json_extract(data_json, '$.status') as status,
  json_extract(data_json, '$.endpoint') as endpoint,
  json_extract(data_json, '$.auth_ref') as auth_ref,
  COALESCE(json_extract(data_json, '$.capabilities'), '[]') as capabilities_json,
  json_extract(data_json, '$.health') as health_json,
  created_at,
  updated_at
FROM registry_objects
WHERE object_type = 'pdp_runtime'
ON CONFLICT(tenant_id, id) DO NOTHING;

INSERT INTO pdp_routes (
  tenant_id, id, name, enabled, priority, match_cond_json, mode, primary_pdp_id, fallback_pdp_ids_json, shadow_pdp_ids_json, merge_strategy, failure_behavior, timeout_ms, max_retries, created_at, updated_at
)
SELECT 
  tenant_id,
  object_id as id,
  json_extract(data_json, '$.name') as name,
  COALESCE(json_extract(data_json, '$.enabled'), 1) as enabled,
  COALESCE(json_extract(data_json, '$.priority'), 0) as priority,
  COALESCE(json_extract(data_json, '$.match'), '{}') as match_cond_json,
  json_extract(data_json, '$.mode') as mode,
  json_extract(data_json, '$.primary_pdp_id') as primary_pdp_id,
  COALESCE(json_extract(data_json, '$.fallback_pdp_ids'), '[]') as fallback_pdp_ids_json,
  COALESCE(json_extract(data_json, '$.shadow_pdp_ids'), '[]') as shadow_pdp_ids_json,
  json_extract(data_json, '$.merge_strategy') as merge_strategy,
  json_extract(data_json, '$.failure_behavior') as failure_behavior,
  COALESCE(json_extract(data_json, '$.timeout_ms'), 1000) as timeout_ms,
  COALESCE(json_extract(data_json, '$.max_retries'), 0) as max_retries,
  created_at,
  updated_at
FROM registry_objects
WHERE object_type = 'pdp_route'
ON CONFLICT(tenant_id, id) DO NOTHING;

-- Cleanup registry_objects to avoid dual source of truth for migrated records
DELETE FROM registry_objects WHERE object_type IN ('pdp_runtime', 'pdp_route');
