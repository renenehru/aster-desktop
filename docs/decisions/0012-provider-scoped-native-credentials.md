# ADR-0012: Isolate Native Credentials by Provider

**Status:** Accepted

**Date:** 2026-07-13

**Extends:** `ADR-0002` and `ADR-0008`

**Decision owners:** `FR-023`, `SEC-001`-`SEC-005`, `SEC-048`

## Context

A single generic target cannot safely support multiple providers. The renderer must be able to request setup for a provider without ever carrying the key, while Rust must prevent provider-ID manipulation from selecting a wrong target, prompt label, or authorization header. The reviewed native Windows prompt boundary remains the narrowest secret-capture surface.

## Decision

- The credential command carries only a registry-valid provider ID; it never carries a secret value.
- Rust derives fixed English prompt copy and a distinct application credential target from that provider record.
- Confirmation replaces only the selected provider entry, cancellation changes nothing, and removal deletes only that entry.
- Provider status exposes only provider ID and configured/not-configured/cancelled state.
- A request adapter may read only its matching provider target and holds the key for the minimum practical lifetime.
- At most one native prompt is in flight, retaining every buffer, modal ownership, zeroization, unsafe-FFI, truncation-sentinel, and safe-error invariant in `ADR-0008`.
- Browser demo has no credential command or substitute.

## Consequences

- A user may configure any subset of catalog providers independently.
- Cross-provider target and authorization-capture tests become mandatory.
- Removing one key does not remove history, another provider key, usage, or budget.
- Credential targets are internal policy and never appear in React, errors, or export.

## Rejected alternatives

- A shared generic target with a provider name inside the secret.
- One global API-key field or renderer-managed key map.
- Sending a key through typed IPC because the command is provider-specific.
- Reusing the Z.AI key for a compatible API or silently selecting another target.

## Verification

Primary evidence is `AC-041`, `AC-005`, `AC-019`, `AC-025`, and the packaged Windows portion of `AC-035`. No real credential is permitted in the fixtures.
