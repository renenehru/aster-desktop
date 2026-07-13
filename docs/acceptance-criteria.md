# Aster Desktop — MVP v1 Acceptance Criteria and Traceability

**Status:** Normative verification contract

**Initial evidence state:** `NOT RUN` until a revision-specific report proves otherwise

**Last updated:** 2026-07-12

## 1. How acceptance works

Every criterion has a stable `AC-` identifier and must produce revision-specific evidence. A requirement is satisfied only when all acceptance criteria mapped to it pass in the stated environment. Documentation, code review, a screenshot, or a browser demo alone cannot prove a backend, operating-system, network, persistence, packaging, or security requirement.

Results use exactly:

- `PASS`: the specified verification ran against the identified source/artifact and met every assertion;
- `FAIL`: an assertion failed or the result is materially ambiguous;
- `NOT RUN`: verification or evidence is missing, unavailable, stale, or ran in the wrong environment.

No criterion is pre-approved by this document. A report must record source revision, build/artifact hash where applicable, environment, exact command or procedure, result, evidence path, and reviewer/job identity. Retries must retain the failed result as well as the final result.

## 2. Release classes

| Class                  | Permitted description                            | Minimum acceptance                                                                                      |
| ---------------------- | ------------------------------------------------ | ------------------------------------------------------------------------------------------------------- |
| Browser visual preview | `Demo mode UI preview`                           | Applicable component/accessibility criteria; no desktop/security claims                                 |
| Engineering MVP build  | `Windows engineering build for local evaluation` | All runtime Must product/security criteria; any production-only `NOT RUN` gates explicitly reported     |
| Production release     | `Signed production release`                      | Every in-scope Must, security, supply-chain, Windows E2E, package, SBOM, and signature criterion passes |

An engineering build cannot be described as production-ready, signed, generally distributable, or update-secure. Runtime protections for secrets, networking, rendering, IPC, storage, and privacy are mandatory for both engineering and production desktop builds.

## 3. Functional and UX acceptance scenarios

### `AC-001` — Original desktop shell and scope cues

**Given** the desktop app starts at 1280×720 and at a typical larger desktop size, **when** the main window loads with no conversation and with sample history, **then** it shows an original-branded sidebar, recent conversation area, focused work area, settings/connection state, one accessible and operable Windows title bar, and an anchored composer without clipped primary actions. Deferred Projects, Files, or Tools entries are disabled or labelled `Coming soon`, and none is represented as functional.

**Method:** Windows E2E plus visual review at 100%, 150%, and 200% display scale.

**Evidence:** screenshots, accessibility snapshot, and packaged command/capability inventory.

### `AC-002` — Conversation lifecycle

**Given** at least two conversations, **when** the user creates, selects, renames, and requests deletion of each applicable conversation, **then** creation does not overwrite the prior record, selection loads the correct messages, rename persists, deletion requires explicit confirmation, cancellation preserves the record, confirmation removes only the target and selects the documented next state.

**Method:** component, repository integration, and Windows E2E tests.

### `AC-003` — Restart persistence and recovery

**Given** complete, cancelled, and error messages in multiple conversations, **when** the desktop process exits normally and is relaunched, **then** ordering, content, titles, modes, timestamps, and terminal statuses are restored from SQLite. **Given** termination during an active stream or terminal database-write failure, **when** the application restarts, **then** the committed user message remains, no `streaming`/`stopping` value exists in SQLite, import, or export, no complete assistant answer is fabricated, and unrelated data remains intact.

**Method:** temporary-profile desktop integration/E2E plus fault injection.

### `AC-004` — Composer and keyboard behavior

**Given** focus in the composer, **when** the user enters blank text, multiline text, values at and beyond the 32,000-unit UI limit, presses `Shift+Enter`, presses `Enter`, uses an IME composition event, or submits during an active request, **then** blank/oversized/duplicate submission is blocked, newline and IME behavior are correct, a valid message submits once, and loading/disabled text states are visible and accessible. Direct IPC boundary tests separately enforce the 256-KiB backend input ceiling.

