// Ground-truth generator for a Cloud-signed bundle-manifest.v2 vector.
// Mirrors AECInfraconnect/Pollek-Cloud apps/api/server.mjs EXACTLY:
//   - stableJson()                       (server.mjs ~L759)
//   - unsignedPolicyBundleManifest shape (server.mjs ~L2020)
//   - signPolicyBundle()                 (server.mjs ~L2122): ed25519 over
//     Buffer.from(stableJson(manifest)), signature base64url, key as SPKI PEM.
// Uses only Node crypto builtins — identical to the Cloud — so the emitted
// manifest + signatures[] is byte-identical to a real Cloud
// GET /v1/policy-bundles/{id}/manifest response. Regenerate with:
//   node gen_cloud_manifest.mjs > cloud_signed_manifest.json
import crypto from "node:crypto";

function stableJson(value) {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map((i) => stableJson(i)).join(",")}]`;
  return `{${Object.keys(value)
    .sort()
    .map((k) => `${JSON.stringify(k)}:${stableJson(value[k])}`)
    .join(",")}}`;
}
const sha256 = (v) => crypto.createHash("sha256").update(String(v)).digest("hex");

const policies = [{ control: "AC-2", engines: ["cedar", "wasm"] }];
const artifacts = [
  {
    name: "policy.wasm",
    media_type: "application/wasm",
    sha256: sha256("WASM-POLICY-BYTES"),
    size_bytes: 17,
  },
];
const manifest = {
  manifest_version: "1.0",
  schema_version: "bundle-manifest.v2",
  bundle_id: "bnd_eu_ai_act_high_risk_ab12cd34",
  tenant_id: "local",
  revision: "2026.07.23.001",
  created_at: "2026-07-23T00:00:00.000Z",
  target: {
    control_level: "Enforce",
    pep_capabilities: ["mcp-stdio", "http-proxy"],
    agent_selectors: [{ kind: "label", value: "managed=true" }],
  },
  policies,
  artifacts,
  compliance_bundle_id: "cmp_eu_ai_act_high_risk",
  hot_reload: true,
  approval: {
    approval_id: "approval_eu_ai_act_1",
    status: "approved",
    approved_by: "local-dev-compliance-admin",
    approved_at: "2026-07-23T00:00:00.000Z",
    source: "enterprise_compliance_bundle",
  },
  source_hashes: {
    policies_sha256: sha256(stableJson(policies)),
    artifacts_sha256: sha256(stableJson(artifacts)),
  },
};

const { publicKey, privateKey } = crypto.generateKeyPairSync("ed25519");
const payload = stableJson(manifest);
const payloadHash = sha256(payload);
const sig = crypto.sign(null, Buffer.from(payload), privateKey).toString("base64url");
const publicKeyPem = publicKey.export({ type: "spki", format: "pem" }).toString();

const wire = {
  ...manifest,
  payload_hash: payloadHash,
  signatures: [
    {
      id: "sig_fixture_1",
      schema_version: "pollek.cloud.policy-bundle-signature.v1",
      tenant_id: "local",
      bundle_id: manifest.bundle_id,
      revision: manifest.revision,
      alg: "Ed25519",
      key_id: "local-dev-ed25519",
      sig,
      payload_hash: payloadHash,
      public_key_pem: publicKeyPem,
      signed_by: "local-dev-compliance-admin",
      signed_at: "2026-07-23T00:00:00.000Z",
    },
  ],
};
process.stdout.write(JSON.stringify(wire, null, 2) + "\n");
