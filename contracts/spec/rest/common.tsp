import "@typespec/http";
using TypeSpec.Http;
namespace Pollek.Contract;

model PollekHeaders {
  @header("X-Pollek-Contract-Version")
  contractVersion: string;

  @header("X-Pollek-Device-Id")
  deviceId?: string;

  @header("X-Pollek-Tenant-Id")
  tenantId?: string;
}

@error
model PollekError {
  code: string;
  message: string;
  trace_id?: string;
}

model NotModifiedResponse { @statusCode statusCode: 304; }

model Page<T> {
  items: T[];
  next_cursor?: string;
}
