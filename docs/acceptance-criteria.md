# Aster Desktop — MVP v2 Acceptance Criteria and Traceability

**Status:** Normative verification contract

**Initial evidence state:** `NOT RUN` until a revision-specific report proves otherwise

**Last updated:** 2026-07-13

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

**Given** the desktop app starts at 1280×720 and at a typical larger size, **when** the main window loads with no conversation and with history, **then** it shows an original-branded sidebar, recent conversations, work area, provider/model/profile/connection state, Usage access, one accessible Windows title bar, and anchored composer without clipped actions. Deferred Projects, Files, or Tools are disabled or labelled `Coming soon` and have no backend capability.

**Method:** Windows E2E plus visual review at 100%, 150%, and 200% display scale.

**Evidence:** screenshots, accessibility snapshot, and packaged command/capability inventory.

### `AC-002` — Conversation lifecycle

**Given** at least two conversations, **when** the user creates, selects, renames, and requests deletion of each applicable conversation, **then** creation does not overwrite the prior record, selection loads the correct messages, rename persists, deletion requires explicit confirmation, cancellation preserves the record, confirmation removes only the target and selects the documented next state.

**Method:** component, repository integration, and Windows E2E tests.

### `AC-003` — Restart persistence and recovery

**Given** complete, cancelled, and error messages across catalog pairs plus usage observations and budgets, **when** the process relaunches, **then** ordering, content, titles, pinned provider/model/profile, nullable usage, budgets, timestamps, and terminal statuses restore from SQLite. Usage exists only on complete assistant messages. Directly tampered message roles/statuses/token fields and usage-ledger identities/timestamps/pairs/arithmetic/partial states fail closed at the Rust read boundary even when SQLite constraints are bypassed. **Given** termination during an active stream or database-write failure, **when** restart occurs, **then** committed state remains consistent, no ephemeral status exists in SQLite/import/export, no answer or usage total is fabricated, and unrelated data remains intact.

**Method:** temporary-profile desktop integration/E2E plus fault injection.

### `AC-004` — Composer and keyboard behavior

**Given** focus in the composer, **when** the user enters blank text, multiline text, values at and beyond the 32,000-unit UI limit, presses `Shift+Enter`, presses `Enter`, uses an IME composition event, or submits during an active request, **then** blank/oversized/duplicate submission is blocked, newline and IME behavior are correct, a valid message submits once, and loading/disabled text states are visible and accessible. Direct IPC boundary tests separately enforce the 256-KiB backend input ceiling.

**Method:** component tests and keyboard-only Windows E2E.

### `AC-005` — Credential lifecycle without value disclosure

**Given** isolated targets for every catalog provider and fake values below/at/above the 8–255-byte boundary, including a full 256-character native result, **when** each provider prompt confirms, replaces, checks, cancels, and removes a sentinel key, **then** invalid/truncated input is rejected, only the matching target changes, cancellation is a no-op, the webview reveals status only, no credential field exists, and no sentinel appears in frontend storage, secret-bearing IPC, SQLite, errors, exports, logs, another provider request, or package.

**Method:** Windows native-prompt and credential-adapter integration, command/capability inventory, DOM inspection, and automated sentinel scans. A real user credential is prohibited.

### `AC-006` — Ordered progressive streaming

**Given** a fake provider emits legal SSE across adversarial transport chunk boundaries, including more than 4,096 tiny valid deltas and the 65,536-event boundary while remaining within every byte/content bound, **when** a valid prompt is sent, **then** the backend emits the accepted start event and ordered content deltas before completion, persists exactly one final assistant message, and the first delta is visible without waiting for the full response. Pre-command buffering and post-command processing accept the same legal event cardinality; event 65,537 fails closed and cannot revive or cross a request.

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

### `AC-012` — Z.AI `glm-5.1` profile regression contract

**Given** each selector option, **when** Rust constructs the provider request, **then** the only top-level keys are `model`, `messages`, `stream`, `thinking`, and `max_tokens`; `model` is `glm-5.1`; `stream` is `true`; and messages contain only relevant visible `user`/`assistant` content. Fast sends only `thinking.type: "disabled"` with a 4,096 output-token cap, Standard sends only `thinking.type: "enabled"` with an 8,192 cap, and Deep sends only `thinking.type: "enabled"` with a 16,384 cap. UI/help text identifies these as response profiles, does not claim Standard and Deep are different provider reasoning-effort levels, and accurately states that their difference is the output cap. No `reasoning_content` is displayed, replayed, logged, stored, or exported.

**Method:** exact request snapshot/contract tests, UI copy assertion, response/database/log/export scan. The dated documentation review in [provider-contract.md](provider-contract.md) is supporting input, not execution evidence.

### `AC-013` — Actionable failure states and bounded recovery

**Given** missing credential, `401/403`, `429`, provider `5xx`, content/context rejection, offline, invalid TLS, connection/idle/overall timeout, malformed/oversized stream, unsupported profile, balance failure, database failure, and cancellation, **when** each occurs, **then** the UI shows the correct stable safe category/action, preserves recoverable input, performs no automatic provider retry, and exposes no raw provider/SQL/path/secret detail.

**Method:** fake adapter/server and repository fault-injection contract tests plus component assertions.

### `AC-014` — Versioned export and atomic import

