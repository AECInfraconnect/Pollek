import type { Page, Route } from "@playwright/test";

const externalServer = process.env.DEK_PLAYWRIGHT_EXTERNAL_SERVER === "1";
const now = "2026-06-27T10:15:00Z";

const json = (route: Route, body: unknown, status = 200) =>
  route.fulfill({
    status,
    contentType: "application/json",
    body: JSON.stringify(body),
  });

const objectMeta = (source = "discovery", status = "registered") => ({
  schema_version: "pollek.registry.meta.v1",
  tenant_id: "local",
  workspace_id: "local",
  environment_id: "desktop",
  created_at: "2026-06-27T10:00:00Z",
  updated_at: now,
  created_by: "playwright-fixture",
  updated_by: "playwright-fixture",
  source,
  status,
  tags: ["e2e", "governance-loop"],
});

const agent = {
  meta: objectMeta(),
  agent_id: "agent-antigravity",
  name: "Antigravity",
  agent_type: "openai_agent",
  vendor: "Google",
  runtime: {
    runtime_name: "native",
    version: "1.0.0",
  },
  entrypoints: [
    {
      command: "antigravity.exe",
      args: [],
    },
  ],
  declared_tools: ["tool-workspace-files"],
  declared_resources: ["resource-workspace-src"],
  identity: {
    spiffe_id: "spiffe://local.pollek/device/dev-win/agent/antigravity",
    process_path: "C:\\Program Files\\Google\\Antigravity\\antigravity.exe",
    user_subject: "DELL\\LocalAdmin",
    token_bindings: [
      {
        kind: "oidc_id_token",
        provider: "Pollek Cloud",
        issuer: "https://cloud.pollek.ai",
        subject: "agent-antigravity",
        audience: ["pollek-cloud"],
        scopes: ["telemetry.write"],
        confirmation: "spiffe_svid",
        expires_at: "2026-06-27T12:00:00Z",
        last_rotated_at: "2026-06-27T10:00:00Z",
      },
    ],
  },
  trust_level: "medium",
  capabilities: [
    "workspace_file_access",
    "terminal_execution",
    "browser_control",
    "tool_calling",
    "mcp_client",
  ],
  labels: {
    reference_intel: "google-antigravity",
    source: "discovery",
  },
  enforcement_mode: "Enforce",
};

const tool = {
  tool_id: "tool-workspace-files",
  name: "Workspace Files",
  type: "filesystem",
  vendor: "Pollek",
  description: "Observed file and folder access from the local agent process.",
  agent_id: agent.agent_id,
  source: "local-observer",
  status: "active",
  call_count: 3,
  last_used: now,
};

const resource = {
  resource_id: "resource-workspace-src",
  name: "repo/src",
  type: "folder",
  uri: "file:///C:/Users/DELL/Documents/Codex/repo/apps/local-admin-dashboard/src",
  path: "C:\\Users\\DELL\\Documents\\Codex\\repo\\apps\\local-admin-dashboard\\src",
  host: "DELL-WINDOWS",
  description:
    "Source folder observed through the local filesystem telemetry plane.",
  sensitivity: "internal_source",
  source: "local-observer",
  status: "active",
  last_accessed: now,
};

const policy = {
  policy_id: "policy-protect-workspace-files",
  name: "Protect workspace source files",
  description:
    "Require local policy evaluation before Antigravity reads source folders.",
  engine: "opa_wasm",
  status: "published",
  mode: "enforce",
  scope: agent.agent_id,
  created_at: "2026-06-27T10:05:00Z",
  updated_at: now,
  rules_count: 2,
  source: "policy-suggestions",
  last_deployed_at: now,
  bundle_id: "bundle-local-1",
};

const policyFeasibility = {
  policy_id: policy.policy_id,
  requested_level: "enforce",
  achievable_level: "enforce",
  verdict: "fully_enforceable",
  per_domain: [
    {
      domain: "filesystem",
      chosen_method: "windows_process_observer",
      level: "enforce",
      reason_en: "Local process observer is active.",
      reason_th: "",
    },
  ],
  gaps: [],
  friendly_en: "This policy can be fully enforced on this device.",
  friendly_th: "",
};

const cloudPdpProfile = {
  status: "disconnected",
  manual_override_enabled: false,
  health: {
    state: "not_configured",
    detail: "Pollek Cloud is not connected in the local mock dashboard.",
  },
};

const candidate = {
  schema_version: "discovery.candidate.v2",
  candidate_id: agent.agent_id,
  tenant_id: "local",
  device_id: "DELL-WINDOWS",
  status: "registered",
  canonical_service_id: "google.antigravity",
  surface_group_id: "google.ai",
  authority_boundary: "local_device",
  entity_role: "local_agent_host",
  duplicate_policy: "standalone",
  observe_scope:
    "Local process, filesystem metadata, MCP/tool activity, and linked browser surfaces.",
  enforce_scope:
    "Use local wrappers, MCP control bindings, and OS observers where available.",
  related_surfaces: [
    {
      service_id: "google.ai_studio",
      display_name: "Google AI Studio (Chrome)",
      entity_role: "web_ai_surface",
      authority_boundary: "local_browser_profile",
      evidence_sources: ["browser_window"],
      confidence: 0.86,
      control_parent_id: agent.agent_id,
      grouping_reason:
        "Google AI Studio is a related web surface, not a duplicate Antigravity agent.",
    },
  ],
  instance_count: 1,
  matched_signature_id: "google_antigravity",
  display_name: agent.name,
  vendor: agent.vendor,
  product: "Antigravity",
  inferred_agent_type: "openai_agent",
  confidence: 0.96,
  risk_score: 42,
  first_seen: "2026-06-27T10:00:00Z",
  last_seen: now,
  scan_ids: ["scan-e2e-1"],
  last_scan_id: "scan-e2e-1",
  evidence: [
    {
      evidence_id: "ev-proc-antigravity",
      source: "process",
      confidence: 0.98,
      observed_at: now,
      privacy_class: "metadata_only",
      redacted: true,
      merge_key: "Google/Antigravity",
      source_path_redacted:
        "C:\\Program Files\\Google\\Antigravity\\antigravity.exe",
      data: {
        process_name: "antigravity.exe",
        window_title: "Antigravity - Pollek Workspace",
      },
    },
    {
      evidence_id: "ev-file-src",
      source: "filesystem",
      confidence: 0.9,
      observed_at: now,
      privacy_class: "metadata_only",
      redacted: true,
      data: {
        path: resource.path,
        access: "read",
      },
    },
  ],
  matched_signals: [
    { kind: "process_name", detail: "antigravity.exe", weight: 0.45 },
    { kind: "well_known_vendor", detail: "Google Antigravity", weight: 0.35 },
  ],
  capability_tags: agent.capabilities,
  discovered_configs: [],
  discovered_endpoints: [],
  discovered_mcp_servers: [
    {
      server_name: "workspace-files",
      transport: "stdio",
    },
  ],
  suggested_registration: {
    agent_id: agent.agent_id,
    display_name: agent.name,
  },
  suggested_observation_profile: {
    exact_usage_first: true,
    sources: ["process", "filesystem", "usage_logs"],
  },
  observation_coverage: [
    {
      signal: "process_metadata",
      label: "Process activity",
      status: "active",
      method: "process_scan + ebpf_exec",
    },
    {
      signal: "network_metadata",
      label: "Network egress",
      status: "active",
      method: "ebpf_egress + sni_inspection",
    },
    {
      signal: "mcp_tool_metadata",
      label: "MCP tool calls",
      status: "active",
      method: "mcp_tool_call_metadata",
    },
    {
      signal: "token_usage",
      label: "Token & cost usage",
      status: "active",
      method: "local_session_log + egress_llm_usage_parser",
    },
    {
      signal: "file_metadata",
      label: "File access",
      status: "active",
      method: "ebpf_file_access",
    },
  ],
  suggested_control_bindings: [
    {
      binding_id: "binding-workspace-files",
      kind: "tool",
      target_candidate_id: agent.agent_id,
      action: "enforce",
      requires_user_approval: false,
      risk: "medium",
      reversible: true,
      summary: "Apply filesystem policy before source folder access.",
    },
  ],
  telemetry_plan: {
    exact_usage_sources: ["wrapper", "provider_usage_logs"],
    fallback_sources: ["process_metadata"],
  },
  labels: {
    reference_intel: "google-antigravity",
    canonical_service_id: "google.antigravity",
    surface_group_id: "google.ai",
    entity_role: "local_agent_host",
    duplicate_policy: "standalone",
  },
};

