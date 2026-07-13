# ADR-0001: Keep Credentials and Provider Networking Behind the Rust Boundary

**Status:** Accepted

**Date:** 2026-07-12

**Decision owners:** `PR-002`, `SEC-001`-`SEC-011`, `SEC-017`-`SEC-021`

## Context

The React webview is inspectable presentation code and processes untrusted model content. Giving it a provider credential or direct external networking would expose the secret to frontend state and would let a compromised renderer bypass backend policy. Streaming and cancellation also need one authoritative owner to prevent stale cross-conversation events and paid requests that continue after the UI stops.

## Decision

The React application is an untrusted presentation layer. It may call only typed, registered Tauri commands and consume bounded typed events. Rust owns:

- Windows credential-store access and the minimum-lived secret value;
- exact-origin provider HTTP, TLS, authorization injection, request mapping, timeouts, bounded retry, SSE parsing, and cancellation;
- conversation state transitions, SQLite transactions, import/export validation, native file dialogs, and safe error classification;
- enforcement of the main-window command boundary and event/request identity.

The API key is captured through a Rust-owned native Windows credential prompt and never enters the React DOM, frontend state, or any IPC argument/result/event. React receives only configured/not-configured state. Provider URLs, models, headers, paths, and arbitrary request fields are not IPC inputs.

## Consequences

- The privileged command and capability surface remains small and inventoryable.
- Fake Rust adapters and a local SSE server are required for deterministic tests.
- The backend must emit incremental events without placing policy authority in UI state.
- More Rust implementation is required, but secret and network policy remain outside the renderer.
- Browser-only execution cannot exercise this boundary and has no evidentiary value for it.

## Rejected alternatives

- Calling Z.AI from React, even with an obfuscated or environment-injected key.
- Running a local HTTP proxy whose arbitrary origin is chosen by the renderer.
- Treating UI visibility, disabled controls, or request IDs generated only by React as authorization.

## Verification

Primary evidence is defined by `AC-005`-`AC-007`, `AC-019`-`AC-022`, `AC-025`, `AC-029`, and `AC-035`. This ADR records the decision only; those criteria remain `NOT RUN` until a revision-specific evidence report says otherwise.