**Given** version 1 and version 2 fixtures with Unicode, Markdown, supported statuses, provider/model/profile, nullable disjoint usage, and sentinels, **when** export/import uses Rust-owned dialogs, **then** export emits the exact architecture section 10 version 2 schema and import maps exact v1 to `zai`/`glm-5.1` or accepts a catalog-valid v2 pair. It includes no source/local ID, aggregate usage, budget, balance, credential, configuration, path, header, log, or hidden reasoning. Bounds, safe integers, usage invariants, UTC normalization, generated IDs, cancellation, and atomic rollback are enforced; React supplies neither path nor desktop file payload.

**Method:** golden serialization, schema abuse corpus, rollback integration tests, and native-dialog E2E.

### `AC-015` — External-processing and local-storage notice

**Given** a clean profile and each provider disclosure version, **when** the user attempts a send, **then** a notice names the selected external provider, includes Alibaba Cloud US when applicable, and warns about sensitive data before transmission. Acceptance persists only for that provider/version; increasing its catalog `noticeVersion` prompts again, and another provider is not covered. The UI states that history is local without claiming app encryption; browsing history or local Usage causes no provider request.

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

### `AC-020` — Z.AI origin regression and validated TLS

Using controlled valid/invalid certificate and redirect endpoints, prove the `zai` adapter alone sends to its exact HTTPS origin/path and rejects HTTP, invalid hostname/chain, cross-origin redirect, and caller URL/proxy/header/model. Catalog-wide origin evidence is `AC-043`. No certificate bypass exists in source, test-to-production configuration, or package.

### `AC-021` — Timeout and no-automatic-retry safety

Using fake servers, cover delayed connection/headers/body, transient/authentication/rate-limit failure, and failure before/after first delta. Assert explicit bounds and exactly one provider attempt for every result. No automatic retry, backoff, replay, or hidden second billable request occurs; an explicit user retry is a new operation and coverage marker.

### `AC-022` — Bounded hostile SSE processing

Run boundary, property/fuzz, and adversarial fixtures at, below, and above architecture section 8 bounds for every adapter, including split UTF-8/lines, headers, line/event/delta/transport/accumulated size, idle/overall time, malformed JSON, first invalid event, provider error body, duplicate/out-of-order/cross-request sequence, abrupt EOF, and cancellation. Exercise every exact provider-contract section 6 finish/block value and an unknown value; `[DONE]` requirements for Z.AI/DeepSeek/Alibaba/NVIDIA; Google terminal EOF without `[DONE]`; Alibaba valid answer with missing usage as partial; tool/function events as unsupported; and hidden-reasoning discard. Each parser fails closed without panic, automatic retry, replay, or cross-conversation mutation.

### `AC-023` — Packaged CSP and remote-content prohibition

Inspect and run the packaged desktop UI. Assert the effective CSP has no `unsafe-eval` or arbitrary remote active-content origin; scripts, styles, fonts, frames, plugins, analytics, and runtime executable code are local; no remote request occurs during offline navigation/rendering; and production source maps/build metadata contain no secret or sensitive path.

### `AC-024` — Link and navigation protocol policy

Render a protocol/encoding corpus including HTTPS, approved HTTP policy if supported, `javascript:`, `data:`, `file:`, UNC, shell, encoded controls, mixed case, custom/Tauri schemes, relative URLs, deceptive labels, and opener attacks. Only documented approved protocols open through safe external navigation; rejected content cannot navigate the webview or invoke commands.

### `AC-025` — Least-privilege IPC and capabilities

Inventory source and packaged Tauri commands, events, windows, scopes, capabilities, plugins, and protocols. Assert every capability maps to an in-scope requirement, only the main window invokes privileged commands, every application-defined call uses a raw UTF-8 JSON byte body, the 320-KiB check precedes JSON parsing, pre-deserialized JSON/raw oversize/malformed/deep/unknown/wrong-type/unregistered/wrong-window application calls fail with a stable safe error, framework-owned calls are limited to the exact documented window/event actions, and no shell/process/arbitrary filesystem/arbitrary URL/project/file/tool capability exists.

### `AC-026` — Storage integrity and safe migration

Against temporary WAL databases, exercise parameterized injection, foreign keys, bounds, concurrent state, v1→v2 migration, forced backup/migration/write/finalization failure, corruption, and restart. Prove the SQLite backup API creates a WAL-consistent deterministic sibling with current-user ACL and verified integrity/schema/source identity; `fs::copy` is absent; only a matching verified v1 backup is reused; a conflict blocks; failure retains recovery; verified v2 commit permits deletion; only a matching verified orphan is cleaned. No secret/header/balance/hidden reasoning enters SQLite and failure never replaces data silently.

### `AC-027` — Hostile import rejection

Run a maintained corpus at, below, and above architecture section 8 import bounds and section 10 exact v1/v2 shapes, including 32-MiB pre/during-read, counts, field sizes, nesting, unknown/trailing fields, IDs, `conversationId`, derived counts, invalid version/encoding/JSON/types/enums/timestamps, unsupported provider/model/profile/role/status, invalid role/status combinations, unsafe or inconsistent usage, aggregate ledger/budget/balance, scripts/HTML/paths/configuration/credentials, controls, and database fault. Each rejects with a bounded safe error and leaves the database byte/logically unchanged.

### `AC-028` — Scoped file access and export minimization

Exercise Rust-owned native-dialog cancel, allowed selection, hostile renderer attempts to provide a path or serialized file, traversal names, reserved Windows names, long paths, symlink/reparse scenarios where applicable, and denied permissions. Assert the app reads/writes only the selected scoped file, never constructs a path from conversation content, never accepts a renderer path, warns before plaintext export, and emits only the versioned minimum schema. Browser-demo picker/download tests remain out of scope for this result.