const googleAiStudioCandidate = {
  ...candidate,
  candidate_id: "candidate-google-ai-studio-chrome",
  status: "pending_approval",
  canonical_service_id: "google.ai_studio",
  surface_group_id: "google.ai",
  authority_boundary: "local_browser_profile",
  entity_role: "web_ai_surface",
  duplicate_policy: "child_surface",
  control_parent_id: agent.agent_id,
  grouping_reason:
    "Google AI Studio is related to Google AI surfaces but remains its own browser-scoped surface.",
  observe_scope:
    "Browser tab/window metadata and extension telemetry when installed.",
  enforce_scope:
    "Ask the user to configure the browser extension or the parent local agent; do not treat this as a standalone OS process.",
  related_surfaces: [],
  matched_signature_id: "google_ai_studio",
  display_name: "Google AI Studio (Chrome)",
  vendor: "Google",
  product: "AI Studio",
  inferred_agent_type: "web_a_i_app",
  confidence: 0.86,
  risk_score: 60,
  evidence: [
    {
      evidence_id: "ev-web-google-ai-studio",
      source: "browser_window",
      confidence: 0.86,
      observed_at: now,
      privacy_class: "metadata_only",
      redacted: true,
      merge_key: "browser:Chrome:google_ai_studio",
      data: {
        browser_name: "Chrome",
        url_host: "aistudio.google.com",
        matched_signature_id: "google_ai_studio",
        canonical_service_id: "google.ai_studio",
        evidence_strength: "browser_surface",
        duplicate_policy: "child_surface",
        scan_id: "scan-e2e-1",
      },
    },
  ],
  matched_signals: [
    { kind: "domain", detail: "aistudio.google.com", weight: 0.5 },
    { kind: "browser_title", detail: "Google AI Studio", weight: 0.3 },
  ],
  capability_tags: ["llm.chat", "web.chat", "net.egress.llm"],
  discovered_mcp_servers: [],
  suggested_registration: {
    agent_id: "candidate-google-ai-studio-chrome",
    display_name: "Google AI Studio (Chrome)",
  },
  suggested_control_bindings: [],
  telemetry_plan: {
    exact_usage_sources: ["browser_extension"],
    fallback_sources: ["browser_window_metadata"],
  },
  labels: {
    reference_intel: "google-ai-studio",
    canonical_service_id: "google.ai_studio",
    surface_group_id: "google.ai",
    entity_role: "web_ai_surface",
    duplicate_policy: "child_surface",
  },
};

const enrichmentSession = (status = "waiting_for_consent") => ({
  schema_version: "pollek.discovery.enrichment_session.v1",
  session_id: "enrich-google-ai-studio",
  tenant_id: "local",
  candidate_id: googleAiStudioCandidate.candidate_id,
  status,
  created_at: now,
  consent_required: status === "waiting_for_consent",
  privacy_guardrails: [
    "Uses local metadata only by default",
    "Does not read prompts or responses",
    "No package install or code execution",
  ],
  local_evidence_summary: {
    display_name: googleAiStudioCandidate.display_name,
    evidence_count: googleAiStudioCandidate.evidence.length,
    authority_boundary: googleAiStudioCandidate.authority_boundary,
  },
  source_plan: [
    {
      source_id: "local_evidence",
      label: "Local evidence",
      allowed: true,
      network_access: "none",
      safety: "local_metadata_only",
    },
    {
      source_id: "official_docs",
      label: "Official product documentation",
      allowed: true,
      network_access: "requires_user_approval",
      safety: "public_metadata_only",
    },
  ],
  extracted_facts: [
    {
      fact: "canonical_service_id",
      value: googleAiStudioCandidate.canonical_service_id,
      confidence: 0.92,
      source: "local_evidence",
    },
  ],
  definition_candidate: {
    canonical_service_id: googleAiStudioCandidate.canonical_service_id,
    surface_group_id: googleAiStudioCandidate.surface_group_id,
    duplicate_policy: googleAiStudioCandidate.duplicate_policy,
  },
  accepted_sources:
    status === "waiting_for_consent"
      ? undefined
      : ["local_evidence", "official_docs"],
  research_result:
    status === "waiting_for_consent"
      ? undefined
      : {
          summary:
            "Google AI Studio should stay as a browser-scoped child surface, not a duplicate Antigravity agent.",
        },
  learned_profile_id:
    status === "submitted" ? "learned-google-ai-studio" : undefined,
});

const canonicalCapability = {
  capability_id: "cap-workspace-file-access",
  candidate_id: agent.agent_id,
  capability_kind: "tool_access",
  name: "Workspace file access",
  description:
    "Local discovery observed filesystem metadata for source folder reads.",
  modality: ["filesystem"],
  actions: ["read", "list"],
  source: "filesystem observer",
  confidence: 0.92,
  risk_tags: ["source_code", "local_file"],
  evidence_ids: ["ev-file-src"],
  privacy_class: "metadata_only",
};

const capabilityInventory = {
  schema_version: "discovery.capability-inventory.v1",
  candidate_id: agent.agent_id,
  entity: {
    schema_version: "discovery.entity.v1",
    candidate_id: agent.agent_id,
    tenant_id: "local",
    device_id: "DELL-WINDOWS",
    entity_kind: "agent",
    display_name: agent.name,
    vendor: agent.vendor,
    product: "Antigravity",
    confidence: 0.96,
    risk_score: 42,
    status: "registered",
    capabilities: [canonicalCapability],
    evidence: candidate.evidence,
    relationships: [
      {
        relationship_id: "rel-agent-tool",
        subject_candidate_id: agent.agent_id,
        relation: "uses_tool",
        object_candidate_id: tool.tool_id,
        confidence: 0.9,
        evidence_ids: ["ev-file-src"],
      },
    ],
    suggested_registration: candidate.suggested_registration,
    suggested_control_bindings: candidate.suggested_control_bindings,
    privacy_profile: "metadata_only",
    performance_cost_class: "passive_metadata",
    first_seen: candidate.first_seen,
    last_seen: candidate.last_seen,
  },
  capabilities: [canonicalCapability],
  relationships: [
    {
      relationship_id: "rel-agent-tool",
      subject_candidate_id: agent.agent_id,
      relation: "uses_tool",
      object_candidate_id: tool.tool_id,
      confidence: 0.9,
      evidence_ids: ["ev-file-src"],
    },
  ],
  retrieval_status: "derived",
  source: "local discovery fixture",
  privacy_note: "Metadata-only fixture. No file content is read.",
};

const graphNodes = [
  {
    id: "agent:agent-antigravity",
    type: "agent",
    entity_id: agent.agent_id,
    label: agent.name,
    subtitle: "Google native coding agent",
    status: "enforcing",
    risk: "medium",
    mode: "enforce",
    badges: ["Registered", "SPIFFE bound"],
    metrics: [
      { label: "Tools", value: "1" },
      { label: "Resources", value: "1" },
    ],
    href: `/agents?id=${agent.agent_id}`,
    raw: agent,
  },
  {
    id: "tool:tool-workspace-files",
    type: "tool",
    entity_id: tool.tool_id,
    label: tool.name,
    subtitle: tool.type,
    status: "active",
    risk: "medium",
    mode: "enforce",
    badges: ["Observed"],
    metrics: [{ label: "Calls", value: "3" }],
    href: `/tools?id=${tool.tool_id}`,
    raw: tool,
  },
  {
    id: "resource:resource-workspace-src",
    type: "resource",
    entity_id: resource.resource_id,
    label: resource.name,
    subtitle: resource.path,
    status: "active",
    risk: "medium",
    mode: "enforce",
    badges: ["Observed"],
    metrics: [{ label: "Sensitivity", value: resource.sensitivity }],
    href: `/tools?id=${resource.resource_id}`,
    raw: resource,
  },
  {
    id: "policy:policy-protect-workspace-files",
    type: "policy",
    entity_id: policy.policy_id,
    label: policy.name,
    subtitle: policy.engine,
    status: "enforcing",
    risk: "medium",
    mode: "enforce",
    badges: ["Published"],
    metrics: [{ label: "Rules", value: "2" }],
    href: `/policies?id=${policy.policy_id}`,
    raw: policy,
  },
];

