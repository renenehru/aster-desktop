# ADR-0009: Persist No Application Diagnostics in MVP v1

**Status:** Accepted

**Date:** 2026-07-12

**Decision owners:** `SEC-003`, `SEC-020`, `SEC-023`, `SEC-029`-`SEC-031`, `AC-012`, `AC-026`, `AC-029`

**MVP v2 applicability:** Adopted unchanged. Aster MVP v2 adds provider-scoped
safe states but no application diagnostic log, telemetry, analytics, or crash
reporting sink.

## Context

The original architecture described a bounded metadata-only diagnostic log, but the MVP implementation has no logging subsystem, retention configuration, protected viewer, or evidence that every sensitive error path is redacted. Adding one solely to match prose would create a new persistence path for potentially sensitive data without improving the core chat workflow.

## Decision

Aster v1 creates no application diagnostic log, telemetry, analytics, or crash-report upload. Rust maps failures directly to a stable code, bounded English message, and retryable flag, and returns that safe envelope only to the main webview. The application does not print or persist prompts, responses, titles, provider bodies, imports, exports, keys, headers, URLs, paths, SQL details, database rows, or internal causes.

Operating-system records outside Aster's control are not described as application logs. They are inspected only with fake sentinels during authorized verification and are not copied into repository evidence when they contain sensitive content.

## Consequences

- The MVP has no application-owned log retention, file-permission, rotation, or support-correlation surface.
- Safe user errors remain actionable but cannot be correlated to a local application log.
- Filesystem and package inventory can prove the absence of an Aster diagnostic sink.
- Adding diagnostics later is behavior- and privacy-changing work requiring requirements, a data inventory, retention/deletion policy, threat-model update, ADR, redaction tests, and explicit acceptance evidence.

## Rejected alternatives

- Claiming a metadata-only logger that is not implemented.
- Persisting raw internal errors for development convenience.
- Uploading errors, analytics, or crash reports without a separately specified privacy decision.
- Treating an empty or missing evidence artifact as proof that a logger is safe.

## Verification

Static checks reject direct application logging/telemetry dependencies, logger initialization, runtime console/print macros, configured sinks/endpoints, and application log paths. Transitive libraries that contain logging facades are inventoried separately and must have no Aster-configured sink. Error contract tests use unique fake sentinels and assert the exact bounded envelope. Package and runtime filesystem inventories verify no application diagnostic log is created. Network evidence remains required before `AC-029` can pass for a desktop candidate.
