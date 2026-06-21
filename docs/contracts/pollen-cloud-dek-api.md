# Pollen Cloud ↔ DEK API Contracts

This document is the Single Source of Truth for the APIs and data models used between the Pollen Cloud (and Mock-Cloud) and the Data Execution Kernel (DEK).

## JSON Schemas
All objects exchanged must adhere to their respective JSON schemas:

- `tenant.schema.json`
- `principal.schema.json`
- `dek-device.schema.json`
- `ai-agent.schema.json`
- `mcp-server.schema.json`
- `tool.schema.json`
- `resource.schema.json`
- `relationship.schema.json`
- `policy.schema.json`
- `pep-deployment.schema.json`
- `telemetry-event.schema.json`

## API Endpoints

### 1. Registry APIs
Used to manage the entities within a tenant.

- `GET    /v1/tenants/{tenant_id}/registry/agents`
- `POST   /v1/tenants/{tenant_id}/registry/agents`
- `GET    /v1/tenants/{tenant_id}/registry/resources`
- `POST   /v1/tenants/{tenant_id}/registry/resources`
- `GET    /v1/tenants/{tenant_id}/registry/mcp-servers`
- `POST   /v1/tenants/{tenant_id}/registry/mcp-servers`
- `GET    /v1/tenants/{tenant_id}/registry/relationships`
- `POST   /v1/tenants/{tenant_id}/registry/relationships`

### 2. Policy APIs
Used to manage policies and perform simulation testing.

- `GET    /v1/tenants/{tenant_id}/policies`
- `POST   /v1/tenants/{tenant_id}/policies`
- `POST   /v1/tenants/{tenant_id}/policies/{policy_id}/publish`
- `POST   /v1/tenants/{tenant_id}/policies/{policy_id}/rollback`
- `POST   /v1/tenants/{tenant_id}/policies/simulate`

### 3. Bundle APIs
Used by DEK to fetch TUF-lite metadata and the securely signed bundles.

- `GET  /v1/tenants/{tenant_id}/devices/{device_id}/bundles/metadata/root.json`
- `GET  /v1/tenants/{tenant_id}/devices/{device_id}/bundles/metadata/timestamp.json`
- `GET  /v1/tenants/{tenant_id}/devices/{device_id}/bundles/metadata/snapshot.json`
- `GET  /v1/tenants/{tenant_id}/devices/{device_id}/bundles/metadata/targets.json`
- `GET  /v1/tenants/{tenant_id}/devices/{device_id}/bundles/artifacts/{sha256}`
- `POST /v1/tenants/{tenant_id}/bundles/publish`
- `POST /v1/tenants/{tenant_id}/bundles/canary`
- `POST /v1/tenants/{tenant_id}/bundles/rollback`

### 4. DEK Config APIs
Device-specific configurations and credentials.

- `GET /v1/tenants/{tenant_id}/devices/{device_id}/config`
- `GET /v1/tenants/{tenant_id}/devices/{device_id}/pep-config`
- `GET /v1/tenants/{tenant_id}/devices/{device_id}/trusted-keys`
- `GET /v1/tenants/{tenant_id}/devices/{device_id}/capabilities`

### 5. Telemetry APIs
Used by DEK to send telemetry to the cloud.

- `POST /v1/telemetry/events`
- `POST /v1/telemetry/decision-logs`
- `POST /v1/telemetry/security-events`
- `POST /v1/telemetry/runtime-metrics`
- `POST /v1/telemetry/traces`
- `POST /v1/telemetry/ebpf-events`
- `POST /v1/metrics`
- `GET  /admin/api/telemetry/recent`
- `GET  /admin/api/traces/{trace_id}`

## Enforced Headers
- `Authorization: Bearer <token>`
- `Content-Type: application/json`
- `X-Pollen-Device-Id: <device_id>`
- `X-Pollen-Tenant-Id: <tenant_id>`
