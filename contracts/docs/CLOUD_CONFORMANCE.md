# Cloud Conformance Guide

Any Pollek Cloud implementation must conform to the shared, generated contract
so the local service can talk to it without client changes.

- It must consume generated artifacts (OpenAPI, JSON Schema).
- It must not use independent logic for schema validation.
- Implement tests to ensure cloud behavior strictly matches provider requirements.
