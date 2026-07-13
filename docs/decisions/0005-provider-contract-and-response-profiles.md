# ADR-0005: Pin the Verified `glm-5.1` Contract and Use Honest Application Response Profiles

**Status:** Accepted

**Date:** 2026-07-12

**Decision owners:** `FR-007`, `FR-013`, `SEC-006`-`SEC-010`

## Context

The attached source plan assumed `glm-5.2`, `enable_thinking`, and `reasoning_effort` values `high` and `max`. The accepted Z.AI contract reviewed for this MVP pins `glm-5.1`, `thinking.type` values `enabled` and `disabled`, `max_tokens`, and SSE chat completions. The general provider reference now also lists `glm-5.2` and limits `reasoning_effort` to that model, but changing models is outside this decision and requires a separate specification change. Presenting three provider effort levels for the pinned `glm-5.1` contract would overstate its interface.

## Decision

- Pin MVP requests to the exact origin, path, model, and fields in [../provider-contract.md](../provider-contract.md).
- Expose Fast, Standard, and Deep as application response profiles.
- Map Fast to `thinking.type: "disabled"` and 4,096 `max_tokens`.
- Map Standard to `thinking.type: "enabled"` and 8,192 `max_tokens`.
- Map Deep to `thinking.type: "enabled"` and 16,384 `max_tokens`.
- Describe Standard and Deep as sharing the same provider thinking switch and differing only in application output cap and guidance.
- Keep the request key set exact: `model`, visible `user`/`assistant` `messages`, `stream`, `thinking.type`, and `max_tokens`; rely on documented defaults rather than sending temperature or preserved-thinking fields.
- Discard `reasoning_content` and never preserve it across turns.
- Fail with a stable compatibility error instead of silently changing model or profile when the provider rejects the contract.

## Consequences

- Contract snapshots are small and exact.
- Deep may permit a longer answer but cannot promise deeper provider effort or better quality.
- Provider documentation must be re-verified before a model or request-field change.
- Output caps may affect cost and latency; user copy must not imply they are input-context sizes.

## Rejected alternatives

- Restoring `glm-5.2` or `reasoning_effort` from the source plan without verification.
- Inventing distinct provider settings for Standard and Deep.
- Displaying, storing, exporting, or replaying hidden reasoning.
- Silently falling back to another model or dropping an unsupported field.

## Verification

Primary evidence is defined by `AC-006`, `AC-012`, `AC-019`-`AC-022`, and `AC-029`. The documentation review in the provider contract is specification input, not a `PASS` for those criteria.