### `AC-029` — Privacy-preserving diagnostics and network silence

Across the golden path and every injected error, assert Aster creates no application diagnostic log, crash-report upload, telemetry, or analytics endpoint; IPC errors contain exactly the allowlisted bounded fields; sentinel content is absent from errors and package files; and only explicit provider sends produce external application network traffic.

### `AC-030` — Dependency and CI assurance

For the release revision, retain lockfile review, formatter/type/lint/test/clippy/static-analysis results, secret scan, dependency vulnerability/license/policy reports, and an SBOM covering shipped JavaScript, Rust, and native components. Inventory every workflow `uses:` reference, prove that each is a reviewed immutable 40-character commit SHA, verify that its action runtime is supported by the hosted runner, and verify that tools installed during CI name an exact reviewed version and use upstream lock metadata when available. Review all workflow annotations. Any exploitable high/critical finding either blocks release or links to a valid unexpired exception record.

### `AC-031` — Installer, update, and provenance integrity

Start an engineering build only from a clean identified commit and prove it emits a strict identity containing that exact revision plus SHA-256 identities for the executable, installer, both SBOMs, and ordinal deterministic production-frontend tree. Execute temporary-repository positive and abuse fixtures. Before generic JSON conversion or copy, reject duplicate/extra manifest fields, wrong/string/fractional token types, unsafe-range byte counts, impossible timestamps, wrong paths/hashes/revision, and a noncanonical schema. Packaging must reject protected-path reparse points; require the full revision in the ignored evidence filename, exactly one canonical revision field, and exactly one canonical `Unsigned engineering build for local evaluation` classification; duplicate, contradictory, engineering-MVP, and production classifications must fail. Enforce the record's 256 KiB strict-UTF-8 limit, a non-empty strict-UTF-8 sibling log no larger than 4 MiB, real ordered UTC window, self-declared procedure identity, one current row per gate, exact per-result source/environment/UTC-window/identity plus command/evidence/artifact/scope fields, a canonical sibling-log locator for every `PASS`, rejection of every malformed or indented table row, shared-secret scan, Markdown-aware protected-header scan, and slash/JSON-escape-normalized personal-path scan across both evidence inputs; and reject stale/tampered identities or a changed source. It must scan binary candidates for UTF-16LE/BE credentials at offsets zero and one; assemble into fresh contained non-reparse staging; copy the retained log; validate copied identities and controlled source; archive the exact revision; compare every ZIP file, exact safe parent-directory entry, external Git mode, and blob with the emitted source inventory; recheck final clean `HEAD`, allowlist, and checksums while the candidate still has a random non-final name; and publish only to a previously absent ignored same-volume output. The same-volume directory rename must be the sole publication operation and the last fallible action that can grant the final handoff name; a failure may leave the candidate only under its ignored staging name, and cleanup must refuse reparse points. The verifier label is self-declared. An unavailable PowerShell Authenticode verifier is recorded exactly as `NOT RUN (verifier unavailable)` in the engineering identity table; this observation does not block an engineering package and never counts as production signature evidence. For a production release, additionally retain authorized provenance and a valid Windows signature verified after packaging; mutate a copy and demonstrate verification failure. Assert automatic updates are disabled unless separately signed metadata/artifact positive, tamper-negative, origin, and rollback tests all pass. Missing signer/evidence is `NOT RUN` and limits the artifact to an engineering build.

### `AC-032` — Browser demo isolation

Build/run browser demo and inspect UI, bundle, storage, and network. It says `Demo mode`, has the synthetic catalog/streams/usage/balance/stale states, no credential control/key placeholder, never calls any provider, balance, account action, or desktop IPC, uses memory only, loses state on reload, and is not evidence for desktop persistence/security.

### `AC-033` — Clean package inventory

Inventory the installed/package contents, source archive, source-inventory file, build-identity manifest, retained verification log, and effective configuration. Assert there are no real secrets, private test conversations, development/fake provider endpoints reachable in production, debug/certificate/policy bypasses, writable runtime scripts, unexpected executables, dormant privileged handlers, remote executable fetches, or enabled auto-update without `AC-031` evidence. Before packaging, place an unexpected private sentinel in the ignored output directory and prove the fresh-output policy rejects it without hashing or copying it. After removal, prove the staged inventory equals the exact final allowlist, checksums cover that allowlist, every source ZIP file and exact safe parent directory has the tracked path and external Git mode, each blob equals the ordinal tracked inventory of the identity's clean commit, and ignored/untracked workspace sentinels are absent rather than filtered by a directory-wide denylist. Immediately before the final rename, race publication with a destination junction and, separately, hold a staged child open. Prove the external sentinel remains unchanged, neither candidate receives the final output name, and any cleanup remains confined to its ignored non-final staging path.

### `AC-034` — Exception and incident readiness

Review every active exception for affected IDs, owner, reason, threat/impact, compensating control, expiry, and removal work. Conduct or retain a credential-exposure tabletop showing containment, out-of-band rotation, artifact/log assessment, remediation, regression test, and threat-model update without putting the key in an issue/log.

### `AC-035` — Clean-profile Windows 11 end-to-end

On a supported 64-bit Windows 11 clean user profile, install/launch the exact candidate, complete credential setup with an isolated test target, exercise the golden chat path against a controlled provider, restart, stop, edit, regenerate, import/export, remove the credential, and uninstall. Record WebView2 behavior, application data retained/removed by documented policy, crashes, accessibility smoke results, artifact hash, and environment.