**Method:** component tests and keyboard-only Windows E2E.

### `AC-005` — Credential lifecycle without value disclosure

**Given** a clean isolated Windows credential target and unmistakably fake values below, at, and above the 8–255-byte printable-ASCII boundary, including a full 256-character native result, **when** the user opens the Rust-owned native credential prompt, confirms, replaces, checks, cancels, and removes a valid sentinel key, **then** boundary-invalid and potentially truncated input is rejected without mutation, the credential store changes correctly for valid input, cancellation is a no-op, the webview reveals only configured/not-configured state, no credential field exists in the DOM, and the sentinel never appears in frontend state/storage, any IPC argument/result/event, SQLite, errors, exports, application logs, or packaged files.

**Method:** Windows native-prompt and credential-adapter integration, command/capability inventory, DOM inspection, and automated sentinel scans. A real user credential is prohibited.

### `AC-006` — Ordered progressive streaming

**Given** a fake provider emits legal SSE across adversarial transport chunk boundaries, **when** a valid prompt is sent, **then** the backend uses `glm-5.1`, emits the accepted start event and ordered content deltas before completion, persists exactly one final assistant message, and the first delta is visible without waiting for the full response.

**Method:** provider contract/integration test and desktop E2E with a fake server.

### `AC-007` — True cancellation

**Given** active requests paused before headers, during streaming, and at the completion race, **when** Stop is activated, **then** the UI changes to stopping/cancelled, a backend cancellation token aborts network work, exactly one terminal state is persisted/emitted, and later duplicate or stale deltas cannot change any conversation.

**Method:** deterministic cancellation/race integration tests and Windows E2E; verify UI response and backend signal against `NFR-003` thresholds.

### `AC-008` — Safe Markdown and code rendering

**Given** valid Markdown plus an adversarial corpus containing raw HTML, script/event attributes, malformed nesting, SVG/MathML, embedded content, CSS, and hostile language labels, **when** an assistant response renders, **then** required headings/lists/tables/links/code are readable while forbidden markup cannot execute, navigate, register handlers, or invoke IPC.

**Method:** component security tests in the packaged CSP configuration; DOM and allowlist renderer/sanitizer assertions.

### `AC-009` — Copy behavior

**Given** a response and individual fenced code block, **when** the user copies each, **then** the clipboard receives the expected plain text exactly, success/failure feedback is non-disruptive and accessible, and no copied content executes or is automatically transmitted/saved.

**Method:** component and Windows clipboard E2E tests.

### `AC-010` — Edit and resend semantics

**Given** a conversation with a user message and subsequent descendants, **when** that user message is edited and resent after confirmation if required by the UI, **then** the original is replaced, invalid descendants are removed transactionally, the revised prompt is sent once, and unrelated earlier messages remain unchanged.

**Method:** service/repository integration and E2E tests, including injected persistence failure rollback.

### `AC-011` — Regenerate semantics

**Given** a terminal most-recent assistant response, **when** Regenerate is selected, **then** that response is replaced using preceding context without duplicating its user prompt. Invalid states such as no assistant response or an active request reject safely.

**Method:** state-machine, request contract, repository, and E2E tests.

### `AC-012` — Honest reasoning-mode contract

**Given** each selector option, **when** Rust constructs the provider request, **then** the only top-level keys are `model`, `messages`, `stream`, `thinking`, and `max_tokens`; `model` is `glm-5.1`; `stream` is `true`; and messages contain only relevant visible `user`/`assistant` content. Fast sends only `thinking.type: "disabled"` with a 4,096 output-token cap, Standard sends only `thinking.type: "enabled"` with an 8,192 cap, and Deep sends only `thinking.type: "enabled"` with a 16,384 cap. UI/help text identifies these as response profiles, does not claim Standard and Deep are different provider reasoning-effort levels, and accurately states that their difference is the output cap. No `reasoning_content` is displayed, replayed, logged, stored, or exported.

**Method:** exact request snapshot/contract tests, UI copy assertion, response/database/log/export scan. The dated documentation review in [provider-contract.md](provider-contract.md) is supporting input, not execution evidence.

