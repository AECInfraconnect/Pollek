# Pollek Cloud Consumer Guide for Contract Hub

Pollek Cloud must consume the generated API schemas and validation files from this Hub.
It must not implement its own conflicting schemas.

1. Fetch Contract Hub artifacts during build.
2. Ensure provider conformance testing passes against OpenAPI specs.
3. Serve `/.well-known/pollek-contract` using `contract-discovery.v1.schema.json`.
4. Validate all incoming Telemetry and Bundle requests against JSON schemas.