### `AC-036` — Local conversation-title filter

**Given** loaded conversation summaries with matching, non-matching, mixed-case, Unicode, and empty titles, **when** the user enters, edits, clears, and keyboard-navigates the sidebar filter, **then** matching titles are shown in their existing deterministic recent-first order, a specific no-match state appears when appropriate, the selected conversation remains deterministic, and clearing restores the list. The filter operates on already loaded summary titles, does not inspect message bodies or persist/log the query, and causes no provider request.

**Method:** unit/component keyboard test with deterministic fixtures plus network, storage, and log-spy negative assertions.

### `AC-037` — Apache license and attribution consistency

For an identified clean revision, compare root `LICENSE` byte-for-byte with the canonical Apache License 2.0 text; inspect `NOTICE` for all required project and third-party attribution; and prove that JavaScript, Rust, Tauri bundle, source-archive, and installed-package metadata consistently declare and carry Apache-2.0 plus `LICENSE` and `NOTICE`. Inventory separately licensed material and verify its provenance, terms, attribution, and modification status remain explicit. Controlled fixtures with a changed license text, missing notice, stripped attribution, or conflicting metadata must fail the policy gate before packaging.

### `AC-038` — SBOM component-license inventory

Generate the frontend and Rust SBOMs from the locked identified revision. Validate that each Aster root component has exactly the Apache-2.0 expression, every enumerated workspace and third-party component has a non-empty license expression derived from the corresponding package or Cargo metadata, and component name/version coverage reconciles with that metadata. Missing, unknown, and conflicting-license fixtures must fail. Record the generators' manifest-derived scope and do not use this result alone as binary redistribution-compliance evidence.

### `AC-039` — Public binary redistribution compliance

For the exact binary and installer candidate, reconcile the final package and installed-file inventory with the frontend, Rust, native, and bundled-component inventory; collect and review every applicable third-party copyright notice, attribution, and license text; prove the candidate contains those materials plus root `LICENSE` and `NOTICE`; and record the source revision and artifact hashes. If any component scope, required material, review, or package inspection is missing, record `NOT RUN` and do not distribute the binary or installer publicly. Source-repository publication is evaluated separately by `AC-037`.

### `AC-040` — Curated catalog, default, and capability truth

Generate a golden snapshot from Rust and compare it with the product specification, provider contract, React-visible DTO, browser-demo synthetic catalog, and packaged strings. It contains exactly 17 selectable entries: four `zai`, two `deepseek`, six `alibaba-us`, three `google`, and two `nvidia` pairs from product-spec section 4.1; the only default is `zai`/`glm-5.1`; NVIDIA entries say `Hosted prototype`; Alibaba says `Alibaba Cloud (US)`; and each provider carries an explicit `noticeVersion` plus only its supported profile/account-action metadata. No unavailable entry/status, arbitrary provider/model, omitted requested placeholder, remote discovery, endpoint, key, or account URL crosses to React. Unknown, case-changed, aliased, or injected pairs/defaults fail before persistence or network.

**Method:** Rust/TypeScript catalog snapshot and schema tests, package/source inventory, official-source review dated to the revision, hostile IPC/import corpus, and browser network/storage spies.

### `AC-041` — Provider-scoped native credential isolation

Using five isolated fake credential targets and a distinct sentinel for each provider, exercise native prompt success, replace, cancel, invalid/truncated input, concurrent prompt, status, read-for-request, remove, and adapter failure. Rust derives prompt copy/target from an allowlisted provider ID; only the selected target changes; the correct sentinel appears only in its matching fake authorization capture; cancellation and another provider remain unchanged; and no key/target/value appears in React, IPC, SQLite, exports, logs, errors, or package. Unknown provider IDs fail before the prompt or target access.

**Method:** unit/static boundary tests plus isolated Windows native prompt/credential-store integration and packaged command inventory. Real user credentials are prohibited.

### `AC-042` — Deterministic new chat and immutable conversation pair

Given no selected conversation, New chat uses Rust's `zai`/`glm-5.1` default. Given a fully loaded current conversation whose ID equals the selected ID, New chat inherits its pair. New chat and Ctrl+N are disabled while a selected conversation is loading or its empty pair mutation is pending. Selecting a model from the no-conversation composer creates that pair without clearing the existing draft. An empty conversation can change to every catalog pair transactionally. While navigation or an empty-conversation model mutation is pending, send/edit/regenerate/model-mutation controls have no authority over the prior conversation. A later navigation invalidates the earlier load, create-selection result, or mutation; a delayed completion cannot overwrite the newly selected conversation, clear its draft, or cause a send under the stale pair. A backend-created conversation from a stale create result may remain in the summary list but cannot steal selection. A background conversation's terminal reconciliation cannot invalidate or wedge the visible conversation load. After its first message is persisted, direct IPC/import/database attempts to mutate provider/model fail with `CONVERSATION_MODEL_LOCKED`, create no network/usage marker, and leave history unchanged. The UI confirmation names current/requested pairs; Cancel changes nothing; `Start a new chat with this model` creates a distinct empty conversation. Send, edit, resend, regenerate, credential, events, usage, export, and restart remain pinned, including crossed/stale-event races.

**Method:** state-machine, repository constraint/migration/import, component focus/keyboard, crossed-event integration, and Windows E2E tests.

### `AC-043` — Exact per-provider request, profile, stream, and disclosure contracts