const graphEdges = [
  {
    id: "edge-agent-tool",
    source: "agent:agent-antigravity",
    target: "tool:tool-workspace-files",
    relation: "uses_tool",
    label: "uses",
    evidence: "filesystem observer",
    observed: true,
    enforced: true,
  },
  {
    id: "edge-tool-resource",
    source: "tool:tool-workspace-files",
    target: "resource:resource-workspace-src",
    relation: "accesses_resource",
    label: "reads",
    evidence: "resource telemetry",
    observed: true,
    enforced: true,
  },
  {
    id: "edge-policy-agent",
    source: "policy:policy-protect-workspace-files",
    target: "agent:agent-antigravity",
    relation: "governs",
    label: "governs",
    evidence: "deployed policy bundle",
    observed: true,
    enforced: true,
  },
];

const graphResponse = {
  schema_version: "entity-graph.v1",
  tenant_id: "local",
  generated_at: now,
  center: null,
  nodes: graphNodes,
  edges: graphEdges,
  summaries: [
    { kind: "agents", label: "Agents", count: 1, tone: "info" },
    {
      kind: "observed_links",
      label: "Observed Links",
      count: 3,
      tone: "success",
    },
    {
      kind: "enforced_links",
      label: "Enforced Links",
      count: 3,
      tone: "success",
    },
  ],
  warnings: [],
};

const activityItem = {
  event_id: "evt-governance-loop-1",
  timestamp: now,
  actor: {
    id: "agent:agent-antigravity",
    type: "agent",
    entity_id: agent.agent_id,
    label: agent.name,
  },
  action: "filesystem.read",
  tool: {
    id: "tool:tool-workspace-files",
    type: "tool",
    entity_id: tool.tool_id,
    label: tool.name,
  },
  resource: {
    id: "resource:resource-workspace-src",
    type: "resource",
    entity_id: resource.resource_id,
    label: resource.name,
  },
  policies: [
    {
      id: "policy:policy-protect-workspace-files",
      type: "policy",
      entity_id: policy.policy_id,
      label: policy.name,
    },
  ],
  decision: "allow",
  enforcement_mode: "enforce",
  pep_plane: "windows_user_mode_observer",
  pdp_engine: "opa_wasm",
  trace_id: "trace-governance-loop-1",
  cost: {
    total_cost_usd: 0.0012,
    total_tokens: 128,
    provider: "local-observer",
    model: "exact-usage-fixture",
  },
  explanation: "Policy allowed source folder read after local PDP evaluation.",
  raw: {
    evidence_id: "ev-file-src",
    capture_quality: "exact",
  },
};

const guardIncident = {
  schema_version: "telemetry-envelope.v1",
  event_id: "guard-e2e-prompt-injection",
  event_type: "guard_incident",
  timestamp: now,
  tenant_id: "local",
  agent_id: agent.agent_id,
  redaction_applied: true,
  payload: {
    guard_event: {
      event_id: "guard-e2e-prompt-injection",
      ts: now,
      tenant_id: "local",
      agent_id: agent.agent_id,
      direction: "request",
      action: "redact",
      categories: ["llm01_prompt_injection"],
      injection_score: 0.93,
      findings_summary: [{ kind: "api_key", count: 1 }],
      severity: "warn",
      remediation: {
        user_message: "Redacted prompt safety event.",
        recommended_actions: [],
        doc_url: null,
        can_override: false,
      },
      redaction_applied: true,
      source: "content_guard_local_engine",
      analysis_pipeline: {
        mode: "local_only",
        steps: ["deterministic_prompt_guard_rules"],
        enterprise_cloud_ner_supported: true,
        enterprise_cloud_ner_enabled: false,
        third_party_provider: null,
      },
    },
    findings: [{ kind: "api_key", count: 1 }],
    redaction: { applied: true },
  },
};

const guardActivityItem = {
  schema_version: "activity-timeline.v1",
  event_id: guardIncident.event_id,
  timestamp: now,
  actor: {
    id: `agent:${agent.agent_id}`,
    type: "agent",
    entity_id: agent.agent_id,
    label: agent.name,
  },
  action: "prompt_guard_redact",
  tool: null,
  resource: {
    id: "resource:prompt-guard:llm01_prompt_injection",
    type: "resource",
    entity_id: "prompt-guard:llm01_prompt_injection",
    label: "Prompt injection attempt",
  },
  policies: [],
  decision: "redact",
  enforcement_mode: "guarded_path",
  pep_plane: "prompt_guard",
  pdp_engine: null,
  trace_id: "trace-guard-e2e",
  cost: null,
  explanation: "Prompt injection attempt - redacted",
  raw: guardIncident,
};

const activityTimeline = {
  schema_version: "activity-timeline.v1",
  tenant_id: "local",
  generated_at: now,
  items: [activityItem, guardActivityItem],
  next_cursor: null,
};

const userFriendlyActivity = {
  schema_version: "user-friendly-activity-list.v1",
  tenant_id: "local",
  generated_at: now,
  source: "mock-api",
  items: [
    {
      schema_version: "user-friendly-activity.v1",
      event_id: activityItem.event_id,
      timestamp: activityItem.timestamp,
      agent_id: agent.agent_id,
      agent_name: agent.name,
      category: "files",
      action: "read",
      target_label: resource.name,
      target_kind: "Files & folders",
      access_mode: "read",
      result: "allowed",
      result_label: "Allowed",
      plain_summary: `${agent.name} read ${resource.name}`,
      rule_label: policy.name,
      capability_note: "Pollek saw this action and it was allowed.",
      next_step:
        "Set a rule for this folder, or restrict file access inside the AI app settings.",
      privacy_note:
        "Pollek shows activity metadata here, not file contents, email bodies, raw prompts, or raw responses.",
      cost_usd: activityItem.cost.total_cost_usd,
      tokens: activityItem.cost.total_tokens,
      trace_id: activityItem.trace_id,
      advanced: {
        decision: activityItem.decision,
        mode: activityItem.enforcement_mode,
        pep_plane: activityItem.pep_plane,
        pdp_engine: activityItem.pdp_engine,
      },
    },
    {
      schema_version: "user-friendly-activity.v1",
      event_id: guardActivityItem.event_id,
      timestamp: guardActivityItem.timestamp,
      agent_id: agent.agent_id,
      agent_name: agent.name,
      category: "safety",
      action: "redact",
      target_label: "Prompt injection attempt",
      target_kind: "Prompt & data safety",
      access_mode: "unknown",
      result: "redacted",
      result_label: "Redacted",
      plain_summary: `${agent.name} protected Prompt injection attempt`,
      rule_label: undefined,
      capability_note:
        "Pollek removed or masked sensitive content before it could continue.",
      next_step:
        "Review the safety rule and confirm the AI app is using the guard path for prompts and outputs.",
      privacy_note:
        "Pollek shows activity metadata here, not file contents, email bodies, raw prompts, or raw responses.",
      trace_id: guardActivityItem.trace_id,
      advanced: {
        decision: guardActivityItem.decision,
        mode: guardActivityItem.enforcement_mode,
        pep_plane: guardActivityItem.pep_plane,
      },
    },
  ],
  next_cursor: null,
};

const activitySummary = {
  activity_sets: [
    {
      id: "governance-loop",
      label: "Governance loop",
      items: [activityItem],
    },
  ],
};

