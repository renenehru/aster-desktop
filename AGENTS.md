# Engineering Governance for Aster Desktop

This file is the governing instruction set for every human or automated contributor to this repository. It applies to the entire repository unless a deeper `AGENTS.md` imposes stricter rules. Product decisions, user-facing text, source code, tests, configuration, commit messages, and evidence produced for this project must be written in English.

## 1. Product and trust boundary

The product is a Windows 11 desktop chat client built with Tauri, React, TypeScript, Rust, SQLite, and the official Z.AI chat API. The verified MVP provider contract uses `glm-5.1`; do not restore the source plan's unverified `glm-5.2`, `reasoning_effort: high`, or `reasoning_effort: max` assumptions without a new contract verification and spec change. The dated external contract record is [docs/provider-contract.md](docs/provider-contract.md). The React webview is a presentation layer and is never a trusted place for secrets or privileged operations.

The provider exposes a verified `thinking.type` switch with `enabled` and `disabled` values. Fast, Standard, and Deep are application-owned response profiles. They must not be described as three provider reasoning-effort levels: Standard and Deep both send `thinking.type: "enabled"` and differ only in the application's output-token cap and user guidance. A provider-contract document or passing contract test verifies an external interface; it does not by itself prove that the shipped implementation uses that interface correctly.

The following boundaries are non-negotiable:

- React may call only explicitly registered, typed Tauri commands and may not call Z.AI directly.
- The API key may exist only in the Rust process for the minimum practical lifetime and in the Windows credential store at rest. It must never be placed in frontend state, browser storage, SQLite, logs, fixtures, source code, build arguments, or packaged assets.
- Credential setup must use the Rust-owned native Windows prompt defined by `ADR-0008`. The webview must not render a credential field or carry the key in an IPC argument, result, or event.
- The Rust backend owns provider networking, SSE parsing, request cancellation, persistence, import/export validation, and security-relevant policy decisions.
- Provider traffic is restricted to the documented Z.AI HTTPS origin and uses normal platform certificate validation. Certificate bypasses are prohibited.
- Model output, imported conversations, database content, IPC arguments, and stream events are untrusted input.
- Arbitrary command execution, shell access, filesystem tools, projects, document attachments, and model-invoked tools are outside MVP v1. A visible preview must be labelled unavailable and must not silently expose a backend capability.
- Browser-only mode is for visual QA. It may use an in-memory demo adapter, but it must not accept, request, persist, or transmit an API key and must clearly identify itself as demo mode.

The normative product boundary is in [docs/product-spec.md](docs/product-spec.md). The architecture and trust boundaries are in [docs/architecture.md](docs/architecture.md).

## 2. Spec-Driven Development policy

No behavior-changing implementation starts from an informal request alone. The specification is the source of truth, and the implementation is evidence that the specification is met.

### 2.1 Requirement language and identifiers

Use RFC 2119-style `MUST`, `MUST NOT`, `SHOULD`, and `MAY` deliberately. Every normative behavior must have a stable identifier:

| Prefix | Owner document                  | Purpose                                      |
| ------ | ------------------------------- | -------------------------------------------- |
| `PR-`  | `docs/product-spec.md`          | Product and user outcome                     |
| `FR-`  | `docs/product-spec.md`          | Functional requirement                       |
| `UX-`  | `docs/product-spec.md`          | User experience or accessibility requirement |
| `NFR-` | `docs/product-spec.md`          | Non-functional requirement                   |
| `SEC-` | `docs/security-requirements.md` | Security or privacy requirement              |
| `TM-`  | `docs/threat-model.md`          | Threat or abuse case                         |
| `AC-`  | `docs/acceptance-criteria.md`   | Executable acceptance criterion              |
| `ADR-` | `docs/decisions/`               | Architecture decision                        |

Identifiers are permanent. Do not renumber reused IDs; mark retired requirements as `Retired` with a reason. New requirements receive new IDs.

### 2.2 Required change workflow

For every behavior, data-flow, dependency, permission, or security-sensitive change:

1. **Specify**: add or update the relevant `PR-`, `FR-`, `UX-`, `NFR-`, or `SEC-` requirement and its acceptance criterion before implementation.
2. **Model**: update the architecture and threat model when a trust boundary, data flow, asset, actor, permission, parser, persistence path, or external service changes.
3. **Decide**: add an ADR when the change is difficult to reverse, affects a trust boundary, introduces a new dependency class, or changes the supported security posture.
4. **Trace**: update the traceability matrix in `docs/acceptance-criteria.md`. Every in-scope normative requirement must map to at least one acceptance criterion and verification method.
5. **Test first**: create or update a failing automated test when automation is practical. If it is not practical, define a deterministic manual verification procedure and explain why.
6. **Implement minimally**: implement only the specified behavior. Do not create dormant permissions or speculative privileged APIs for future features.
7. **Verify**: run the applicable quality and security gates in section 5 and record evidence. A passing UI screenshot alone is not evidence for backend or security requirements.
8. **Review drift**: compare implementation, configuration, docs, and tests. Update the spec only when the intended product decision changed, never merely to excuse a defect.