### `AC-013` — Actionable failure states and bounded recovery

**Given** missing credential, `401/403`, `429` with valid/invalid retry guidance, provider `5xx`, provider content rejection, provider context limit, offline, invalid TLS, connection/idle/overall timeout, malformed/oversized SSE, database failure, and user cancellation, **when** each occurs, **then** the UI shows the correct stable safe category and action, preserves recoverable input, performs only permitted bounded retries, and exposes no raw provider/SQL/path/secret detail.

**Method:** fake adapter/server and repository fault-injection contract tests plus component assertions.

### `AC-014` — Versioned export and atomic import

**Given** a conversation containing Unicode, Markdown, all supported terminal statuses, and credential/path sentinels outside its allowed content fields, **when** exported through the Rust-owned native save dialog, **then** the output matches the exact architecture section 9 schema and contains conversation data but no source/local ID, `conversationId`, derived count, credential, configuration, internal path, header, log, or hidden reasoning. Serialization at or below 32 MiB is bounded; an output above 32 MiB is rejected before the save dialog or write. **Given** that valid export, **when** selected through the Rust-owned native open dialog and imported, **then** the 32-MiB file limit is checked before and during bounded reading, Rust generates every local ID, timestamps are normalized to UTC, and the conversation commits once. React supplies neither a path nor a serialized desktop file payload. Dialog cancellation is a safe no-op. Any invalid, oversized, unsupported, or mid-transaction failure leaves the database unchanged.

**Method:** golden serialization, schema abuse corpus, rollback integration tests, and native-dialog E2E.

### `AC-015` — External-processing and local-storage notice

**Given** a clean user profile, **when** the user attempts the first provider send, **then** a clear notice explains external processing and sensitive-data caution before transmission. The UI also states that history is stored locally without claiming app-level database encryption; browsing history causes no provider request.

**Method:** clean-profile Windows E2E with network observation and copy review.

### `AC-016` — History ordering and dismissible non-critical UI

**Given** conversations with controlled UTC update times, **when** history loads and a conversation changes, **then** sorting/grouping is deterministic and recent-first, empty/selected states are clear, and dismissing non-critical onboarding/promotion does not block or mutate the chat workflow.

**Method:** unit/component and E2E tests with a deterministic clock.

### `AC-017` — Accessibility and adaptive layout

**Given** keyboard-only input, a screen-reader accessibility inspection, reduced motion, 320 logical content pixels, 1280×720, and Windows scaling from 100% to 200%, **when** the golden path and error recovery are exercised, **then** controls have accessible names/roles/states, focus is visible and predictable, status is not color-only, contrast meets WCAG 2.2 AA, motion preference is honored, and primary controls remain operable without two-dimensional scrolling.

**Method:** automated accessibility/component checks plus Windows manual/E2E matrix.

### `AC-018` — Responsiveness and bounded large-data behavior

**Given** the specified reference fixtures and an already-started app, **when** window interaction, streaming, stop, a 100,000-character response, and 100 conversations × 200 messages are exercised, **then** `NFR-001` through `NFR-004` thresholds are measured without UI deadlock, unbounded memory growth, or lost input.

**Method:** repeatable Windows performance harness; retain raw measurements and environment specification.

## 4. Security and platform acceptance scenarios

### `AC-019` — Secret and sensitive-data non-disclosure

Inject unique sentinels as fake credentials, prompts, responses, titles, imports, provider errors, and paths across every success/failure path. Scan frontend state/storage, IPC capture, SQLite, exports, logs, crash-safe diagnostics, build output, package contents, and debug/error formatting. Each sentinel may appear only in its explicitly allowed data locations; the credential sentinel must appear only in the isolated credential adapter and fake request authorization capture.

### `AC-020` — Exact provider origin and validated TLS

Using controlled valid/invalid certificate and redirect endpoints, prove that Rust alone creates provider requests, rejects HTTP, invalid hostname/chain, cross-origin redirect, caller-supplied URL/proxy/header/model, and sends only to the configured exact Z.AI HTTPS origin/path. No certificate bypass exists in source, test-to-production configuration, or package.

