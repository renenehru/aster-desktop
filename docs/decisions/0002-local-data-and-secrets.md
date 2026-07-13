# ADR-0002: Store Conversations Locally and Credentials Separately

**Status:** Accepted

**Date:** 2026-07-12

**Decision owners:** `FR-004`, `FR-006`, `FR-015`, `SEC-001`-`SEC-004`, `SEC-022`-`SEC-032`

## Context

The MVP needs history that survives restart and a provider key that survives relaunch. Conversation content and credentials have different sensitivity, access, export, and lifecycle requirements. Storing both in one application database, browser storage, a configuration file, or the package would increase secret exposure and contradict the desktop trust boundary.

## Decision

- Conversations and messages are stored in a user-scoped SQLite database owned by Rust.
- The Z.AI API key is stored only in an application-specific Windows credential-store entry and is accessed through a Rust adapter.
- SQLite never contains the API key, authorization header, raw provider trace, or `reasoning_content`.
- The application does not claim application-level database encryption. The UI states that history is local plaintext protected by Windows account and filesystem controls.
- Export is an explicit Rust-owned native save-dialog action, produces a minimal versioned plaintext JSON schema, and warns about sensitive content.
- Import is selected by a Rust-owned native open dialog, bounded before allocation, validated completely, and committed in one transaction with backend-generated IDs.
- Tests use a temporary database and isolated fake credential adapter/target, never the user's actual data or credential entry.

## Consequences

- A compromised unlocked Windows account may read local history; this is an explicit residual risk.
- Credential deletion and conversation deletion are distinct operations.
- Database backup does not back up the API key.
- Import/export require schema, size, path-scope, rollback, and sentinel tests.
- A future encrypted database would require migration design, recovery design, new threats, and a separate ADR.

## Rejected alternatives

- Storing the key in SQLite, browser storage, preferences, environment assets, or a local plaintext file.
- Sending the stored key back to React for status or editing.
- Accepting renderer-supplied filesystem paths or constructing export paths from titles.
- Claiming that local storage is private or encrypted without evidence.

## Verification

Primary evidence is defined by `AC-003`, `AC-005`, `AC-014`, `AC-015`, `AC-019`, and `AC-026`-`AC-029`. This ADR does not assert that any result is `PASS`.