For every catalog pair and Fast/Standard/Deep profile, execute the exact provider-contract section 5/6 fixture against the matching fake adapter. Assert the fixed method/origin/path/region/auth; exact request keys and caps; per-model thinking mapping; provider-specific SSE termination (`[DONE]` for Z.AI, DeepSeek, Alibaba, and NVIDIA; clean terminal HTTP EOF for Google); exact finish/block allowlists; hidden-reasoning discard; usage equations; one attempt; TLS/redirect rejection; and no fallback. Alibaba success requires terminal choice plus `[DONE]`; its missing usage chunk yields partial coverage without invalidating a valid answer. Google never requires `[DONE]`. For every adapter, normal completion persists/emits `stop`, output-cap completion preserves partial content and valid usage while persisting/emitting `outputLimit`, and the UI shows the adjacent accessible incomplete-response notice only for `outputLimit`. Migrated or imported complete assistant messages without authoritative terminal evidence use `unknown`; user/cancelled/error messages omit the field. Export/import round-trips the typed value without inferring a normal stop. Early, duplicate, or out-of-order terminal/usage claims and unknown capability-bearing fields fail closed. A provider disclosure blocks the first send for each `noticeVersion`, persists only that provider/version after acknowledgement, and reappears when the version increases. Unknown fields/profiles/finish values, wrong endpoint/key/region, malformed content, or a disclosure mismatch fail safely.

**Method:** exact request snapshots, fake HTTPS/SSE servers, parser/property fixtures, authorization capture, UI copy/component tests, and per-provider/version network-observed E2E. No billable endpoint is called by default.

### `AC-044` — Usage coverage, normalization, and seven-day integrity

With a fixed clock, run successful, cancelled, connection-failed, provider-failed, timed-out, malformed-stream, missing-usage, malformed-usage, and pre-network-rejected sends. Exactly one partial marker is created immediately before networking; authoritative final usage fills that marker once; cancellation/failure/missing/malformed usage leaves nullable partial coverage; and pre-network rejection creates none. Exercise every provider equation, cache-inclusive prompt subtraction, reasoning/thought subsets, absent optional NVIDIA usage, negative/fractional/inconsistent/overflow values, duplicate terminal/usage frames, explicit retry, deletion, and trailing-seven-day boundaries. Counts are disjoint, safe integers, idempotent, and scoped by provider/optional model. Directly insert malformed canonical IDs, UTC times, pairs, tokens, arithmetic, and partial flags with check constraints bypassed and prove aggregation returns a safe integrity failure before time filtering. Explicit retry is a new operation; there is no automatic retry. Conversation deletion does not erase incurred v2 observations. V1 totals remain on messages but never seed observations/budgets. No content/title/key/raw payload/balance enters the ledger, and partial coverage is visible.

**Method:** adapter normalization/property tests, fake-stream integration, temporary SQLite/fault injection, fixed-clock boundary tests, IPC safe-integer tests, and UI copy assertions.

### `AC-045` — Per-provider advisory budget and accessible threshold

For each provider, set, replace, and clear budgets at 1, `Number.MAX_SAFE_INTEGER`, zero, negative, fractional, overflow, wrong type, and unknown provider. Only valid values persist. Aggregate known total tokens over the trailing exact seven days and test remaining states at 11%, exactly 10%, 9%, and 0%, including a maximum-safe pair whose exact integer state differs from a rounded floating-point comparison. If the aggregate exceeds `Number.MAX_SAFE_INTEGER`, the budget remains present as exhausted, known-used tokens are null rather than fabricated, the UI explains that the exact total exceeds the supported display range, and coverage is partial. At 10% or less the UI is red and also exposes explicit warning text, icon, and accessible live status; above 10% it does not. Partial coverage warns that known usage may undercount. The budget is called local/advisory, never provider credit/balance/quota/price, and cannot block or alter a send. If a later local query fails, a prior timestamped successful summary may remain only as visibly `Stale` alongside the error; provider offline state does not independently stale a successful local SQLite summary.

**Method:** checked-arithmetic/unit, repository, component/accessibility, fixed-clock, offline/error, 320-pixel and 100–200% Windows scaling tests.

### `AC-046` — Explicit DeepSeek read-only exact balance

An explicit refresh makes exactly one `GET https://api.deepseek.com/user/balance` request with only the DeepSeek credential and no body; no background/automatic retry or other provider balance request exists. Test the 64-KiB body and 16-entry shape cap, required/duplicate keys, then `CNY|USD` only and duplicate-currency rejection (therefore at most two semantically accepted entries), exact decimal grammar `(0|[1-9][0-9]{0,17})(\.[0-9]{1,18})?`, canonical strings, and exact `total = granted + topped_up`. Unknown fields are ignored only after structural/security checks. Cover valid zero/multiple currency, auth/rate-limit/timeout/offline/TLS/redirect/malformed/oversized outcomes. The successful result and last-success UTC time remain Rust-memory-only. A later network/provider failure may retain it only as visibly `Stale` with the error/time. A successful DeepSeek credential replace/delete clears it, invalidates any in-flight authority, and resets reachability; a credential-load failure clears it before showing the safe error. In overlapping refreshes only the latest operation under the unchanged credential generation may update memory. A deterministic race pauses a newer begin while an older completion holds the authority/commit synchronization, then proves the newer operation invalidates the older authority before either balance or reachability can be changed. No result enters SQLite/log/export/browser storage or binary floating point.

