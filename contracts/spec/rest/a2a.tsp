import "@typespec/http";
import "@typespec/rest";
using TypeSpec.Http;
using TypeSpec.Rest;

namespace Pollek.Contract;

model A2AMessage {
  schema_version: "a2a-message.v1";
  sender_id: string;
  receiver_id: string;
  payload_encrypted: string;
  signature: string;
}

model A2AMessageResponse {
  schema_version: "a2a-response.v1";
  status: string;
  reason?: string;
}

@tag("A2A Mediator")
@route("/v1/a2a/message")
interface A2AApi {
  @doc("Send an Inter-Agent Trust Protocol (IATP) message via the DEK mediator.")
  @post
  sendMessage(
    ...PollekHeaders,
    @body body: A2AMessage
  ): A2AMessageResponse | PollekError;
}
