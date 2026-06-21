import "@typespec/http";
import "@typespec/rest";
import "@typespec/openapi";
import "./rest/common.tsp";
import "./rest/contract-discovery.tsp";
import "./rest/bundles.tsp";
import "./rest/telemetry.tsp";
import "./rest/registry.tsp";
import "./rest/enrollment.tsp";
import "./rest/a2a.tsp";
import "./rest/pdp-routing.tsp";

using TypeSpec.Http;
using TypeSpec.Rest;
using TypeSpec.OpenAPI;

@service(#{ title: "Pollen DEK Control-Plane API" })
@info(#{
  version: "1.0.0",
  contact: #{ name: "Pollen Platform Team", url: "https://your-cloud-domain.com", email: "platform@your-cloud-domain.com" }
})
@doc("Contract between Pollen Cloud / Local Control Plane and the DEK edge enforcement plane.")
@server("https://api.pollen.cloud", "Pollen Cloud")
@server("http://127.0.0.1:17891", "Local Control Plane / Mock Cloud")
namespace Pollen.Contract;
