-- Add tenant scoping to the cost ledger so per-tenant cost summaries are
-- correct instead of aggregating across all tenants. Existing rows get an
-- empty tenant_id (they predate multi-tenant scoping); new rows carry the
-- observation event's real tenant_id.
ALTER TABLE cost_ledger ADD COLUMN tenant_id TEXT NOT NULL DEFAULT '';
CREATE INDEX idx_cost_ledger_tenant ON cost_ledger(tenant_id);
