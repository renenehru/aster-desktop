## Summary

<!-- Explain the outcome and why this change is needed. -->

## Scope and non-goals

<!-- Identify what is intentionally included and excluded. -->

## Specification and threat traceability

- Changed product/security requirement IDs:
- Acceptance-criterion IDs:
- Threat IDs reviewed:
- ADRs added, superseded, or reviewed:
- Non-behavioral justification, if no requirement applies:

<!-- Behavior, data-flow, dependency, permission, and security-sensitive changes must update specifications before implementation. -->

## Implementation

<!-- Describe the smallest implementation used and any affected trust boundaries, data flows, permissions, storage, provider behavior, or dependencies. -->

## Verification evidence

| Gate or criterion | Outcome (`PASS`/`FAIL`/`NOT RUN`) | Exact command or procedure | Environment and UTC time | Evidence location | Revision/artifact SHA-256 |
| ----------------- | --------------------------------- | -------------------------- | ------------------------ | ----------------- | ------------------------- |
|                   |                                   |                            |                          |                   |                           |

<!-- Keep superseded or retried failures visible. State exactly what each PASS exercised. -->

## Security and privacy review

- [ ] No real API key, authorization data, personal conversation, private path, or sensitive fixture is included.
- [ ] The API key does not enter React state, browser storage, SQLite, logs, source, assets, or IPC.
- [ ] Rust revalidates changed inputs and security-relevant state transitions.
- [ ] Untrusted Markdown, imports, database data, IPC, and provider events are handled safely where affected.
- [ ] No wildcard or out-of-scope Tauri, network, filesystem, URL, process, or shell permission was added.
- [ ] CSP, external navigation, redaction, cancellation, and persistence impacts were reviewed where applicable.
- [ ] New or changed dependencies have a documented need, license, maintenance, footprint, and security assessment.
- [ ] I am authorized to submit this contribution under Apache-2.0 and have identified all third-party material and required attribution.
- [ ] Security regression tests were added for every vulnerability fixed.

## User experience and documentation

- [ ] User-facing text and repository documentation are in English.
- [ ] Loading, empty, success, failure, offline, cancellation, and confirmation states were considered where applicable.
- [ ] Keyboard operation, focus, accessible names, contrast, reduced motion, and Windows scaling impacts were reviewed.
- [ ] Product, architecture, security, threat, acceptance, ADR, and provider-contract documents are current where applicable.

## Release impact and residual risk

<!-- List every residual risk, deferred item, required NOT RUN/FAIL gate, exception, migration concern, or rollback constraint. Do not claim production readiness without evidence. -->

- Intended classification: documentation only / source change / browser preview / engineering build / production candidate
- Residual risks:
- Required gates not run or failed:
- Follow-up issues:

## Reviewer checklist

- [ ] The change matches the controlling specification and does not expand scope silently.
- [ ] Tests cover relevant normal, boundary, failure, cancellation, and abuse paths.
- [ ] Evidence is revision-specific and does not overstate its scope.
- [ ] No unresolved conflict exists among governance, ADRs, security requirements, product requirements, acceptance criteria, implementation, and tests.