const capabilitySnapshot = {
  schema_version: "local-capability-snapshot.v2",
  tenant_id: "local",
  device_id: "DELL-WINDOWS",
  os: {
    family: "windows",
    version: "11 24H2",
    arch: "x86_64",
    is_server: false,
    elevated: true,
  },
  mode: "desktop_advanced",
  generated_at: now,
  control_methods: [
    {
      method_id: "windows_process_observer",
      display_name_en: "Windows Process Observer",
      display_name_th: "Windows Process Observer",
      status: "available",
      domains: ["process", "filesystem"],
      max_level: "enforce",
      maturity: "beta",
      install_state: "installed",
      warm_check: "passed",
      setup_action_ids: [],
      limitations_en: [
        "User-mode fixture for E2E. Kernel WFP driver is not required.",
      ],
      limitations_th: [],
    },
    {
      method_id: "browser_extension",
      display_name_en: "Browser Extension",
      display_name_th: "Browser Extension",
      status: "needs_install",
      domains: ["browser"],
      max_level: "observe",
      maturity: "alpha",
      install_state: "not_installed",
      warm_check: "not_run",
      setup_action_ids: ["install-browser-extension"],
      limitations_en: [
        "Browser message body capture requires user installation.",
      ],
      limitations_th: [],
    },
  ],
  observation_sources: [
    {
      source_id: "process",
      display_name_en: "Process Table",
      display_name_th: "Process Table",
      status: "available",
      domains: ["process"],
      privacy_note_en: "Reads process metadata only.",
      privacy_note_th: "",
      setup_action_ids: [],
    },
    {
      source_id: "filesystem",
      display_name_en: "Filesystem Metadata",
      display_name_th: "Filesystem Metadata",
      status: "available",
      domains: ["filesystem"],
      privacy_note_en: "Captures file and folder paths, not contents.",
      privacy_note_th: "",
      setup_action_ids: [],
    },
  ],
  setup_actions: [
    {
      action_id: "install-browser-extension",
      title_en: "Install browser extension",
      title_th: "Install browser extension",
      detail_en: "Required for exact browser AI prompt and response telemetry.",
      detail_th: "",
      estimated_minutes: 3,
      requires_admin: false,
      requires_restart: false,
      safe_to_skip: true,
    },
  ],
  contract: {
    local_contract_version: "pollek.local.v1",
    compatible_cloud_contracts: ["pollek.cloud.v1"],
    status: "compatible",
    reason_code: null,
  },
};

const detectionSensors = [
  {
    id: "mcp_proxy",
    title: "MCP proxy and tool wrapper",
    os: ["windows", "macos", "linux"],
    domains: ["tools", "files", "commands"],
    layer: "application",
    status: "ready",
    achieved_level: "enforce",
    achievable_level: "enforce",
    deterministic_decision:
      "This source can contribute enforce decisions. Other evidence sources still cross-check activity so policy does not depend on this source alone.",
    evidence_sources: ["application", "domain:tools", "domain:files"],
    missing_requirements: [],
    remediation: [],
    can_observe: true,
    can_enforce: true,
    requires_admin: false,
    user_consent_required: true,
    setup_action: "Route AI tools through the Pollek MCP proxy or wrapper.",
    reason:
      "MCP traffic is plaintext at the tool boundary, so Pollek can observe and block before the tool runs.",
    fallback:
      "If the agent cannot use MCP, keep OS/process observation and configure the AI app's own permissions.",
    package_path: null,
    setup_state: null,
  },
  {
    id: "browser_ai_extension",
    title: "Browser AI connector",
    os: ["windows", "macos", "linux"],
    domains: ["web", "prompts", "uploads", "safety"],
    layer: "browser",
    status: "package_available_user_install_required",
    achieved_level: "none",
    achievable_level: "observe_only",
    deterministic_decision:
      "This source contributes deterministic observe evidence after user approval. If it fails, Pollek keeps deciding from the remaining evidence matrix and lowers confidence/control level.",
    evidence_sources: ["browser", "domain:web", "domain:prompts"],
    missing_requirements: [
      {
        code: "browser.extension_installed",
        description: "Pollek browser extension / native messaging host installed",
      },
    ],
    remediation: [
      {
        action: "Install the Pollek browser extension",
        requires_admin: false,
      },
    ],
    can_observe: true,
    can_enforce: true,
    requires_admin: false,
    user_consent_required: true,
    setup_action:
      "Build or install the browser connector, then approve it in Chrome, Edge, or Safari.",
    reason:
      "Browsers do not permit silent local extension install. User approval or enterprise browser policy is required.",
    fallback:
      "Without the extension, Pollek can still observe browser windows, domains, and process metadata.",
    package_path: "apps/prompt-guard-browser-extension",
    setup_state: null,
  },
  {
    id: "windows_wfp_driver",
    title: "Windows WFP network driver",
    os: ["windows"],
    domains: ["network", "dns", "egress"],
    layer: "kernel",
    status: "signed_driver_required",
    achieved_level: "none",
    achievable_level: "observe_only",
    deterministic_decision:
      "This source is unavailable on this host. Pollek excludes it from the current decision matrix and uses MCP, gateway, browser, process, local log, or registry evidence when available.",
    evidence_sources: ["kernel", "domain:network", "unavailable-excluded"],
    missing_requirements: [
      {
        code: "windows.driver_signed",
        description: "A Microsoft-attested signed kernel driver to enforce",
      },
    ],
    remediation: [
      {
        action: "Install the signed Pollek driver build",
        requires_admin: true,
      },
    ],
    can_observe: false,
    can_enforce: false,
    requires_admin: true,
    user_consent_required: true,
    setup_action:
      "Install the signed Pollek WFP service/driver and approve the Windows administrator prompt.",
    reason:
      "Windows network blocking requires a running WFP callout/service plus OS approval.",
    fallback:
      "Use MCP/HTTP gateway enforcement or observe-only network metadata until WFP is active.",
    package_path: "crates/dek-windows-wfp/driver",
    setup_state: null,
  },
];

const detectionRules = [
  {
    id: "POLLEK-DET-0001",
    name: "AI agent read secret file",
    severity: "high",
    confidence: "high",
    maturity: "beta",
    detect_type: "signature",
    default_response: "ask",
    enforce_if_capable: "block",
    observe_only_fallback: true,
    user_message:
      "An AI agent read a likely secret file such as .env, SSH keys, AWS credentials, or password databases.",
    maps: {
      owasp_llm: ["LLM02"],
      owasp_agentic: ["Sensitive information disclosure"],
      attack: ["T1552", "T1552.001"],
      atlas: [],
      nist_rmf: ["MEASURE", "MANAGE"],
    },
    setup_requirements: [
      "File/process visibility needs local OS metadata, MCP wrapper, SDK wrapper, or structured agent logs.",
    ],
    can_stop_next_time: true,
    privacy_note:
      "Detection uses redacted metadata and rule IDs. It does not store raw prompt, response, email body, or file content.",
  },
  {
    id: "POLLEK-DET-0003",
    name: "Untrusted web content led to an action",
    severity: "critical",
    confidence: "medium",
    maturity: "beta",
    detect_type: "sequence",
    default_response: "warn",
    enforce_if_capable: "block",
    observe_only_fallback: true,
    user_message:
      "An agent consumed untrusted web or email content and soon after executed a command, tool, or other action.",
    maps: {
      owasp_llm: ["LLM01"],
      owasp_agentic: ["Prompt injection"],
      attack: ["T1059"],
      atlas: [],
      nist_rmf: ["GOVERN", "MEASURE"],
    },
    setup_requirements: [
      "Browser or network visibility needs browser connector, HTTP/MCP proxy, WFP, Network Extension, or eBPF.",
      "Command execution visibility needs terminal wrapper, MCP tool proxy, process audit, or agent SDK hook.",
    ],
    can_stop_next_time: true,
    privacy_note:
      "Detection uses redacted metadata and rule IDs. It does not store raw prompt, response, email body, or file content.",
  },
];

