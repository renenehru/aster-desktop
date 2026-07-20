# ADR-0013: Pin Conversation Identity, Preserve Finish Truth, and Keep an Advisory Usage Ledger

**Status:** Accepted

**Date:** 2026-07-13

**Decision owners:** `FR-002`, `FR-022`, `FR-025`, `FR-026`, `FR-029`, `FR-030`, `SEC-010`, `SEC-022`, `SEC-049`, `SEC-052`

## Context

Changing provider/model inside an established conversation can send prior context to an unintended processor, mix credentials and events, and make edit/regenerate history dishonest. Token metadata is also provider consumption evidence that should not disappear when a conversation is deleted, but it is incomplete and cannot become a fabricated billing ledger. MVP v1 total usage may originate from an imported file and is not proven local consumption.

A provider can also stop successfully because Aster's configured output cap was reached. Discarding that fact makes a partial response look normally complete; inferring a normal stop for legacy data without terminal evidence is equally misleading.

## Decision

- New chat inherits only from a fully loaded conversation whose identity equals the current selection; with no selection Rust uses `zai`/`glm-5.1`. New chat is disabled during a selected-conversation load or empty-pair mutation. Navigation invalidates earlier conversation-load, create-selection, and mutation authority; a stale create may retain its summary but cannot steal selection or clear a draft. Background reconciliation uses separate authority and cannot wedge the visible load.
- An empty conversation may change pair. After its first message is persisted, Rust and SQLite treat provider/model as immutable. A different selection creates a distinct empty conversation only after confirmation.
- Edit, resend, regenerate, credential selection, requests, events, import/export, and usage remain bound to the persisted pair.
- Rust normalizes provider usage into four disjoint optional safe-integer fields: non-cached input, cached input, output including provider-documented thought/reasoning tokens, and their checked total.
- Each validated MVP v2 send creates exactly one partial coverage observation immediately before provider networking. Valid authoritative final usage fills it once; cancellation, failure, timeout, missing or malformed usage, and a legal terminal without usage leave it partial. Pre-network rejection creates no observation. Duplicate terminals cannot double count, and an explicit retry is a new operation and observation; there is no automatic retry. It contains no conversation text/title, credential, raw payload, or balance. Recent observations survive conversation deletion because external consumption already occurred.
- Usage aggregates cover the trailing seven days. One local budget per provider uses known total tokens only, is advisory, never blocks a request, and shows an accessible warning at 10% or less remaining. The threshold uses exact integer comparison. If an aggregate exceeds the JavaScript-safe range, the budget remains exhausted, its exact known-used value is null, and coverage is partial rather than silently removing the budget or fabricating a number.
- Missing, invalid, inconsistent, or overflowing metadata yields partial coverage rather than a fabricated zero.
- The v1 migration preserves legacy total only on its message/export. It does not backfill usage observations or budgets; v2 tracking starts with authoritative v2 operations.
- A newly completed assistant response persists the adapter-authenticated finish reason `stop` or `outputLimit`. `outputLimit` preserves validated partial content and usage and triggers an accessible incomplete-response notice. Completed legacy/imported assistant messages without terminal evidence use `unknown`; user, cancelled, and error messages carry no finish reason. Version 2 import/export preserves this constrained enum and never infers `stop`.
- Provider notice acknowledgement is persisted per provider and `noticeVersion`; an increased version requires acknowledgement again.

## Consequences

- Switching model is explicit and cannot silently transfer context.
- Usage remains useful after deletion while storing less information than conversation history.
- Local totals can undercount provider use; the UI must keep partial coverage and provider billing distinct.
- Output-limited responses remain usable while visibly distinct from normal completion and provider failure.
- A schema v2 rebuild, backup-or-abort path, v1/v2 import handling, fixed-clock tests, and idempotency tests are required.

## Rejected alternatives

- Mutating a populated conversation or cloning its context automatically to a new provider.
- Deriving the request pair from current UI selection instead of SQLite.
- Computing usage only from messages, which would erase incurred consumption on delete.
- Treating missing fields as zero, cached input as overlapping input, or unsafe integers as rounded JavaScript numbers.
- Backfilling the v2 ledger from untrusted/importable v1 `tokenUsage`.
- Treating every completed message as a normal stop or converting output-limit completion into an error.
- Blocking requests at the local budget or calling it provider credit.

## Verification

Primary evidence is `AC-042` through `AC-045` and `AC-048`, plus migration/import evidence in `AC-026` and `AC-027`.
