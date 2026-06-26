# Changelog

All notable changes to the Pollek Contract Hub will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Optional `ResourceTraceDetails` on resource access telemetry and resource
  inventory items so Local Dashboard and Pollek Cloud can share exact file,
  folder, database, table/collection, and provenance metadata.
- Initial setup of Pollek Contract Hub.
- TypeSpec REST specs for basic endpoints.
- AsyncAPI spec for SSE.
- JSON Schemas for bundle envelope, signatures, manifest, and telemetry.
- Rust code generation structure.