const detectionCoverage = {
  schema_version: "pollek.detection.coverage.v1",
  tenant_id: "local",
  generated_at: now,
  pack_id: "pollek-core",
  pack_version: "2026.06.29",
  manifest_integrity: "verified",
  rule_count: detectionRules.length,
  coverage: {
    schema_version: "pollek.detection.coverage.v1",
    rule_count: detectionRules.length,
    frameworks: {
      owasp_llm: { LLM01: ["POLLEK-DET-0003"], LLM02: ["POLLEK-DET-0001"] },
      owasp_agentic: {
        "Prompt injection": ["POLLEK-DET-0003"],
        "Sensitive information disclosure": ["POLLEK-DET-0001"],
      },
      attack: {
        T1059: ["POLLEK-DET-0003"],
        T1552: ["POLLEK-DET-0001"],
      },
    },
  },
  rules: detectionRules,
  sensors: detectionSensors,
  research_basis: [
    {
      framework: "OWASP Top 10 for LLM Applications",
      source: "https://genai.owasp.org/llm-top-10/",
      implementation_use:
        "Rule mappings for prompt injection, sensitive disclosure, supply chain, excessive agency, and unbounded consumption.",
    },
    {
      framework: "NIST AI RMF / Generative AI Profile",
      source: "https://doi.org/10.6028/NIST.AI.600-1",
      implementation_use:
        "Risk mapping, measurement, governance traceability, and user disclosure for AI activity monitoring.",
    },
  ],
  privacy_guards: [
    "No raw prompt, response, email body, or file content is stored by detection rules.",
    "Rules operate on redacted metadata, classifications, hashes, timestamps, and provenance tags.",
  ],
  limitations: [
    "Kernel-level enforcement depends on OS support, signed components, user or admin approval, and warm checks.",
    "Encrypted HTTPS metadata alone cannot reveal prompt or response bodies.",
  ],
};

const usageSummary = {
  schema_version: "ai-usage-summary.v1",
  tenant_id: "local",
  generated_at: now,
  currency: "USD",
  totals: {
    total_cost_usd: 0.0012,
    total_tokens: 128,
    input_tokens: 80,
    output_tokens: 48,
    cached_input_tokens: 0,
    reasoning_output_tokens: 0,
    tool_tokens: 18,
    multimodal_tokens: 0,
    calls: 1,
  },
  by_agent: [
    {
      agent_id: agent.agent_id,
      total_cost_usd: 0.0012,
      total_tokens: 128,
      calls: 1,
      budget: { status: "ok" },
    },
  ],
  by_provider: [
    {
      provider: "local-observer",
      total_cost_usd: 0.0012,
      total_tokens: 128,
      calls: 1,
    },
  ],
  by_model: [
    {
      model: "exact-usage-fixture",
      total_cost_usd: 0.0012,
      total_tokens: 128,
      calls: 1,
    },
  ],
  buckets: [],
};

const usageEvents = {
  schema_version: "ai-usage-event-page.v1",
  tenant_id: "local",
  items: [
    {
      event_id: "usage-exact-1",
      occurred_at: now,
      agent_id: agent.agent_id,
      provider: "local-observer",
      model: "exact-usage-fixture",
      surface: "tool",
      tokens: {
        input_tokens: 80,
        output_tokens: 48,
        cached_input_tokens: 0,
        reasoning_output_tokens: 0,
        tool_tokens: 18,
        multimodal_tokens: 0,
        total_tokens: 128,
        estimated: false,
      },
      cost: {
        amount_usd: 0.0012,
        total_cost: 0.0012,
        currency: "USD",
        estimated: false,
      },
      cloud_sync_status: "pending",
      metadata: {
        capture_quality: "exact",
        source: "wrapper telemetry fixture",
      },
    },
  ],
  next_cursor: null,
};

const pluginMarketItem = {
  id: "browser-observe-connector",
  name: "Browser Observe Connector",
  version: "0.1.0",
  latest_version: "0.2.0",
  kind: "observe.collector",
  publisher: "Pollek",
  verified: true,
  rating: 4.8,
  installs: 1200,
  capabilities: ["browser:metadata:read", "telemetry:activity:write"],
  human_capabilities: [
    "Observe AI websites opened in the browser",
    "Write local activity metadata to Pollek",
  ],
  os: ["windows", "macos", "linux"],
  min_engine_version: "0.1.0",
  signature_ok: true,
  signature_state: "valid",
  update_available: true,
  rollback_supported: true,
  registry_ref: "local://plugins/browser-observe-connector/0.2.0",
  release_notes:
    "Adds tab lifecycle, prompt metadata, attachment metadata, and visible response metadata.",
  trust_labels: ["verified", "local_only"],
  lifecycle_state: "update_available",
  description_en:
    "Adds browser-tab observation for ChatGPT, Claude, DeepSeek, Manus, and similar AI websites without reading prompt text by default.",
  privacy_note:
    "Metadata-only by default. Prompt and response bodies require explicit browser permission later.",
  source: "local_catalog",
};

const installedPlugin = {
  schema_version: "pollek.installed_plugin.v1",
  id: "prompt-guard-local",
  name: "Prompt Guard Local",
  version: "0.1.0",
  latest_version: "0.1.0",
  kind: "resource.classifier",
  enabled: true,
  granted_caps: ["prompt_guard:check", "telemetry:guard_event:write"],
  human_grants: [
    "Check prompt and output safety locally",
    "Write guard incidents to local history",
  ],
  health: "healthy",
  source: "local_catalog",
  signature_state: "valid",
  update_available: false,
  rollback_available: true,
  previous_versions: ["0.0.9"],
  rollback_version: "0.0.9",
  revoked: false,
  rollout: "stable",
  canary_percent: 100,
  trust_labels: ["verified", "local_only"],
  lifecycle_state: "enabled",
  health_metrics: {
    heartbeat_status: "ok",
    error_rate: 0,
    latency_ms: 12,
  },
  privacy_note:
    "Runs local deterministic checks and stores findings, counts, and redaction status without raw prompt text.",
  installed_at: now,
  last_seen: now,
};

const policySuggestion = {
  suggestion_id: "suggest-protect-workspace-files",
  title: "Protect workspace source files",
  summary:
    "Antigravity was observed reading the source folder. Deploy an enforce policy for local file access.",
  severity: "medium",
  status: "ready",
  feasibility: "can_enforce_now",
  recommended_policy_type: "filesystem_access_guard",
  recommended_control_level: "enforce",
  confidence: 0.91,
  target_agent_id: agent.agent_id,
  created_at: now,
  setup_required: [],
};

function entity360(entityType: string | null, entityId: string | null) {
  const entity =
    graphNodes.find(
      (node) => node.type === entityType && node.entity_id === entityId,
    ) ?? graphNodes[0];
  return {
    schema_version: "entity-360.v1",
    tenant_id: "local",
    generated_at: now,
    entity,
    graph: {
      ...graphResponse,
      center: entity,
    },
    summaries: [
      { kind: "entity", label: entity.label, count: 1, tone: "info" },
      {
        kind: "observed_links",
        label: "Observed Links",
        count: 3,
        tone: "success",
      },
      {
        kind: "enforced_links",
        label: "Enforced Links",
        count: 3,
        tone: "success",
      },
    ],
    activity: [activityItem],
    warnings: [],
  };
}

function routeUrl(route: Route) {
  return new URL(route.request().url());
}

