import "@typespec/http";
import "@typespec/rest";
import "./common.tsp";

using TypeSpec.Http;
using TypeSpec.Rest;

namespace Pollen.Contract;

enum PdpRuntimeCategory {
  LocalEngine: "local_engine",
  RemoteConnector: "remote_connector",
  PollenCloud: "pollen_cloud",
}

enum PdpKind {
  PolicyRouter: "policy_router",
  CedarLocal: "cedar_local",
  OpaWasm: "opa_wasm",
  WasmPlugin: "wasm_plugin",
  OpaServer: "opa_server",
  OpenfgaServer: "openfga_server",
  CedarHttp: "cedar_http",
  CustomHttp: "custom_http",
  CustomGrpc: "custom_grpc",
  PollenCloudPdp: "pollen_cloud_pdp",
}

enum PdpRuntimeStatus {
  Installed: "installed",
  NotConfigured: "not_configured",
  Loading: "loading",
  Ready: "ready",
  Degraded: "degraded",
  Error: "error",
  Disabled: "disabled",
}

model PdpProbeResult {
  ok: boolean;
  effect: string;
  reason: string;
  latency_ms: int64;
  decision_id: string;
  details: unknown;
}

model PdpRuntime {
  id: string;
  name: string;
  category: PdpRuntimeCategory;
  kind: PdpKind;
  mode: string;
  system_managed: boolean;
  enabled: boolean;
  status: PdpRuntimeStatus;
  capabilities: string[];
  endpoint?: string;
  auth_ref?: string;
  config_source: string;
  active_bundle_id?: string;
  active_bundle_hash?: string;
  last_activated_at?: string;
  last_probe?: PdpProbeResult;
  health?: unknown;
  created_at: string;
  updated_at: string;
}

enum PdpRouteMode {
  LocalOnly: "local_only",
  LocalPrimaryRemoteFallback: "local_primary_remote_fallback",
  RemotePrimaryLocalFallback: "remote_primary_local_fallback",
  CloudPrimaryLocalFallback: "cloud_primary_local_fallback",
  ShadowRemote: "shadow_remote",
  MirrorAuditOnly: "mirror_audit_only",
  StrictRemote: "strict_remote",
}

enum PdpFailureBehavior {
  Deny: "deny",
  Fallback: "fallback",
  LastKnownGood: "last_known_good",
  Allow: "allow",
  NotApplicable: "not_applicable",
}

model RouteMatch {
  agent_ids?: string[];
  tool_categories?: string[];
  resource_types?: string[];
  protocols?: string[];
  sensitivities?: string[];
  environments?: string[];
  risk_tiers?: string[];
}

model PdpRouteRule {
  id: string;
  name: string;
  enabled: boolean;
  priority: int32;
  description?: string;
  match_cond: RouteMatch;
  mode: PdpRouteMode;
  primary_pdp_id: string;
  fallback_pdp_ids: string[];
  shadow_pdp_ids: string[];
  required_pdp_ids?: string[];
  merge_strategy: string;
  failure_behavior: PdpFailureBehavior;
  timeout_ms: int32;
  max_retries: int32;
  circuit_breaker_threshold?: int32;
  cooldown_secs?: int32;
  last_known_good_ttl_secs?: int32;
}

model PdpRuntimeListResponse {
  items: PdpRuntime[];
}

model PdpRouteRuleListResponse {
  items: PdpRouteRule[];
}

model SimulationRequest {
  action: string;
  resource: string;
  principal: string;
  context: unknown;
  target_pep?: string;
}

model SimulationResult {
  passed?: boolean;
  decision?: string;
  logs?: string[];
}

@tag("PDPConfig")
@route("/v1/tenants/{tenant_id}/pdp/runtimes")
interface PdpRuntimeApi {
  @doc("List all PDP runtimes")
  @get listRuntimes(@path tenant_id: string, ...PollenHeaders): PdpRuntimeListResponse | PollenError;

  @doc("Get a specific PDP runtime")
  @get getRuntime(@path tenant_id: string, @path runtime_id: string, ...PollenHeaders): PdpRuntime | PollenError;

  @doc("Create or update a PDP runtime")
  @put upsertRuntime(@path tenant_id: string, @path runtime_id: string, ...PollenHeaders, @body body: PdpRuntime): PdpRuntime | PollenError;

  @doc("Delete a PDP runtime")
  @delete deleteRuntime(@path tenant_id: string, @path runtime_id: string, ...PollenHeaders): void | PollenError;

  @doc("Probe a PDP runtime for health and capability")
  @route("{runtime_id}/probe")
  @post probeRuntime(@path tenant_id: string, @path runtime_id: string, ...PollenHeaders, @body input?: unknown): PdpProbeResult | PollenError;

  @doc("Validate a PDP runtime configuration")
  @route("{runtime_id}/validate")
  @post validateRuntime(@path tenant_id: string, @path runtime_id: string, ...PollenHeaders): unknown | PollenError;

  @doc("Clear cache for a PDP runtime")
  @route("{runtime_id}/clear-cache")
  @post clearCache(@path tenant_id: string, @path runtime_id: string, ...PollenHeaders): void | PollenError;
}

@tag("PDPConfig")
@route("/v1/tenants/{tenant_id}/pdp/routes")
interface PdpRoutingApi {
  @doc("List all PDP route rules")
  @get listRoutes(@path tenant_id: string, ...PollenHeaders): PdpRouteRuleListResponse | PollenError;

  @doc("Get a specific PDP route rule")
  @get getRoute(@path tenant_id: string, @path route_id: string, ...PollenHeaders): PdpRouteRule | PollenError;

  @doc("Create or update a PDP route rule")
  @put upsertRoute(@path tenant_id: string, @path route_id: string, ...PollenHeaders, @body body: PdpRouteRule): PdpRouteRule | PollenError;

  @doc("Delete a PDP route rule")
  @delete deleteRoute(@path tenant_id: string, @path route_id: string, ...PollenHeaders): void | PollenError;

  @doc("Simulate a routing decision")
  @route("simulate")
  @post simulateRoute(@path tenant_id: string, ...PollenHeaders, @body body: SimulationRequest): SimulationResult | PollenError;
}