### `AC-021` — Timeout and retry safety

Using a fake server, cover delayed connection, delayed headers, stalled body, transient failure, authentication failure, rate limiting, and failure before/after first delta. Assert explicit bounds, maximum attempts, jitter/retry guidance handling, no authentication retry, no replay after surfaced content, and one authoritative persisted result.

### `AC-022` — Bounded hostile SSE processing

Run boundary, property/fuzz, and adversarial fixtures at, below, and above every provider/stream bound in architecture section 7.1, including split UTF-8/lines, oversized header count/bytes, line, event, delta, transport, accumulated visible response, idle/overall time, malformed JSON, first invalid/unsupported event, provider error body, duplicate/out-of-order/cross-request sequence, abrupt EOF, and cancellation. Cover `stop`, `length`, `sensitive`, `model_context_window_exceeded`, `network_error` before/after a visible delta, and an unknown finish reason. The parser remains within the documented limits, applies the exact terminal/retry mapping, and fails closed without panic, replay after visible content, or cross-conversation mutation.

### `AC-023` — Packaged CSP and remote-content prohibition

Inspect and run the packaged desktop UI. Assert the effective CSP has no `unsafe-eval` or arbitrary remote active-content origin; scripts, styles, fonts, frames, plugins, analytics, and runtime executable code are local; no remote request occurs during offline navigation/rendering; and production source maps/build metadata contain no secret or sensitive path.

### `AC-024` — Link and navigation protocol policy

Render a protocol/encoding corpus including HTTPS, approved HTTP policy if supported, `javascript:`, `data:`, `file:`, UNC, shell, encoded controls, mixed case, custom/Tauri schemes, relative URLs, deceptive labels, and opener attacks. Only documented approved protocols open through safe external navigation; rejected content cannot navigate the webview or invoke commands.

### `AC-025` — Least-privilege IPC and capabilities

Inventory source and packaged Tauri commands, events, windows, scopes, capabilities, plugins, and protocols. Assert every capability maps to an in-scope requirement, only the main window invokes privileged commands, every application-defined call uses a raw UTF-8 JSON byte body, the 320-KiB check precedes JSON parsing, pre-deserialized JSON/raw oversize/malformed/deep/unknown/wrong-type/unregistered/wrong-window application calls fail with a stable safe error, framework-owned calls are limited to the exact documented window/event actions, and no shell/process/arbitrary filesystem/arbitrary URL/project/file/tool capability exists.

### `AC-026` — Storage integrity and safe migration

Against temporary databases, exercise parameterized injection corpora, foreign-key behavior, size bounds, concurrent state operations, schema upgrade from every supported prior version, forced migration/write/finalization failure, corruption detection, and restart recovery. No secret/header/hidden reasoning enters SQLite; failed operations roll back without silent database replacement.

### `AC-027` — Hostile import rejection

Run a maintained corpus at, below, and above the architecture section 7.1 import bounds, including the 32-MiB pre-read/during-read ceiling, conversation/message counts, field sizes, and the 8-container nesting limit before typed deserialization. Assert the exact architecture section 9 shape and include unknown/trailing fields, source/local IDs, `conversationId`, derived counts, incompatible version, invalid encoding/JSON/types/enums/timestamps, unsupported model/roles/status, invalid role/status combinations, excessive token usage, script/HTML/paths/configuration/credential fields, disallowed controls, decompression-like expansion if applicable, and database fault. Each rejects with a bounded safe error and leaves the database byte/logically unchanged.

### `AC-028` — Scoped file access and export minimization

Exercise Rust-owned native-dialog cancel, allowed selection, hostile renderer attempts to provide a path or serialized file, traversal names, reserved Windows names, long paths, symlink/reparse scenarios where applicable, and denied permissions. Assert the app reads/writes only the selected scoped file, never constructs a path from conversation content, never accepts a renderer path, warns before plaintext export, and emits only the versioned minimum schema. Browser-demo picker/download tests remain out of scope for this result.

