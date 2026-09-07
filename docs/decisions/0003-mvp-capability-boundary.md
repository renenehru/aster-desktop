# ADR-0003: Keep Privileged Tools and Attachments Outside MVP v1

**Status:** Accepted

**Date:** 2026-07-12

**Decision owners:** `SEC-017`, `SEC-021`, `SEC-030`, `SEC-039`

**MVP v2 applicability:** Adopted unchanged. Every exclusion and least-privilege
control below remains active for MVP v2 even where the historical decision text
says MVP v1.

## Context

The source plan discussed projects, local files, attachments, document processing, and model-invoked tools. Each would add new privileged data flows, parsers, permissions, prompt-injection paths, and destructive-action risks. They are not required to prove the direct chat lifecycle.

## Decision

MVP v1 has no project directory access, attachments, document parsing, retrieval, function calling, model tools, shell/process execution, arbitrary filesystem access, or arbitrary network capability. No dormant Rust handler, Tauri command, capability, plugin, scope, or package permission is created for them.

The shell may show an original disabled or `Coming soon` preview when it is visibly unavailable, keyboard-safe, and not represented as functional. Import/export of the application's versioned conversation schema is the only in-scope file flow and is not a general filesystem capability.

## Consequences

- Prompt injection cannot directly obtain an application tool or filesystem authority in v1.
- The MVP remains a focused chat client rather than a coding agent.
- Future capability work must start with new `FR-`, `SEC-`, `TM-`, and `AC-` records, a data-flow update, and an ADR before permissions or handlers are added.

## Rejected alternatives

- Shipping hidden backend support behind a disabled frontend control.
- Exposing a general command, path, URL, or filesystem broker for future use.
- Treating user confirmation alone as sufficient containment for arbitrary shell or file access.

## Verification

Primary negative evidence is defined by `AC-001`, `AC-023`, `AC-025`, `AC-029`, `AC-032`, and `AC-033`. This ADR does not prove the absence of packaged capabilities.