A pull request or handoff must list the requirement IDs changed, threat IDs reviewed, test/evidence locations, and any residual risk. A change with no applicable requirement must say why it is non-behavioral.

### 2.3 Source-of-truth order

When documents conflict, resolve them in this order:

1. This `AGENTS.md` for engineering process and non-negotiable controls.
2. Accepted ADRs for deliberate architecture decisions.
3. Security requirements for security and privacy behavior.
4. Product specification for scope and functional behavior.
5. Acceptance criteria for verification detail.
6. Implementation and tests.

Conflicts must be corrected in the same change. Implementation does not silently override a specification.

## 3. Secure development rules

### 3.1 Secrets and privacy

- Never commit a real API key or personal conversation data. Use unmistakably fake values such as `test_key_not_a_secret` in tests.
- Do not print authorization headers, credential-store values, prompts, responses, imported content, database rows, or full provider payloads. Logs use request-scoped opaque IDs and safe metadata only.
- Zeroize or drop secret-bearing values as soon as practical. Error types and debug formatting must not include secrets.
- Local conversations are not synchronized by the application. Any content sent to Z.AI requires a visible external-processing notice.
- Export is an explicit user action. The UI warns that an export can contain sensitive plaintext.

### 3.2 Frontend and rendering

- Treat Markdown and model output as hostile. Raw HTML is disabled. An allowlist renderer that creates React elements without an HTML sink is preferred; any HTML sink requires the approved allowlist sanitizer immediately before the sink.
- Reject dangerous URL schemes. Links may use only explicitly allowed schemes, open with safe external-navigation behavior, and never execute inside the webview.
- Do not use `dangerouslySetInnerHTML` unless the value has passed the approved sanitizer immediately before the sink and the use is covered by a security test.
- Do not load runtime JavaScript, fonts, styles, analytics, or other active content from remote origins. Keep the Content Security Policy restrictive and documented.
- UI state must not be treated as authorization. Revalidate all inputs and state transitions in Rust.

### 3.3 Tauri IPC and permissions

- Expose the smallest command surface. Commands use typed request/response structures, validate length and shape, and return stable error codes without sensitive detail.
- Tauri capabilities and allowlists grant only permissions exercised by an in-scope requirement. Wildcard shell, process, URL, filesystem, or network permissions are prohibited.
- Privileged commands must be invocable only by the main application window and must not accept arbitrary paths or URLs.
- Request IDs are unguessable enough to avoid cross-stream confusion. Stream events are associated with their originating conversation and request, and stale events are ignored.
- Cancellation must propagate to the Rust HTTP request; hiding a stream in the UI is not cancellation.

### 3.4 Backend, storage, imports, and exports

- Use parameterized SQL exclusively. Apply schema migrations transactionally and back up or fail safely before a destructive migration.
- Validate all imported data against a versioned schema, total-size limit, field-size limits, nesting/collection limits, and allowed enum values before a transaction begins.
- Import must not interpret HTML, scripts, paths, URLs, or commands. An invalid import must not partially mutate the database.
- Export paths must come from the native save dialog or an equivalent scoped handle. Do not concatenate user input into filesystem paths.
- Destructive actions require deliberate UI confirmation and transaction-safe backend behavior.
- Network operations have explicit connect, request, idle/read, and overall limits as applicable. Retries are bounded, use jittered backoff, honor provider guidance, and do not retry authentication failures.

### 3.5 Dependencies and supply chain

- Lock JavaScript and Rust dependency versions. Commit and review lockfile changes.
- Prefer maintained, narrowly scoped packages. Document why a new runtime dependency is required and assess its license, maintenance, transitive footprint, and security history.
- Do not run untrusted lifecycle scripts or download executable code at runtime.
- CI produces a software bill of materials and runs vulnerability, secret, and license/policy checks. A known exploitable high or critical issue blocks release unless a time-bounded exception is approved.
- Production installers and update artifacts must be signed and verifiable. If signing evidence is unavailable, artifacts are engineering builds and must not be represented or distributed as a production release. Auto-update remains disabled until a signed update chain is verified.

## 4. Implementation and test standards

- TypeScript uses strict typing. Avoid `any`; when unavoidable at an untyped boundary, isolate it, validate immediately, and explain it.
- Rust must compile without warnings in release CI. Avoid `unsafe`; any necessary use requires an ADR, a safety invariant, focused tests, and explicit security review.
- Validate at trust boundaries with explicit schemas or typed deserialization plus semantic checks. Frontend validation is for usability, not security.
- Errors shown to users are actionable and do not expose internals. Preserve stable machine-readable error codes for tests.
- Tests must be deterministic, isolated from a real user credential store and database, and must never call a billable production endpoint by default.
- Provider integration tests use a controlled fake SSE server unless an explicitly authorized, secret-scoped end-to-end job is selected.
- Security regression tests are required for every fixed vulnerability.
- Accessibility is part of correctness: keyboard navigation, focus visibility, semantic controls, accessible names, and Windows scaling behavior are tested.