### `AC-029` — Privacy-preserving diagnostics and network silence

Across the golden path and every injected error, assert Aster creates no application diagnostic log, crash-report upload, telemetry, or analytics endpoint; IPC errors contain exactly the allowlisted bounded fields; sentinel content is absent from errors and package files; and only explicit provider sends produce external application network traffic.

### `AC-030` — Dependency and CI assurance

For the release revision, retain lockfile review, formatter/type/lint/test/clippy/static-analysis results, secret scan, dependency vulnerability/license/policy reports, and an SBOM covering shipped JavaScript, Rust, and native components. Inventory every workflow `uses:` reference, prove that each is a reviewed immutable 40-character commit SHA, verify that its action runtime is supported by the hosted runner, and review all workflow annotations. Any exploitable high/critical finding either blocks release or links to a valid unexpired exception record.

### `AC-031` — Installer, update, and provenance integrity

For a production release, record source revision, build identity, artifact hashes, SBOM, provenance, and a valid authorized Windows signature verified after packaging; mutate a copy and demonstrate verification failure. Assert automatic updates are disabled unless separately signed metadata/artifact positive, tamper-negative, origin, and rollback tests all pass. Missing signer/evidence is `NOT RUN` and limits the artifact to an engineering build.

### `AC-032` — Browser demo isolation

Build/run the browser profile and inspect UI, bundle, storage, and network. It visibly says `Demo mode`, has no credential control/API/key placeholder, never calls Z.AI or desktop IPC, uses only in-memory sample or explicitly user-selected demo-import state, loses state on reload, and is not used as evidence for desktop persistence/security.

### `AC-033` — Clean package inventory

Inventory the installed/package contents, source archive, and effective configuration. Assert there are no real secrets, private test conversations, development/fake provider endpoints reachable in production, debug/certificate/policy bypasses, writable runtime scripts, unexpected executables, dormant privileged handlers, remote executable fetches, or enabled auto-update without `AC-031` evidence. For a shared source archive, create ignored and untracked sentinels before packaging, prove they are absent, and prove the archive inventory equals the tracked inventory of the recorded clean commit rather than a directory-wide denylist result.

### `AC-034` — Exception and incident readiness

Review every active exception for affected IDs, owner, reason, threat/impact, compensating control, expiry, and removal work. Conduct or retain a credential-exposure tabletop showing containment, out-of-band rotation, artifact/log assessment, remediation, regression test, and threat-model update without putting the key in an issue/log.

### `AC-035` — Clean-profile Windows 11 end-to-end

On a supported 64-bit Windows 11 clean user profile, install/launch the exact candidate, complete credential setup with an isolated test target, exercise the golden chat path against a controlled provider, restart, stop, edit, regenerate, import/export, remove the credential, and uninstall. Record WebView2 behavior, application data retained/removed by documented policy, crashes, accessibility smoke results, artifact hash, and environment.

### `AC-036` — Local conversation-title filter

**Given** loaded conversation summaries with matching, non-matching, mixed-case, Unicode, and empty titles, **when** the user enters, edits, clears, and keyboard-navigates the sidebar filter, **then** matching titles are shown in their existing deterministic recent-first order, a specific no-match state appears when appropriate, the selected conversation remains deterministic, and clearing restores the list. The filter operates on already loaded summary titles, does not inspect message bodies or persist/log the query, and causes no provider request.

**Method:** unit/component keyboard test with deterministic fixtures plus network, storage, and log-spy negative assertions.

## 5. Product traceability matrix

