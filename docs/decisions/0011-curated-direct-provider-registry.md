# ADR-0011: Use a Curated Direct Provider Registry

**Status:** Accepted

**Date:** 2026-07-13

**Supersedes:** `ADR-0005` for MVP v2; the exact `glm-5.1` fixture remains a regression contract

**Decision owners:** `PR-004`, `FR-020`, `FR-021`, `FR-024`, `SEC-046`, `SEC-047`

## Context

MVP v2 requires multiple model providers without allowing the untrusted renderer, an import, or user configuration to choose a network destination or arbitrary model. Provider names do not prove hosted API availability, and superficially similar chat APIs have different authentication, reasoning controls, stream shapes, usage semantics, regions, and lifecycle. Displaying an unimplemented name as unavailable would create catalog drift and is explicitly outside the product decision.

## Decision

- Rust owns one closed catalog version with an explicit default `zai`/`glm-5.1`.
- The catalog contains only the exact provider/model pairs in the dated provider contract. It has no arbitrary entry and no unavailable state.
- Each provider family has a concrete adapter with a fixed official HTTPS origin/path, authentication, request mapping, stream parser, finish mapping, usage normalization, and retry policy.
- Aster calls the official provider directly from Rust; no inference aggregator, renderer proxy, or remote model-list service participates.
- Fast, Standard, and Deep are Aster profiles. The registry records the exact mapping for each model. Unsupported fields fail before network access; no field/model/provider fallback is allowed.
- Alibaba Cloud is a distinct `alibaba-us` provider with a fixed US endpoint, region-specific key, and disclosure.
- NVIDIA catalog entries are labelled `Hosted prototype` and do not imply a production deployment or enterprise entitlement.
- Adding or removing a pair requires specification, provider-contract, threat, migration/compatibility, test-fixture, and catalog-version review.

## Consequences

- The network and secret boundary is finite, inspectable, and testable with per-adapter snapshots.
- New models require code and evidence rather than a configuration edit.
- Provider-specific semantics remain honest, but adapters and fixtures add implementation work.
- Catalog/model availability can become stale; the entry must be removed through the governed change path when verification fails.

## Rejected alternatives

- User-entered base URLs, provider IDs, headers, or model names.
- OpenRouter or another inference aggregator for the direct-provider MVP.
- Runtime remote model discovery or automatic catalog mutation.
- Displaying requested but unverified names as disabled or unavailable entries.
- A universal reasoning-effort translation or silent provider/model fallback.

## Verification

Primary evidence is `AC-040` and `AC-043`, plus `AC-019`-`AC-022`, `AC-025`, and `AC-033`. This decision and provider documentation are inputs, not passing results.