**Method:** fake HTTPS contract and decimal/property tests, provider-network negative assertions, credential-generation/overlap/load-failure state tests, memory/persistence/export/log sentinel scans, and Usage component states.

### `AC-047` — Typed provider account action and external browser boundary

Snapshot every supported pair of provider and exact JSON action `usage | billing | addCredits | spend | deployment` against provider-contract section 8. Observe that Rust delegates the corresponding fixed official HTTPS URL to the Windows default browser. NVIDIA `deployment` uses the one general NVIDIA Build destination. Reject every unsupported/case-changed/extra action, wrong provider/window, pre-deserialized form, encoded injection, and attempts to add model ID, URL, path, query, scheme, or content-derived label. A simulated operating-system opener rejection returns a stable safe error and changes no local usage or credential. After the operating system accepts the URL, Aster makes no claim that an offline browser loaded it. React receives labels as non-authoritative catalog metadata and has no raw opener permission. Aster never embeds the page, submits a form, performs a purchase/plan mutation, observes browser login, or reports external transaction success.

**Method:** action-map unit/snapshot and IPC abuse tests, capability inventory, packaged Windows default-browser E2E, and application-network observation.

### `AC-048` — MVP v1 to v2 database and document compatibility

Start from exact valid/corrupt/WAL-active v1 databases and version 1 exports. Before a destructive migration, prove a WAL-consistent SQLite-backup-API snapshot at the deterministic sibling name, current-user ACL, integrity/schema/source identity verification, and absence of live `fs::copy`. Exercise interruption before/during/after backup and transaction, matching verified-backup reuse, conflicting/unverified sibling blocking, failure retention, post-commit v2 integrity/schema verification, deletion only after success, and safe cleanup only of a matching verified orphan. The v2 database maps conversations to `zai`/`glm-5.1`, preserves profile/status/content/timestamps and legacy total-only message usage, leaves breakdown null, creates no v2 usage observation/budget from legacy totals, and is idempotent. Version 1 import applies the same semantic mapping; version 2 requires a catalog pair, valid profile, safe disjoint usage, and emits version 2 on export. Every injected failure rolls back without data loss or partial import.

**Method:** temporary SQLite WAL/backup/ACL/integrity and migration fault-injection suite, golden v1/v2 serialization, import abuse corpus, and restart integration test.

## 5. Product traceability matrix

| Requirement | Acceptance criteria                                                        |
| ----------- | -------------------------------------------------------------------------- |
| `PR-001`    | Retired for MVP v2; historical MVP v1 evidence only                        |
| `PR-002`    | `AC-019`, `AC-025`, `AC-026`, `AC-041`, `AC-043`, `AC-047`                 |
| `PR-003`    | `AC-002`–`AC-015`, `AC-035`, `AC-042`–`AC-048`                             |
| `PR-004`    | `AC-040`–`AC-043`                                                          |
| `PR-005`    | `AC-044`–`AC-047`                                                          |
| `FR-001`    | `AC-001`, `AC-017`, `AC-040`, `AC-045`                                     |
| `FR-002`    | `AC-002`, `AC-042`                                                         |
| `FR-003`    | `AC-002`                                                                   |
| `FR-004`    | `AC-003`, `AC-026`, `AC-048`                                               |
| `FR-005`    | `AC-004`                                                                   |
| `FR-006`    | Retired for MVP v2; superseded by `FR-023` / `AC-041`                      |
| `FR-007`    | Retired for MVP v2; superseded by `FR-020`, `FR-021` / `AC-040`, `AC-043`  |
| `FR-008`    | `AC-007`, `AC-022`, `AC-043`, `AC-044`                                     |
| `FR-009`    | `AC-008`, `AC-023`, `AC-024`                                               |
| `FR-010`    | `AC-009`                                                                   |
| `FR-011`    | `AC-010`, `AC-026`, `AC-042`                                               |
| `FR-012`    | `AC-011`, `AC-042`                                                         |
| `FR-013`    | Retired for MVP v2; historical `AC-012`, superseded by `FR-024` / `AC-043` |
| `FR-014`    | `AC-013`, `AC-021`, `AC-022`, `AC-043`, `AC-046`                           |
| `FR-015`    | `AC-014`, `AC-027`, `AC-028`, `AC-048`                                     |
| `FR-016`    | `AC-015`, `AC-043`                                                         |
| `FR-017`    | `AC-016`                                                                   |
| `FR-018`    | `AC-016`                                                                   |
| `FR-019`    | `AC-036`                                                                   |
| `FR-020`    | `AC-040`, `AC-025`, `AC-033`                                               |
| `FR-021`    | `AC-020`–`AC-022`, `AC-043`                                                |
| `FR-022`    | `AC-042`, `AC-026`, `AC-027`, `AC-048`                                     |
| `FR-023`    | `AC-005`, `AC-019`, `AC-025`, `AC-035`, `AC-041`                           |
| `FR-024`    | `AC-012`, `AC-017`, `AC-043`                                               |
| `FR-025`    | `AC-043`, `AC-044`                                                         |
| `FR-026`    | `AC-017`, `AC-045`                                                         |
| `FR-027`    | `AC-019`, `AC-021`, `AC-029`, `AC-046`                                     |
| `FR-028`    | `AC-024`, `AC-025`, `AC-029`, `AC-047`                                     |
| `FR-029`    | `AC-014`, `AC-026`, `AC-027`, `AC-044`, `AC-048`                           |
| `FR-030`    | `AC-003`, `AC-013`, `AC-022`, `AC-043`, `AC-048`                           |
| `UX-001`    | `AC-004`, `AC-017`, `AC-036`, `AC-040`–`AC-047`                            |
| `UX-002`    | `AC-004`                                                                   |
| `UX-003`    | `AC-002`, `AC-010`, `AC-017`, `AC-036`, `AC-042`                           |
| `UX-004`    | `AC-013`, `AC-017`, `AC-044`–`AC-046`                                      |
| `UX-005`    | `AC-001`, `AC-017`, `AC-040`, `AC-045`                                     |
| `UX-006`    | `AC-017`, `AC-045`                                                         |
| `UX-007`    | `AC-017`                                                                   |
| `UX-008`    | `AC-002`, `AC-015`, `AC-017`, `AC-042`, `AC-047`                           |
| `UX-009`    | `AC-017`, `AC-040`, `AC-042`                                               |
| `UX-010`    | `AC-017`, `AC-044`–`AC-047`                                                |
| `UX-011`    | `AC-017`, `AC-042`                                                         |
| `UX-012`    | `AC-013`, `AC-017`, `AC-043`                                               |
| `NFR-001`   | `AC-018`                                                                   |
| `NFR-002`   | `AC-006`, `AC-018`, `AC-043`                                               |
| `NFR-003`   | `AC-007`, `AC-018`                                                         |
| `NFR-004`   | `AC-018`, `AC-022`, `AC-044`                                               |
| `NFR-005`   | `AC-003`, `AC-010`, `AC-014`, `AC-026`, `AC-027`, `AC-044`, `AC-048`       |
| `NFR-006`   | `AC-013`, `AC-027`, `AC-040`, `AC-043`–`AC-048`                            |
| `NFR-007`   | `AC-035`                                                                   |
| `NFR-008`   | `AC-029`, `AC-040`, `AC-044`–`AC-047`                                      |