| Requirement | Acceptance criteria                              |
| ----------- | ------------------------------------------------ |
| `PR-001`    | `AC-001`, `AC-017`, `AC-035`, `AC-036`           |
| `PR-002`    | `AC-005`, `AC-020`, `AC-025`, `AC-026`           |
| `PR-003`    | `AC-002`–`AC-015`, `AC-035`                      |
| `FR-001`    | `AC-001`, `AC-017`                               |
| `FR-002`    | `AC-002`                                         |
| `FR-003`    | `AC-002`                                         |
| `FR-004`    | `AC-003`, `AC-026`                               |
| `FR-005`    | `AC-004`                                         |
| `FR-006`    | `AC-005`, `AC-019`                               |
| `FR-007`    | `AC-006`, `AC-020`, `AC-022`                     |
| `FR-008`    | `AC-007`, `AC-022`                               |
| `FR-009`    | `AC-008`, `AC-023`, `AC-024`                     |
| `FR-010`    | `AC-009`                                         |
| `FR-011`    | `AC-010`, `AC-026`                               |
| `FR-012`    | `AC-011`                                         |
| `FR-013`    | `AC-012`                                         |
| `FR-014`    | `AC-013`, `AC-020`–`AC-022`                      |
| `FR-015`    | `AC-014`, `AC-027`, `AC-028`                     |
| `FR-016`    | `AC-015`                                         |
| `FR-017`    | `AC-016`                                         |
| `FR-018`    | `AC-016`                                         |
| `FR-019`    | `AC-036`                                         |
| `UX-001`    | `AC-004`, `AC-017`, `AC-036`                     |
| `UX-002`    | `AC-004`                                         |
| `UX-003`    | `AC-002`, `AC-010`, `AC-017`, `AC-036`           |
| `UX-004`    | `AC-013`, `AC-017`                               |
| `UX-005`    | `AC-001`, `AC-017`                               |
| `UX-006`    | `AC-017`                                         |
| `UX-007`    | `AC-017`                                         |
| `UX-008`    | `AC-002`, `AC-015`, `AC-017`                     |
| `NFR-001`   | `AC-018`                                         |
| `NFR-002`   | `AC-006`, `AC-018`                               |
| `NFR-003`   | `AC-007`, `AC-018`                               |
| `NFR-004`   | `AC-018`, `AC-022`                               |
| `NFR-005`   | `AC-003`, `AC-010`, `AC-014`, `AC-026`, `AC-027` |
| `NFR-006`   | `AC-013`, `AC-027`                               |
| `NFR-007`   | `AC-035`                                         |

## 6. Security traceability matrix

| Security requirement | Acceptance criteria                                   |
| -------------------- | ----------------------------------------------------- |
| `SEC-001`            | `AC-005`, `AC-019`                                    |
| `SEC-002`            | `AC-005`, `AC-019`, `AC-025`                          |
| `SEC-003`            | `AC-005`, `AC-019`, `AC-029`                          |
| `SEC-004`            | `AC-005`, `AC-035`                                    |
| `SEC-005`            | `AC-032`                                              |
| `SEC-006`            | `AC-006`, `AC-020`, `AC-025`                          |
| `SEC-007`            | `AC-013`, `AC-020`, `AC-033`                          |
| `SEC-008`            | `AC-013`, `AC-021`                                    |
| `SEC-009`            | `AC-006`, `AC-012`, `AC-019`                          |
| `SEC-010`            | `AC-022`                                              |
| `SEC-011`            | `AC-007`, `AC-022`                                    |
| `SEC-012`            | `AC-008`, `AC-023`, `AC-024`                          |
| `SEC-013`            | `AC-023`, `AC-029`, `AC-033`                          |
| `SEC-014`            | `AC-008`, `AC-023`, `AC-024`                          |
| `SEC-015`            | `AC-024`                                              |
| `SEC-016`            | `AC-009`                                              |
| `SEC-017`            | `AC-025`, `AC-033`                                    |
| `SEC-018`            | `AC-004`, `AC-013`, `AC-025`                          |
| `SEC-019`            | `AC-007`, `AC-010`, `AC-011`, `AC-022`, `AC-025`      |
| `SEC-020`            | `AC-013`, `AC-019`, `AC-025`, `AC-029`                |
| `SEC-021`            | `AC-001`, `AC-025`, `AC-033`                          |
| `SEC-022`            | `AC-003`, `AC-010`, `AC-019`, `AC-026`                |
| `SEC-023`            | `AC-003`, `AC-026`, `AC-035`                          |
| `SEC-024`            | `AC-014`, `AC-027`                                    |
| `SEC-025`            | `AC-014`, `AC-026`, `AC-027`                          |
| `SEC-026`            | `AC-014`, `AC-028`                                    |
| `SEC-027`            | `AC-014`, `AC-019`, `AC-028`                          |
| `SEC-028`            | `AC-002`, `AC-026`                                    |
| `SEC-029`            | `AC-015`, `AC-029`, `AC-035`                          |
| `SEC-030`            | `AC-019`, `AC-020`, `AC-029`, `AC-036`                |
| `SEC-031`            | `AC-019`, `AC-023`, `AC-029`, `AC-032`, `AC-036`      |
| `SEC-032`            | `AC-002`, `AC-014`, `AC-015`                          |
| `SEC-033`            | `AC-030`, `AC-033`                                    |
| `SEC-034`            | `AC-030`                                              |
| `SEC-035`            | `AC-030`, `AC-031`                                    |
| `SEC-036`            | `AC-030`, `AC-034`                                    |
| `SEC-037`            | `AC-031`, `AC-035`                                    |
| `SEC-038`            | `AC-031`, `AC-033`                                    |
| `SEC-039`            | `AC-019`, `AC-023`, `AC-032`, `AC-033`                |
| `SEC-040`            | `AC-020`–`AC-028` as applicable to the fixed boundary |
| `SEC-041`            | `AC-030`, `AC-034`                                    |
| `SEC-042`            | `AC-034`                                              |