Expected verification layers:

| Layer       | Minimum purpose                                                                                         |
| ----------- | ------------------------------------------------------------------------------------------------------- |
| Unit        | Reducers/state machines, validators, redaction, SSE parsing, repository logic                           |
| Component   | Keyboard/focus behavior, composer states, safe Markdown, confirmations, errors                          |
| Integration | Typed IPC, SQLite migrations/CRUD, credential-store adapter, import transactions, cancellation          |
| Contract    | Provider request mapping and valid/malformed/partial SSE fixtures                                       |
| End-to-end  | First run, credential setup, chat lifecycle, restart persistence, stop, edit, regenerate, import/export |
| Security    | Secret scanning, CSP/permission assertions, injection corpus, path/schema abuse, log redaction          |

## 5. Mandatory verification gates

Use the repository scripts that implement these gates. If a named script does not exist yet, creating it is part of enabling the associated release gate; do not report the gate as passed.

### 5.1 Every change

- Formatting check for TypeScript, styles, Markdown, and Rust.
- TypeScript type check and lint with zero unexplained warnings.
- Rust format check and `clippy` with warnings denied.
- Relevant unit/component/integration tests.
- Secret scan of tracked content and generated logs/fixtures.
- Review of requirement and traceability impact.

### 5.2 Security-sensitive or release-candidate changes

- Full frontend and Rust test suites.
- Tauri capability/CSP assertion tests.
- Dependency vulnerability and policy audit for both ecosystems.
- SBOM generation for both ecosystems or a combined release SBOM.
- Static analysis and unsafe-code review.
- Import, Markdown, SSE, IPC, and error-message abuse tests.
- Windows 11 end-to-end smoke test from a clean user profile.
- Installer signature verification for a production release.
- Evidence that packaged frontend assets and logs contain no credential or source map exposing secrets.

### 5.3 Gate outcomes

Gates have only three outcomes: `PASS`, `FAIL`, or `NOT RUN`. Missing tools, missing evidence, or an unavailable signing identity are `NOT RUN`, never `PASS`. `FAIL` and required `NOT RUN` outcomes block a production release.

Every recorded gate outcome must identify the source revision, exact command or deterministic procedure, environment, UTC timestamp, evidence location, and artifact hash when an artifact was tested. `PASS` applies only to the scope actually exercised. A successful browser demo, source inspection, type check, unit test, or screenshot cannot be promoted into evidence for desktop IPC, Windows credential storage, provider networking, SQLite persistence, installer behavior, signing, or any other unexercised boundary. Superseded or retried failures remain visible in the evidence history.

## 6. Definition of Done

A change is done only when all applicable items are true:

- The intended outcome and non-goals are represented by stable requirement IDs.
- Architecture, ADRs, data classification, and the threat model reflect any changed boundary or flow.
- Acceptance criteria and the traceability matrix are current.
- Implementation contains no out-of-scope permission or hidden capability.
- Automated tests cover normal, boundary, failure, cancellation, and abuse paths appropriate to the change.
- Applicable quality and security gates pass, with commands and artifacts recorded in the handoff or CI run.
- User-facing states cover loading, empty, success, failure, offline, cancellation, and destructive confirmation where applicable.
- Accessibility and Windows 11 scaling/keyboard behavior are verified for UI changes.
- No secret, sensitive content, debugging bypass, certificate bypass, wildcard permission, or production-only test endpoint was introduced.
- Documentation and UI copy are English and accurately describe current capability.
- Residual risks, deferred requirements, and `NOT RUN` gates are explicit. Nothing is described as secure, signed, production-ready, or complete without evidence.

## 7. Exceptions and vulnerability handling

An exception is allowed only when all of the following are recorded: affected requirement IDs, business reason, risk owner, threat and impact, compensating control, expiry date, and removal issue. Exceptions cannot permit committed secrets, disabled TLS verification, arbitrary shell execution, hidden data transmission, fabricated test evidence, or an unsigned artifact labelled as a production release.

Suspected vulnerabilities take priority over feature work. Preserve evidence without copying secrets, add a regression test, remediate the root cause, rotate exposed credentials outside the repository, and update the threat model. Do not include exploit details or real user data in public logs or screenshots.

## 8. Canonical documents

- [Product specification](docs/product-spec.md)
- [Architecture](docs/architecture.md)
- [Threat model](docs/threat-model.md)
- [Security requirements](docs/security-requirements.md)
- [Acceptance criteria and traceability](docs/acceptance-criteria.md)
- [Verified provider contract](docs/provider-contract.md)
- [Architecture decisions](docs/decisions/README.md)
- [Evidence recording policy](docs/evidence/README.md)
