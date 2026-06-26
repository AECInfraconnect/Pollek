import "@typespec/http";
import "@typespec/rest";
using TypeSpec.Http;
using TypeSpec.Rest;

namespace Pollek.Contract;

@tag("LocalDashboard")
@route("/v1/tenants/local")
interface LocalDashboardApi {
  @doc("Start Local Scan Session")
  @route("scan")
  @post startLocalScanSession(...PollekHeaders): void | PollekError;

  @doc("Get Local Scan Session by ID")
  @route("scans/{scan_id}")
  @get getLocalScanSession(@path scan_id: string, ...PollekHeaders): unknown | PollekError;

  @doc("Stream Local Scan Session Events (SSE)")
  @route("scans/{scan_id}/events")
  @get streamLocalScanSessionEvents(@path scan_id: string, ...PollekHeaders): void | PollekError;

  @doc("Get latest local capability snapshot")
  @route("capability-snapshot")
  @get getLocalCapabilitySnapshot(...PollekHeaders): unknown | PollekError;

  @doc("List suggested policies based on capabilities and discovered agents")
  @route("policy-suggestions")
  @get listLocalPolicySuggestions(...PollekHeaders): unknown | PollekError;

  @doc("Check policy feasibility")
  @route("policies/feasibility")
  @post checkLocalPolicyFeasibility(...PollekHeaders, @body body: unknown): unknown | PollekError;

  @doc("Create a deployment session")
  @route("deployment-sessions")
  @post createLocalDeploymentSession(...PollekHeaders, @body body: unknown): unknown | PollekError;

  @doc("Get deployment session")
  @route("deployment-sessions/{id}")
  @get getLocalDeploymentSession(@path id: string, ...PollekHeaders): unknown | PollekError;

  @doc("Stream deployment session events (SSE)")
  @route("deployment-sessions/{id}/events")
  @get streamLocalDeploymentSessionEvents(@path id: string, ...PollekHeaders): void | PollekError;

  @doc("Approve an action in a deployment session")
  @route("deployment-sessions/{id}/actions/{action_id}/approve")
  @post approveLocalDeploymentAction(@path id: string, @path action_id: string, ...PollekHeaders): void | PollekError;

  @doc("Get health of local enforcement layer")
  @route("health/local-enforcement")
  @get getLocalEnforcementHealth(...PollekHeaders): unknown | PollekError;
}