## 7. Threat-to-evidence traceability

| Threats           | Primary acceptance criteria                                          |
| ----------------- | -------------------------------------------------------------------- |
| `TM-001`–`TM-006` | `AC-005`, `AC-015`, `AC-019`, `AC-029`, `AC-032`, `AC-033`, `AC-036` |
| `TM-007`–`TM-011` | `AC-008`, `AC-009`, `AC-023`, `AC-024`, `AC-025`                     |
| `TM-012`–`TM-017` | `AC-007`, `AC-013`, `AC-021`, `AC-022`, `AC-025`                     |
| `TM-018`–`TM-021` | `AC-012`, `AC-015`, `AC-020`, `AC-029`                               |
| `TM-022`–`TM-028` | `AC-002`, `AC-003`, `AC-010`, `AC-014`, `AC-026`–`AC-028`            |
| `TM-029`–`TM-034` | `AC-019`, `AC-023`, `AC-029`–`AC-034`, `AC-036`                      |
| `TM-035`          | `AC-005`, `AC-019`, `AC-025`, `AC-035`                               |
| `TM-036`          | `AC-019`, `AC-030`, `AC-031`, `AC-033`                               |

## 8. Release evidence checklist

A release report must state each item, not merely say “all checks passed”:

- source revision and clean/dirty state;
- version and exact artifact hashes;
- requirements and threats changed since the previous baseline;
- unit, component, integration, contract, security, and E2E result summaries;
- Windows version, architecture, WebView2 version, display-scale/accessibility matrix;
- frontend format/type/lint/test and Rust format/clippy/test results;
- secret, static, dependency vulnerability/license/policy scan results;
- SBOM location and coverage;
- CSP, Tauri capability, bundle, remote-origin, and package inventory results;
- migration and import-abuse result summaries;
- signature status and post-package verification;
- update status (`disabled` is acceptable for v1; `enabled` requires full evidence);
- active exceptions, residual risks, failures, and `NOT RUN` gates;
- final classification: browser preview, engineering MVP build, or production release.

## 9. Definition of MVP acceptance

The MVP outcome is accepted only when:

1. Every `Must` product requirement maps to at least one criterion and every mapped criterion required for the claimed release class passes.
2. Every security requirement applicable to that class has evidence; no runtime security control is deferred.
3. All Critical/High threats have implemented and passing mapped controls, except explicitly stated inherent external residuals such as provider-side handling.
4. The application has no hidden capability beyond the documented v1 boundary.
5. Required failures and `NOT RUN` results block the corresponding release claim.
6. The Definition of Done in [AGENTS.md](../AGENTS.md) is complete and the release report accurately states residual risk.