## 6. Security traceability matrix

| Security requirement | Acceptance criteria                                                            |
| -------------------- | ------------------------------------------------------------------------------ |
| `SEC-001`            | `AC-005`, `AC-019`, `AC-041`                                                   |
| `SEC-002`            | `AC-005`, `AC-019`, `AC-025`, `AC-041`                                         |
| `SEC-003`            | `AC-005`, `AC-019`, `AC-029`, `AC-041`, `AC-046`                               |
| `SEC-004`            | `AC-005`, `AC-035`, `AC-041`                                                   |
| `SEC-005`            | `AC-032`, `AC-040`                                                             |
| `SEC-006`            | Retired for MVP v2; historical `AC-020` only                                   |
| `SEC-007`            | `AC-013`, `AC-020`, `AC-033`, `AC-043`, `AC-046`                               |
| `SEC-008`            | `AC-013`, `AC-021`, `AC-043`, `AC-044`, `AC-046`                               |
| `SEC-009`            | Retired for MVP v2; historical `AC-006`, `AC-012` only                         |
| `SEC-010`            | `AC-022`, `AC-043`, `AC-044`                                                   |
| `SEC-011`            | `AC-006`, `AC-007`, `AC-022`, `AC-043`, `AC-044`                               |
| `SEC-012`            | `AC-008`, `AC-023`, `AC-024`                                                   |
| `SEC-013`            | `AC-023`, `AC-029`, `AC-033`                                                   |
| `SEC-014`            | `AC-008`, `AC-023`, `AC-024`                                                   |
| `SEC-015`            | `AC-024`, `AC-047`                                                             |
| `SEC-016`            | `AC-009`                                                                       |
| `SEC-017`            | `AC-025`, `AC-033`, `AC-040`, `AC-041`, `AC-047`                               |
| `SEC-018`            | `AC-004`, `AC-013`, `AC-025`, `AC-040`–`AC-047`                                |
| `SEC-019`            | `AC-007`, `AC-010`, `AC-011`, `AC-022`, `AC-025`, `AC-042`–`AC-044`            |
| `SEC-020`            | `AC-013`, `AC-019`, `AC-025`, `AC-029`, `AC-041`, `AC-043`, `AC-046`, `AC-047` |
| `SEC-021`            | `AC-001`, `AC-025`, `AC-033`, `AC-040`, `AC-047`                               |
| `SEC-022`            | `AC-003`, `AC-010`, `AC-019`, `AC-026`, `AC-043`, `AC-044`, `AC-048`           |
| `SEC-023`            | `AC-003`, `AC-026`, `AC-035`, `AC-048`                                         |
| `SEC-024`            | `AC-014`, `AC-027`, `AC-042`, `AC-048`                                         |
| `SEC-025`            | `AC-014`, `AC-026`, `AC-027`, `AC-048`                                         |
| `SEC-026`            | `AC-014`, `AC-028`, `AC-048`                                                   |
| `SEC-027`            | `AC-014`, `AC-019`, `AC-028`, `AC-048`                                         |
| `SEC-028`            | `AC-002`, `AC-026`                                                             |
| `SEC-029`            | `AC-015`, `AC-029`, `AC-035`, `AC-043`                                         |
| `SEC-030`            | `AC-019`, `AC-020`, `AC-029`, `AC-036`, `AC-043`                               |
| `SEC-031`            | `AC-019`, `AC-023`, `AC-029`, `AC-032`, `AC-036`, `AC-040`, `AC-044`, `AC-046` |
| `SEC-032`            | `AC-002`, `AC-014`, `AC-015`, `AC-045`                                         |
| `SEC-033`            | `AC-030`, `AC-033`, `AC-038`                                                   |
| `SEC-034`            | `AC-030`                                                                       |
| `SEC-035`            | `AC-030`, `AC-031`, `AC-038`                                                   |
| `SEC-036`            | `AC-030`, `AC-034`                                                             |
| `SEC-037`            | `AC-031`, `AC-035`                                                             |
| `SEC-038`            | `AC-031`, `AC-033`                                                             |
| `SEC-039`            | `AC-019`, `AC-023`, `AC-032`, `AC-033`, `AC-039`, `AC-040`, `AC-041`           |
| `SEC-040`            | `AC-020`–`AC-028`, `AC-040`–`AC-048` as applicable                             |
| `SEC-041`            | `AC-030`, `AC-034`                                                             |
| `SEC-042`            | `AC-034`, `AC-041`                                                             |
| `SEC-043`            | `AC-037`, `AC-039`                                                             |
| `SEC-044`            | `AC-030`, `AC-038`                                                             |
| `SEC-045`            | `AC-039`                                                                       |
| `SEC-046`            | `AC-020`, `AC-025`, `AC-033`, `AC-040`, `AC-043`                               |
| `SEC-047`            | `AC-012`, `AC-022`, `AC-040`, `AC-043`, `AC-044`                               |
| `SEC-048`            | `AC-005`, `AC-019`, `AC-025`, `AC-035`, `AC-041`, `AC-043`                     |
| `SEC-049`            | `AC-019`, `AC-026`, `AC-044`, `AC-045`, `AC-048`                               |
| `SEC-050`            | `AC-019`, `AC-021`, `AC-029`, `AC-046`                                         |
| `SEC-051`            | `AC-024`, `AC-025`, `AC-029`, `AC-047`                                         |
| `SEC-052`            | `AC-026`, `AC-027`, `AC-042`, `AC-043`, `AC-048`                               |