export async function installMockApi(page: Page) {
  if (externalServer) {
    return;
  }

  let scanStarted = false;
  let suggestionsGenerated = false;
  let enrichmentStatus = "waiting_for_consent";
  const policies = [policy];
  let installedPlugins = [installedPlugin];

  await page.route("**/.well-known/pollek-contract", (route) =>
    json(route, {
      schema_version: "contract-discovery.v1",
      preferred: "pollek.v1",
      supported: ["pollek.v1"],
      capabilities: ["local-admin-dashboard", "policy-publish"],
    }),
  );

  await page.route(
    "**/v1/tenants/local/devices/local/capability-snapshot-v2**",
    (route) => json(route, capabilitySnapshot),
  );
  await page.route(
    "**/v1/tenants/local/devices/local/capability-refresh**",
    (route) => json(route, capabilitySnapshot),
  );
  await page.route("**/v1/tenants/local/detections/coverage", (route) =>
    json(route, detectionCoverage),
  );
  await page.route("**/v1/tenants/local/detections/sensors", (route) =>
    json(route, {
      schema_version: "pollek.observe.sensors.v1",
      tenant_id: "local",
      generated_at: now,
      items: detectionSensors,
    }),
  );
  await page.route(
    "**/v1/tenants/local/detections/sensors/*/preflight",
    (route) =>
      json(route, {
        schema_version: "pollek.observe.sensor.preflight.v1",
        tenant_id: "local",
        checked_at: now,
      }),
  );
  await page.route(
    "**/v1/tenants/local/detections/sensors/*/consent",
    (route) => json(route, { status: "accepted" }),
  );
  await page.route(
    "**/v1/tenants/local/detections/sensors/*/install",
    (route) => json(route, { status: "waiting_for_user_or_os_approval" }),
  );

  await page.route("**/v1/tenants/local/registry/agents**", (route) =>
    json(route, { items: scanStarted ? [agent] : [] }),
  );
  await page.route("**/v1/tenants/local/registry/mcp-servers**", (route) =>
    json(route, { items: [] }),
  );
  await page.route("**/v1/tenants/local/registry/tools**", (route) =>
    json(route, { items: scanStarted ? [tool] : [] }),
  );
  await page.route("**/v1/tenants/local/registry/resources**", (route) =>
    json(route, { items: scanStarted ? [resource] : [] }),
  );
  await page.route("**/v1/tenants/local/registry/entities**", (route) =>
    json(route, { items: scanStarted ? [agent, tool, resource] : [] }),
  );
  await page.route("**/v1/tenants/local/registry/relationships**", (route) =>
    json(route, { items: scanStarted ? graphEdges : [] }),
  );

  await page.route("**/v1/tenants/local/discovery/candidates", (route) => {
    if (route.request().method() === "DELETE") {
      scanStarted = false;
      suggestionsGenerated = false;
      return json(route, { ok: true });
    }
    const candidates = scanStarted ? [candidate, googleAiStudioCandidate] : [];
    return json(route, {
      schema_version: "agent-discovery-candidate-list.v1",
      candidates,
      items: candidates,
      total: candidates.length,
    });
  });
  await page.route("**/v1/tenants/local/discovery/entities", (route) =>
    json(route, { items: scanStarted ? [capabilityInventory.entity] : [] }),
  );
  await page.route(
    "**/v1/tenants/local/discovery/candidates/*/capabilities",
    (route) => json(route, capabilityInventory),
  );
  await page.route(
    "**/v1/tenants/local/discovery/candidates/*/retrieve-capabilities",
    (route) => json(route, capabilityInventory),
  );
  await page.route(
    "**/v1/tenants/local/observations/agents/*/activity**",
    (route) =>
      json(route, {
        schema_version: "agent-observe-activity.v1",
        tenant_id: "local",
        agent_id: "agent_claude_desktop",
        matched_agent_ids: ["agent_claude_desktop"],
        generated_at: new Date().toISOString(),
        counts: {
          total_events: 2,
          by_kind: { resource_access: 1, tool_call: 1 },
          total_decisions: 0,
          denied_actions: 0,
          mcp_invocations: 1,
        },
        activity: [
          {
            timestamp: "2026-06-26T00:01:00Z",
            event_type: "resource_access",
            decision: null,
            resource: "~/projects/notes.txt",
            reason: "observe",
            pep_plane: "mcp_proxy",
            enforced_for_real: null,
            status_badge: null,
            message_th: null,
          },
          {
            timestamp: "2026-06-26T00:02:00Z",
            event_type: "mcp_tool_call",
            decision: "allow",
            resource: "filesystem",
            reason: "observe",
            pep_plane: "mcp_proxy",
            enforced_for_real: null,
            status_badge: null,
            message_th: null,
          },
        ],
        resources: [
          {
            target: "~/projects/notes.txt",
            resource_type: "file",
            verbs: ["read"],
            access_count: 1,
            total_bytes: 2048,
            first_seen: "2026-06-26T00:01:00Z",
            last_seen: "2026-06-26T00:01:00Z",
          },
        ],
        usage: {
          request_count: 2,
          input_tokens: 200,
          output_tokens: 50,
          cached_input_tokens: 20,
          reasoning_output_tokens: 10,
          total_tokens: 250,
          total_cost: 0.03,
          currency: "USD",
          exact_events: 2,
          estimated_events: 0,
          last_event_at: "2026-06-26T00:02:00Z",
          by_model: [
            {
              model: "fixture-model",
              request_count: 2,
              total_tokens: 250,
              total_cost: 0.03,
            },
          ],
        },
      }),
  );
  await page.route(
    "**/v1/tenants/local/discovery/candidates/*/enrichment/start",
    (route) => {
      enrichmentStatus = "waiting_for_consent";
      return json(route, enrichmentSession(enrichmentStatus));
    },
  );
  await page.route("**/v1/tenants/local/discovery/enrichment/*", (route) =>
    json(route, enrichmentSession(enrichmentStatus)),
  );
  await page.route(
    "**/v1/tenants/local/discovery/enrichment/*/approve",
    (route) => {
      enrichmentStatus = "researched";
      return json(route, enrichmentSession(enrichmentStatus));
    },
  );
  await page.route(
    "**/v1/tenants/local/discovery/enrichment/*/submit",
    (route) => {
      enrichmentStatus = "submitted";
      return json(route, enrichmentSession(enrichmentStatus));
    },
  );
  await page.route(
    "**/v1/tenants/local/discovery/candidates/*/register",
    (route) => {
      scanStarted = true;
      return json(route, agent);
    },
  );
  await page.route("**/v1/tenants/local/discovery/scans", (route) => {
    if (route.request().method() === "POST") {
      scanStarted = true;
      return json(route, {
        scan_id: "scan-e2e-1",
        tenant_id: "local",
        status: "completed",
        started_at: "2026-06-27T10:14:50Z",
        finished_at: now,
        sources: ["process", "filesystem", "mcp_config"],
        candidates_found: 2,
      });
    }
    return json(route, {
      items: scanStarted
        ? [
            {
              scan_id: "scan-e2e-1",
              tenant_id: "local",
              status: "completed",
              started_at: "2026-06-27T10:14:50Z",
              finished_at: now,
              sources: ["process", "filesystem", "mcp_config"],
              candidates_found: 2,
            },
          ]
        : [],
    });
  });
  await page.route("**/v1/tenants/local/discovery/scans/scan-e2e-1", (route) =>
    json(route, {
      scan_id: "scan-e2e-1",
      tenant_id: "local",
      status: "completed",
      started_at: "2026-06-27T10:14:50Z",
      finished_at: now,
      sources: ["process", "filesystem", "mcp_config"],
      candidates_found: 2,
    }),
  );

  await page.route("**/v1/tenants/local/entity-graph**", (route) => {
    const url = routeUrl(route);
    if (url.pathname.endsWith("/entity-graph/node")) {
      return json(
        route,
        entity360(
          url.searchParams.get("entity_type"),
          url.searchParams.get("entity_id"),
        ),
      );
    }
    return json(
      route,
      scanStarted ? graphResponse : { ...graphResponse, nodes: [], edges: [] },
    );
  });
  await page.route("**/v1/tenants/local/activity-timeline**", (route) =>
    json(
      route,
      scanStarted ? activityTimeline : { ...activityTimeline, items: [] },
    ),
  );
  await page.route(/\/v1\/tenants\/local\/activity(?:\?.*)?$/, (route) =>
    json(route, scanStarted ? activitySummary : { activity_sets: [] }),
  );
  await page.route("**/v1/tenants/local/user-friendly-activity**", (route) =>
    json(
      route,
      scanStarted
        ? userFriendlyActivity
        : { ...userFriendlyActivity, items: [] },
    ),
  );
  await page.route("**/v1/tenants/local/telemetry/guard-events", (route) =>
    json(route, {
      schema_version: "guard-events.v1",
      count: 1,
      items: [guardIncident],
    }),
  );
  await page.route(
    "**/v1/tenants/local/telemetry/guard-events/stream",
    (route) =>
      route.fulfill({
        status: 200,
        contentType: "text/event-stream",
        body: `data: ${JSON.stringify(guardIncident)}\n\n`,
      }),
  );
  await page.route("**/v1/tenants/local/prompt-guard/check", (route) => {
    const request = route.request().postDataJSON() as {
      direction?: string;
      source?: string;
      persist?: boolean;
    };
    const checkedEvent = {
      event_id: "guard-e2e-local-check",
      ts: now,
      tenant_id: "local",
      agent_id: "dashboard-local-check",
      direction: request.direction ?? "request",
      action: "redact",
      categories: ["llm01_prompt_injection"],
      injection_score: 0.7,
      findings_summary: [{ kind: "prompt_injection", count: 2 }],
      severity: "warn",
      remediation: {
        user_message:
          "Prompt Guard found a prompt safety signal and recommends redaction or review before this prompt continues.",
        recommended_actions: [
          "Route this AI app through the Prompt Guard browser extension, CLI hook, SDK wrapper, or MCP proxy before similar prompts continue.",
        ],
        doc_url: null,
        can_override: false,
      },
      redaction_applied: true,
      source: request.source ?? "dashboard_manual_check",
      raw_prompt_or_response_stored: false,
      matched_rules: ["instruction_override", "role_rebinding"],
      normalization_steps: [],
      capture: {
        source: request.source ?? "dashboard_manual_check",
        engine: "content_guard_local_engine",
        surface: "local_dashboard",
        text_length: 48,
        raw_text_persisted: false,
      },
      analysis_pipeline: {
        mode: "local_only",
        steps: ["deterministic_prompt_guard_rules"],
        enterprise_cloud_ner_supported: true,
        enterprise_cloud_ner_enabled: false,
        third_party_provider: null,
      },
    };
    return json(route, {
      schema_version: "pollek.prompt_guard.check.v1",
      event_id: checkedEvent.event_id,
      action: checkedEvent.action,
      severity: checkedEvent.severity,
      persisted: request.persist !== false,
      raw_prompt_or_response_stored: false,
      storage_error: null,
      guard_event: checkedEvent,
      recommended_actions: checkedEvent.remediation.recommended_actions,
      message: checkedEvent.remediation.user_message,
    });
  });
  await page.route("**/v1/tenants/local/activity", (route) =>
    json(route, scanStarted ? activitySummary : { activity_sets: [] }),
  );

  await page.route("**/v1/tenants/local/policy-suggestions", (route) => {
    // POST is the simple wizard asking for suggestions for picked agents;
    // GET is the policy-suggestions list view.
    if (route.request().method() === "POST") {
      return json(route, [
        {
          id: "pol_workspace_file_guard",
          title_en: "Protect workspace file access",
          title_th: "Protect workspace file access",
          domains: ["filesystem"],
          recommended_level: "enforce",
        },
      ]);
    }
    return json(route, { items: suggestionsGenerated ? [policySuggestion] : [] });
  });
  await page.route(
    "**/v1/tenants/local/policy-suggestions/generate",
    (route) => {
      suggestionsGenerated = true;
      return json(route, { items: [policySuggestion] });
    },
  );
  await page.route("**/v1/tenants/local/pdp/cloud**", (route) =>
    json(route, cloudPdpProfile),
  );
  await page.route("**/v1/tenants/local/policies", (route) => {
    const method = route.request().method();
    if (method === "GET") {
      return json(route, policies);
    }
    if (method === "POST") {
      const nextPolicy = route.request().postDataJSON();
      policies.push(nextPolicy);
      return json(route, nextPolicy, 201);
    }
    return json(route, { error: "unsupported method" }, 405);
  });
  await page.route("**/v1/tenants/local/policies/feasibility", (route) =>
    json(route, policyFeasibility),
  );
  await page.route("**/v1/tenants/local/deployment-sessions", (route) =>
    json(route, {
      id: "deploy-session-1",
      status: "ready",
      feasibility: {
        policy_id: policy.policy_id,
        requested_level: "enforce",
        achievable_level: "enforce",
        verdict: "fully_enforceable",
        per_domain: [],
        gaps: [],
        friendly_en: "Ready to deploy.",
        friendly_th: "",
      },
    }),
  );
  await page.route(
    "**/v1/tenants/local/deployment-sessions/deploy-session-1/confirm",
    (route) =>
      json(route, {
        policy_id: policy.policy_id,
        bindings: [
          {
            domain: "filesystem",
            method_id: "windows_process_observer",
            effective_level: "enforce",
            maturity: "beta",
          },
        ],
        fallbacks: [],
        auto_selected: true,
      }),
  );
  await page.route(
    "**/v1/tenants/local/deployment-sessions/deploy-session-1/apply",
    (route) => json(route, { applied: true, policy_id: policy.policy_id }),
  );
  await page.route(/\/v1\/tenants\/local\/policies\/[^/]+\/publish$/, (route) =>
    json(route, {
      published: true,
      bundle_id: "bundle-local-1",
      build_number: 1,
    }),
  );

  await page.route("**/v1/tenants/local/telemetry/decision-logs", (route) =>
    json(route, {
      count: 1,
      decisions: [
        {
          timestamp: now,
          event_id: "decision-e2e-1",
          payload: {
            decision: "allow",
            reason:
              "Policy allowed source folder read after local PDP evaluation.",
            request_id: "req-e2e-1",
            matched_policy_ids: [policy.policy_id],
            latency_ms: 7,
            resource: resource.path,
          },
        },
      ],
    }),
  );
  await page.route("**/v1/tenants/local/usage/summary**", (route) =>
    json(route, usageSummary),
  );
  await page.route("**/v1/tenants/local/usage/events**", (route) =>
    json(route, usageEvents),
  );
  await page.route(
    "**/v1/tenants/local/usage/credits**",
    (route) => {
      if (route.request().method() === "PUT") {
        const config = route.request().postDataJSON();
        return json(route, { config });
      }
      return json(route, {
        config: {
          schema_version: "pollek.credit_ledger.v1",
          currency: "USD",
          providers: [
            {
              provider: "local-observer",
              currency_per_credit: 0.001,
              initial_credits: 10000,
              label: "Local observer credits",
            },
          ],
        },
        status: {
          currency: "USD",
          providers: [
            {
              provider: "local-observer",
              label: "Local observer credits",
              currency_per_credit: 0.001,
              initial_credits: 10000,
              consumed_cost: 2.5,
              consumed_credits: 2500,
              remaining_credits: 7500,
            },
          ],
          total_consumed_credits: 2500,
          total_remaining_credits: 7500,
        },
      });
    },
  );
  await page.route("**/v1/tenants/local/marketplace/items", (route) =>
    json(route, {
      schema_version: "pollek.marketplace.v1",
      items: [pluginMarketItem],
    }),
  );
  await page.route("**/v1/tenants/local/plugins", (route) =>
    json(route, {
      schema_version: "pollek.installed_plugins.v1",
      items: installedPlugins,
    }),
  );
  await page.route("**/v1/tenants/local/plugins/install", (route) => {
    const body = route.request().postDataJSON() as {
      id: string;
      granted_caps?: string[];
    };
    const nextPlugin = {
      schema_version: "pollek.installed_plugin.v1",
      id: body.id,
      name: pluginMarketItem.name,
      version: pluginMarketItem.version,
      kind: pluginMarketItem.kind,
      enabled: true,
      granted_caps: body.granted_caps ?? pluginMarketItem.capabilities,
      human_grants: pluginMarketItem.human_capabilities,
      health: "healthy",
      source: pluginMarketItem.source,
      signature_state: pluginMarketItem.signature_state,
      privacy_note: pluginMarketItem.privacy_note,
      installed_at: now,
      last_seen: now,
    };
    installedPlugins = [
      ...installedPlugins.filter((plugin) => plugin.id !== nextPlugin.id),
      nextPlugin,
    ];
    return json(route, nextPlugin);
  });
  await page.route(/\/v1\/tenants\/local\/plugins\/[^/]+\/toggle$/, (route) => {
    const id = route.request().url().split("/plugins/")[1].split("/")[0];
    const body = route.request().postDataJSON() as { enabled?: boolean };
    installedPlugins = installedPlugins.map((plugin) =>
      plugin.id === id ? { ...plugin, enabled: Boolean(body.enabled) } : plugin,
    );
    return json(
      route,
      installedPlugins.find((plugin) => plugin.id === id) ?? installedPlugin,
    );
  });
  await page.route(/\/v1\/tenants\/local\/plugins\/[^/]+\/health$/, (route) => {
    const id = route.request().url().split("/plugins/")[1].split("/")[0];
    installedPlugins = installedPlugins.map((plugin) =>
      plugin.id === id
        ? {
            ...plugin,
            health: plugin.enabled ? "healthy" : "disabled",
            health_metrics: {
              heartbeat_status: "ok",
              error_rate: 0,
              latency_ms: 12,
            },
          }
        : plugin,
    );
    const plugin = installedPlugins.find((item) => item.id === id) ?? installedPlugin;
    return json(route, {
      schema_version: "pollek.plugin_lifecycle.v1",
      status: "ok",
      action: "plugin_health_checked",
      plugin,
      message: "Plugin health check recorded.",
    });
  });
  await page.route(/\/v1\/tenants\/local\/plugins\/[^/]+\/update$/, (route) => {
    const id = route.request().url().split("/plugins/")[1].split("/")[0];
    installedPlugins = installedPlugins.map((plugin) =>
      plugin.id === id
        ? {
            ...plugin,
            previous_versions: [plugin.version ?? "unknown"],
            rollback_version: plugin.version,
            version: plugin.latest_version ?? plugin.version,
            update_available: false,
            rollback_available: true,
            lifecycle_state: "enabled",
          }
        : plugin,
    );
    const plugin = installedPlugins.find((item) => item.id === id) ?? installedPlugin;
    return json(route, {
      schema_version: "pollek.plugin_lifecycle.v1",
      status: "ok",
      action: "plugin_updated",
      plugin,
      message: "Plugin updated.",
    });
  });
  await page.route(/\/v1\/tenants\/local\/plugins\/[^/]+\/rollback$/, (route) => {
    const id = route.request().url().split("/plugins/")[1].split("/")[0];
    installedPlugins = installedPlugins.map((plugin) =>
      plugin.id === id
        ? {
            ...plugin,
            version: plugin.rollback_version ?? plugin.version,
            update_available: true,
            rollback_available: false,
            lifecycle_state: "rollback_available",
          }
        : plugin,
    );
    const plugin = installedPlugins.find((item) => item.id === id) ?? installedPlugin;
    return json(route, {
      schema_version: "pollek.plugin_lifecycle.v1",
      status: "ok",
      action: "plugin_rolled_back",
      plugin,
      message: "Plugin rolled back.",
    });
  });
  await page.route(/\/v1\/tenants\/local\/plugins\/[^/]+\/canary$/, (route) => {
    const id = route.request().url().split("/plugins/")[1].split("/")[0];
    installedPlugins = installedPlugins.map((plugin) =>
      plugin.id === id
        ? { ...plugin, rollout: "canary", canary_percent: 10, lifecycle_state: "canary" }
        : plugin,
    );
    const plugin = installedPlugins.find((item) => item.id === id) ?? installedPlugin;
    return json(route, {
      schema_version: "pollek.plugin_lifecycle.v1",
      status: "ok",
      action: "plugin_canary_started",
      plugin,
      message: "Plugin canary started.",
    });
  });
  await page.route(/\/v1\/tenants\/local\/plugins\/[^/]+\/revoke$/, (route) => {
    const id = route.request().url().split("/plugins/")[1].split("/")[0];
    installedPlugins = installedPlugins.map((plugin) =>
      plugin.id === id
        ? {
            ...plugin,
            enabled: false,
            health: "revoked",
            revoked: true,
            granted_caps: [],
            lifecycle_state: "revoked",
          }
        : plugin,
    );
    const plugin = installedPlugins.find((item) => item.id === id) ?? installedPlugin;
    return json(route, {
      schema_version: "pollek.plugin_lifecycle.v1",
      status: "ok",
      action: "plugin_revoked",
      plugin,
      message: "Plugin revoked.",
    });
  });
  await page.route(/\/v1\/tenants\/local\/plugins\/(?!install$)[^/]+$/, (route) => {
    const id = route.request().url().split("/plugins/")[1];
    if (route.request().method() === "DELETE") {
      installedPlugins = installedPlugins.filter((plugin) => plugin.id !== id);
      return json(route, {
        status: "uninstalled",
        id,
        revoked_caps: true,
        cleared_plugin_namespace: true,
      });
    }
    return json(route, { error: "unsupported method" }, 405);
  });
  await page.route("**/v1/tenants/local/browser-extension/status", (route) =>
    json(route, {
      schema_version: "pollek.browser_extension.status.v1",
      tenant_id: "local",
      items: [
        {
          schema_version: "pollek.browser_extension.status_item.v1",
          extension_id: "pollek-prompt-guard-browser",
          extension_version: "0.1.0",
          browser_id: "chrome",
          browser_name: "Chrome",
          last_provider_id: "chatgpt-browser",
          last_provider_label: "ChatGPT",
          last_event_type: "prompt_submitted",
          last_seen: new Date().toISOString(),
          capture_mode: "observe",
          raw_prompt_or_response_stored: false,
          capabilities: [
            "tab_lifecycle_metadata",
            "prompt_submit_metadata",
            "attachment_metadata",
            "visible_response_metadata",
          ],
        },
      ],
      limitations: [
        "Browsers require user or enterprise approval before a local extension can run.",
        "Server-side AI tool calls require wrapper, proxy, SDK, MCP, or provider integrations.",
      ],
    }),
  );
  await page.route("**/v1/tenants/local/usage/stream", (route) =>
    route.fulfill({
      status: 200,
      contentType: "text/event-stream",
      body: "",
    }),
  );
  await page.route("**/v1/tenants/local/tools", (route) =>
    json(route, [tool]),
  );
  await page.route("**/v1/tenants/local/resources", (route) =>
    json(route, [resource]),
  );
  await page.route("**/v1/tenants/local/telemetry/tools", (route) =>
    json(route, {
      items: [
        {
          ...tool,
          observed_details: {
            use_count: 3,
            agents: [agent.agent_id],
            actions: ["read", "list"],
            last_used: now,
            governed: true,
          },
        },
      ],
    }),
  );
  await page.route("**/v1/tenants/local/telemetry/resources", (route) =>
    json(route, {
      items: [
        {
          ...resource,
          observed_details: {
            access_count: 2,
            agents: [agent.agent_id],
            modes: ["read"],
            last_accessed: now,
            governed: true,
          },
        },
      ],
    }),
  );
  await page.route("**/v1/tenants/local/telemetry/identities", (route) =>
    json(route, {
      items: [
        {
          entity_id: "identity-antigravity",
          identity_id: "identity-antigravity",
          display_name: "Google Antigravity workload",
          entity_type: "workload",
          external_ids: [{ provider: "spiffe", id: agent.identity.spiffe_id }],
          roles: ["local-ai-agent"],
          meta: objectMeta("telemetry", "active"),
          observed_details: {
            access_count: 1,
            agents: [agent.agent_id],
            actions: ["read"],
            spiffe_id: agent.identity.spiffe_id,
            last_seen: now,
          },
        },
      ],
    }),
  );
  await page.route(
    "**/v1/tenants/local/telemetry/identities/stream",
    (route) =>
      route.fulfill({
        status: 200,
        contentType: "text/event-stream",
        body: "",
      }),
  );
  await page.route("**/v1/tenants/local/local-observe/refresh", (route) => {
    scanStarted = true;
    return json(route, {
      schema_version: "local-observe-refresh.v1",
      tenant_id: "local",
      scan_id: "scan-e2e-1",
      candidates_found: 2,
      resource_events: 1,
      identity_events: 1,
      tool_events: 1,
      usage_events: 1,
      exact_usage_events: 1,
      estimated_usage_events: 0,
      capture_quality: ["exact"],
      limitations: [],
      next_steps: [],
    });
  });

  await page.route("**/v1/tenants/local/connectors", (route) => {
    if (route.request().method() === "GET") {
      return json(route, []);
    }
    return json(route, { id: "mock-connector", ok: true });
  });
  await page.route("**/v1/tenants/local/policy-presets", (route) =>
    json(route, []),
  );
  await page.route("**/v1/tenants/local/telemetry/cost-ledger", (route) =>
    json(route, []),
  );
  await page.route("**/v1/tenants/local/telemetry/alerts", (route) =>
    json(route, []),
  );
  await page.route("**/v1/tenants/local/bundles", (route) => json(route, []));
  await page.route("**/v1/tenants/local/settings", (route) =>
    json(route, { ok: true }),
  );
  await page.route("**/v1/tenants/local/discovery/scan", (route) => {
    scanStarted = true;
    return json(route, {
      status: "completed",
      findings: [candidate, googleAiStudioCandidate],
    });
  });
}
