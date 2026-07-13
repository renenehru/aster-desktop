# ADR-0004: Isolate the Browser Demo from Desktop Capabilities and Secrets

**Status:** Accepted

**Date:** 2026-07-12

**Decision owners:** `SEC-005`, `SEC-031`, `SEC-039`

## Context

Visual and accessibility work benefits from running the React interface without Tauri. A browser cannot satisfy the desktop credential, networking, persistence, IPC, package, or installer controls. A demo that requests an API key or looks like a verified desktop session would create secret risk and misleading evidence.

## Decision

Browser-only execution is an explicit `Demo mode` profile. It:

- uses deterministic, non-sensitive, in-memory sample conversations and responses;
- makes no Z.AI, credential, Tauri IPC, telemetry, or analytics call;
- exposes no API-key input and rejects any credential operation;
- loses state on reload and says so in the UI;
- may use a user-initiated browser file picker and local download only to exercise demo import/export presentation;
- never contributes evidence for Rust validation, native dialogs, SQLite, Windows Credential Manager, provider traffic, package policy, signing, or restart persistence.

The desktop profile remains subject to the Rust trust boundary and must not silently fall back to a demo after a desktop initialization or IPC failure.

## Consequences

- UI work can be reviewed without a credential or paid endpoint.
- Automated browser tests can prove presentation and accessibility assertions only.
- The demo requires persistent visible labelling and honest connection/storage copy.
- Desktop and browser import/export use different I/O boundaries and require separate tests.

## Rejected alternatives

- Asking for or storing a real key in browser mode.
- Calling a provider directly from the browser for convenience.
- Treating browser local storage or browser downloads as desktop persistence evidence.
- Hiding the profile distinction in a connection-status tooltip.

## Verification

`AC-032` is the primary isolation criterion. Applicable presentation assertions may also contribute narrowly to `AC-001`, `AC-004`, `AC-008`, `AC-009`, and `AC-017`. All desktop and security-boundary assertions remain `NOT RUN` until exercised in their required environment.