## 7. Threat-to-evidence traceability

| Threats           | Primary acceptance criteria                                                             |
| ----------------- | --------------------------------------------------------------------------------------- |
| `TM-001`–`TM-006` | `AC-005`, `AC-015`, `AC-019`, `AC-029`, `AC-032`, `AC-033`, `AC-036`, `AC-041`          |
| `TM-007`–`TM-011` | `AC-008`, `AC-009`, `AC-023`, `AC-024`, `AC-025`                                        |
| `TM-012`–`TM-017` | `AC-006`, `AC-007`, `AC-013`, `AC-021`, `AC-022`, `AC-025`, `AC-043`, `AC-047`          |
| `TM-018`–`TM-021` | `AC-012`, `AC-015`, `AC-020`, `AC-029`, `AC-040`, `AC-043`                              |
| `TM-022`–`TM-028` | `AC-002`, `AC-003`, `AC-010`, `AC-014`, `AC-026`–`AC-028`, `AC-042`, `AC-044`, `AC-048` |
| `TM-029`–`TM-034` | `AC-019`, `AC-023`, `AC-029`–`AC-034`, `AC-036`, `AC-040`, `AC-046`, `AC-047`           |
| `TM-035`          | `AC-005`, `AC-019`, `AC-025`, `AC-035`, `AC-041`                                        |
| `TM-036`          | `AC-019`, `AC-030`, `AC-031`, `AC-033`                                                  |
| `TM-037`          | `AC-019`, `AC-020`, `AC-041`, `AC-043`                                                  |
| `TM-038`          | `AC-026`, `AC-027`, `AC-042`, `AC-043`, `AC-048`                                        |
| `TM-039`          | `AC-019`, `AC-026`, `AC-044`, `AC-045`, `AC-048`                                        |
| `TM-040`          | `AC-019`, `AC-021`, `AC-029`, `AC-046`                                                  |
| `TM-041`          | `AC-024`, `AC-025`, `AC-029`, `AC-047`                                                  |

## 8. Release evidence checklist

A release report must state each item, not merely say “all checks passed”:

- source revision and clean/dirty state;
- version and exact artifact hashes;
- requirements and threats changed since the previous baseline;
- unit, component, integration, contract, security, and E2E result summaries;
- Windows version, architecture, WebView2 version, display-scale/accessibility matrix;
- frontend format/type/lint/test and Rust format/clippy/test results;
- secret, static, dependency vulnerability/license/policy scan results;
- SBOM location, component/version/license coverage, validation result, and declared scope;
- canonical project-license/NOTICE policy results and packaged third-party redistribution materials;
- CSP, Tauri capability, bundle, remote-origin, and package inventory results;
- migration and import-abuse result summaries;
- signature status and post-package verification;
- update status (`disabled` is acceptable for v2; `enabled` requires full evidence);
- active exceptions, residual risks, failures, and `NOT RUN` gates;
- final classification: browser preview, engineering MVP build, or production release.

## 9. Definition of MVP acceptance

The MVP outcome is accepted only when:

1. Every `Must` product requirement maps to at least one criterion and every mapped criterion required for the claimed release class passes.
2. Every security requirement applicable to that class has evidence; no runtime security control is deferred.
3. All Critical/High threats have implemented and passing mapped controls, except explicitly stated inherent external residuals such as provider-side handling.
4. The application has no hidden capability beyond the documented v2 boundary.
5. Required failures and `NOT RUN` results block the corresponding release claim.
6. The Definition of Done in [AGENTS.md](../AGENTS.md) is complete and the release report accurately states residual risk.
