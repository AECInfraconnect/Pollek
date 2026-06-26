import "@typespec/http";
using TypeSpec.Http;
namespace Pollek.Contract;

model ContractDiscoveryResponse {
  schema_version: "contract-discovery.v1";
  supported: string[];
  preferred: string;
  minimum_dek_version?: string;
  sunset?: Record<string>;
  capabilities: string[];
}

@tag("Discovery")
@route("/.well-known/pollek-contract")
interface ContractDiscoveryApi {
  @doc("Discover contract capabilities and supported protocol versions.")
  @get
  getDiscovery(...PollekHeaders): ContractDiscoveryResponse | PollekError;
}
