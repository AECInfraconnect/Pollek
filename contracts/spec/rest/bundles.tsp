import "@typespec/http";
using TypeSpec.Http;
namespace Pollek.Contract;

model BundleFetchRequest {
  tenant_id: string;
  device_id: string;
  current_generation?: int32;
  capabilities: string[];
}

model BundleFetchResponse {
  schema_version: "bundle-fetch-response.v1";
  status: "not_modified" | "bundle_ready";
  generation?: int32;
  envelope?: unknown; // JSON Schema validates the exact bundle-envelope.v1 shape.
}

@tag("Bundles")
@route("/v1/tenants/{tenant_id}/devices/{device_id}/bundles/latest")
interface BundleApi {
  @doc("Fetch the latest signed policy bundle manifest for a device.")
  @post
  fetchLatest(
    ...PollekHeaders,
    @path tenant_id: string,
    @path device_id: string,
    @body body: BundleFetchRequest
  ): BundleFetchResponse | NotModifiedResponse | PollekError;
}
