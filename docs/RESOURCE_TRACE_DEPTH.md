# Resource Trace Depth

POLLEK uses an exact-first model for Data Resources. A resource access event may
include `details` with the most specific object the local host can prove:
file name, folder name, database namespace, table/collection, query summary, and
query fingerprint. If the local source cannot prove that object, POLLEK records a
clear `capture_quality` and `trace_source` instead of guessing.

## Contract

`ResourceAccessPayload.details` and `ObservedResource.details` are optional
Contract Hub fields shared by Local Dashboard and Pollek Cloud. Older producers
may omit them. New producers should fill them when they have source-backed
evidence.

Important fields:

- `trace_source`: source such as `windows_security_4663`, `windows_minifilter`,
  `linux_fanotify`, `linux_ebpf`, `macos_endpoint_security`, `mcp_wrapper`,
  `browser_extension`, `sqlite_trace_v2`, `postgres_pg_stat_statements`,
  `mysql_performance_schema`, or `known_agent_log_or_session_file`.
- `capture_quality`: `exact_os_audit`, `exact_db_hook`, `exact_wrapper`,
  `exact_local_log_metadata`, or `observed_metadata`.
- `trace_granularity`: `file_path`, `folder_path`, `database_file`,
  `db_table`, `db_collection`, `query_fingerprint`, `host`, or `process_path`.
- `raw_content_stored`: must stay `false` for path/query metadata capture unless
  a user explicitly enables a content capture plugin.

## OS Capabilities

Windows:

- Security Audit Event 4663 can provide object type, object name, access mask,
  process ID, and process name when SACL auditing is configured on the object.
- ETW can provide lower-overhead file/process telemetry but is observe-first.
- File system minifilter is the deepest enforceable path for production-grade
  file open/read/write decisions, but requires signed driver installation.

Linux:

- fanotify can observe filesystem access and, with permission events and
  privileges, can support allow/deny for selected file operations.
- auditd/eBPF can add process, path, syscall, and container context.
- Landlock/seccomp/eBPF-LSM can enforce only where installed and permitted.

macOS:

- EndpointSecurity is the preferred path for exact file/process observe and
  authorization events.
- FSEvents can report filesystem changes, but it is not enough for per-agent
  read decisions.
- NetworkExtension covers network policy, not file/table-level data resources.

Databases:

- SQLite should use `sqlite3_trace_v2` from a wrapper/extension when POLLEK owns
  the connection path.
- PostgreSQL can use `pg_stat_statements`, configured SQL logging, or an
  OpenTelemetry-instrumented client to provide normalized query IDs and table
  context.
- MySQL/MariaDB can use Performance Schema statement tables or an
  OpenTelemetry-instrumented client.
- If only encrypted network metadata is available, POLLEK records the host as an
  API/data-source resource and does not invent table names.

## Current Local Implementation

The local observe refresh path extracts exact metadata from:

- wrapper/proxy/browser/telemetry `resource_access` events,
- known agent JSON/JSONL session logs with structured path or SQL fields,
- discovery evidence such as MCP config paths, process paths, browser sessions,
  and network SNI.

The local log extractor stores object metadata, path hashes, query summaries, and
query fingerprints. It does not store prompt bodies, response bodies, raw SQL, or
file contents.

Sources:

- Microsoft Event 4663 object access audit:
  <https://learn.microsoft.com/en-us/windows/security/threat-protection/auditing/event-4663>
- Microsoft file system filter manager/minifilter concepts:
  <https://learn.microsoft.com/en-us/windows-hardware/drivers/ifs/file-system-minifilter-drivers>
- Linux fanotify:
  <https://man7.org/linux/man-pages/man7/fanotify.7.html>
- Apple EndpointSecurity:
  <https://developer.apple.com/documentation/endpointsecurity>
- SQLite `sqlite3_trace_v2`:
  <https://www.sqlite.org/c3ref/trace_v2.html>
- PostgreSQL `pg_stat_statements`:
  <https://www.postgresql.org/docs/current/pgstatstatements.html>
- MySQL Performance Schema statement event tables:
  <https://dev.mysql.com/doc/refman/8.4/en/performance-schema-statement-tables.html>
- OpenTelemetry database semantic conventions:
  <https://opentelemetry.io/docs/specs/semconv/db/database-spans/>
