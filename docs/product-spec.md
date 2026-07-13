# Aster Desktop — MVP v1 Product Specification

**Status:** Normative baseline

**Target platform:** Windows 11

**Specification version:** 1.0

**Last updated:** 2026-07-12

**Implementation status:** This document defines the required MVP outcome; it does not assert that a source revision or artifact has passed acceptance.

## 1. Product intent

`PR-001` defines a privacy-conscious Windows desktop application for direct, streamed conversations with `glm-5.1` through the official Z.AI API. The experience is visually inspired by a modern developer assistant: a compact navigation rail, local conversation list, focused work area, and anchored composer. It must be original in naming, icons, and assets and must not reproduce third-party branding.

`PR-002` defines the MVP as a real desktop client, not a website containing an API key. Tauri/Rust owns credentials, network access, persistence, validation, and cancellation. React/TypeScript owns presentation and user interaction.

`PR-003` defines success as a complete, dependable chat lifecycle: configure securely, create a conversation, stream and stop a response, persist locally, recover after restart, revise or regenerate, and safely import or export a conversation.

## 2. Intended user and primary journey

The primary user is a Windows 11 developer, analyst, or security practitioner who has their own Z.AI API key and wants a focused local chat history.

The golden path is:

1. Launch the installed desktop application.
2. Read the external-processing notice and use the Rust-owned native credential prompt to save a Z.AI API key to Windows Credential Manager.
3. Start a new chat or choose a local conversation.
4. Enter a prompt and choose Fast, Standard, or Deep mode.
5. Watch the response arrive incrementally, or stop it.
6. Copy, edit and resend, or regenerate content.
7. Close and relaunch without losing completed local history.
8. Export selected conversation data or import a valid application export.

## 3. Release profiles

The implementation may run in two profiles, which must never be confused:

| Profile      | Purpose              | Provider                        | Persistence                    | Secrets                  |
| ------------ | -------------------- | ------------------------------- | ------------------------------ | ------------------------ |
| Desktop      | MVP product behavior | Z.AI through Rust               | SQLite                         | Windows credential store |
| Browser demo | Visual QA only       | In-memory deterministic adapter | In memory for current tab only | Forbidden                |

Browser demo is not an alternative product architecture. It must display a `Demo mode` indicator, must not expose credential settings, and must not imply that persistence, provider networking, OS credential storage, or installer security has been verified.

## 4. MVP scope

### 4.1 Functional requirements

| ID       | Requirement                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | Priority |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| `FR-001` | The desktop application **MUST** present an original Windows 11-appropriate shell with a sidebar, conversation work area, settings access, provider/connection status, and anchored composer.                                                                                                                                                                                                                                                                                                                       | Must     |
| `FR-002` | The user **MUST** be able to create a new empty conversation without overwriting the currently selected conversation.                                                                                                                                                                                                                                                                                                                                                                                               | Must     |
| `FR-003` | The user **MUST** be able to select, rename, and delete a local conversation. Delete **MUST** require confirmation and choose a deterministic next selection.                                                                                                                                                                                                                                                                                                                                                       | Must     |
| `FR-004` | The application **MUST** persist conversations and messages in local SQLite storage and restore them after a normal application restart.                                                                                                                                                                                                                                                                                                                                                                            | Must     |
| `FR-005` | The composer **MUST** support multiline text, keyboard submission, visible disabled/loading states, whitespace rejection, and one active generation per conversation.                                                                                                                                                                                                                                                                                                                                               | Must     |
| `FR-006` | Settings **MUST** allow the user to open a Rust-owned native Windows credential prompt to save or replace an 8–255-byte printable-ASCII Z.AI API key and to remove it through the Rust credential-store adapter. The application **MUST NOT** truncate input. A full 256-character native result is rejected as potentially truncated. The webview **MUST NOT** render a credential field or carry the key in an IPC argument, result, event, or frontend state. It receives only configured/not-configured status. | Must     |
| `FR-007` | Sending a prompt **MUST** call `glm-5.1` through the Rust backend and display valid SSE content increments in order before the response completes.                                                                                                                                                                                                                                                                                                                                                                  | Must     |
| `FR-008` | While generation is active, the send action **MUST** become a stop action that aborts the backend request and leaves a clear cancelled state without later appending stale events.                                                                                                                                                                                                                                                                                                                                  | Must     |
| `FR-009` | Assistant content **MUST** render safe Markdown, including headings, lists, tables, links, and fenced code. Code **MUST** remain readable when language highlighting is unavailable.                                                                                                                                                                                                                                                                                                                                | Must     |
| `FR-010` | The user **MUST** be able to copy a whole response and the contents of an individual code block, with non-disruptive success/failure feedback.                                                                                                                                                                                                                                                                                                                                                                      | Must     |
| `FR-011` | The user **MUST** be able to edit and resend a previous user message. The application **MUST** explicitly replace that message and remove its now-invalid descendants before generating the new response.                                                                                                                                                                                                                                                                                                           | Must     |
| `FR-012` | The user **MUST** be able to regenerate the most recent assistant response using the preceding conversation context without duplicating the user message.                                                                                                                                                                                                                                                                                                                                                           | Must     |
| `FR-013` | The composer **MUST** expose Fast, Standard, and Deep modes and map them in Rust to the verified `thinking.type` contract and explicit output-token caps below. The UI **MUST NOT** claim that Standard and Deep are distinct provider reasoning-effort levels and **MUST NOT** expose or persist hidden chain-of-thought.                                                                                                                                                                                          | Must     |
| `FR-014` | The application **MUST** map missing credentials, `401/403`, `429`, provider `5xx`, timeout, malformed stream, provider content rejection, provider context limit, offline/network, and cancellation outcomes to distinct, actionable UI states.                                                                                                                                                                                                                                                                    | Must     |
| `FR-015` | In the desktop profile, the user **MUST** be able to export one conversation to a versioned JSON document through a Rust-owned native save dialog and import a valid application export atomically through a Rust-owned native open dialog. React **MUST NOT** supply a filesystem path.                                                                                                                                                                                                                            | Must     |
| `FR-016` | Before the first provider request, the application **MUST** show that messages are processed by an external provider and advise the user not to send secrets or sensitive data without reviewing provider policies.                                                                                                                                                                                                                                                                                                 | Must     |
| `FR-017` | The sidebar **MUST** group or sort conversation history by a deterministic recent-first rule and display useful empty and selected states.                                                                                                                                                                                                                                                                                                                                                                          | Must     |
| `FR-018` | The user **MUST** be able to dismiss non-critical onboarding or promotional surfaces without affecting core chat operation.                                                                                                                                                                                                                                                                                                                                                                                         | Should   |
| `FR-019` | The sidebar **MUST** provide a local, case-insensitive title filter over already loaded conversation summaries. Filtering **MUST NOT** inspect message bodies, persist or log the query, or trigger provider traffic.                                                                                                                                                                                                                                                                                               | Must     |

### 4.2 UX and accessibility requirements

| ID       | Requirement                                                                                                                                                         |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `UX-001` | All interactive controls **MUST** be reachable and operable by keyboard, have an accessible name, and display a visible focus indicator.                            |
| `UX-002` | `Enter` **MUST** submit when composition is not active; `Shift+Enter` **MUST** insert a newline. IME composition **MUST NOT** submit prematurely.                   |
| `UX-003` | Focus **MUST** move predictably after new chat, deletion, edit, modal open/close, error recovery, and completed send.                                               |
| `UX-004` | Status and error changes **MUST** be conveyed in text and, when appropriate, an accessible live region; color alone is insufficient.                                |
| `UX-005` | The primary workflow **MUST** remain usable at 320 logical pixels of content width, at Windows display scales from 100% through 200%, and at 1280×720 or greater.   |
| `UX-006` | The application **MUST** support light/dark operating-system preference or provide an accessible dark theme with WCAG 2.2 AA contrast for normal text and controls. |
| `UX-007` | Motion **MUST** respect `prefers-reduced-motion`; streaming content must not cause avoidable focus or scroll loss.                                                  |
| `UX-008` | Destructive confirmation and external-processing notices **MUST** use plain, specific English and identify the consequence before the user proceeds.                |

### 4.3 Non-functional requirements

| ID        | Requirement                                                                                                                                                                                        |
| --------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `NFR-001` | On the reference Windows 11 environment, an already-started application **SHOULD** make the composer interactive within 2 seconds after window creation, excluding first-run OS installation work. |
| `NFR-002` | For a responsive provider connection, the application **MUST** display the first valid content delta as soon as processed and **SHOULD** add no more than 150 ms p95 client-side batching delay.   |
| `NFR-003` | Stop **SHOULD** change the UI state within 100 ms and **MUST** signal backend cancellation within 500 ms under normal local load.                                                                  |
| `NFR-004` | The UI **MUST** remain responsive while rendering a response of at least 100,000 Unicode characters and history of at least 100 conversations with 200 messages each in the verification fixture.  |
| `NFR-005` | A process crash or rejected import **MUST NOT** leave a partially committed message, conversation, or import operation.                                                                            |
| `NFR-006` | User-visible error messages, stored schema versions, and IPC error codes **MUST** be deterministic enough for automated verification.                                                              |
| `NFR-007` | The MVP **MUST** run on currently supported 64-bit Windows 11 editions using the packaged WebView2 prerequisites.                                                                                  |

Security, privacy, supply-chain, and release integrity requirements are normative in [security-requirements.md](security-requirements.md) and apply even when not repeated here.

## 5. Response-profile contract

The UI labels remain provider-neutral and comprehensible. They are application response profiles, not provider reasoning-effort levels:

| UI label | Exact provider thinking field | Application-owned output cap | Honest user guidance |
| --- | --- | --- |
| Fast | `thinking: { "type": "disabled" }` | 4,096 tokens | Thinking off; shortest output cap |
| Standard | `thinking: { "type": "enabled" }` | 8,192 tokens | Thinking on; default output cap |
| Deep | `thinking: { "type": "enabled" }` | 16,384 tokens | Thinking on; longest output cap |

The verified current contract supersedes the source plan's `glm-5.2`, `enable_thinking`, and `reasoning_effort: "high"`/`"max"` assumptions. Standard and Deep both enable provider thinking; their contractual difference is the application-owned output-token budget and UX guidance, not a claimed hidden reasoning-effort setting. The caps are within the provider's documented `max_tokens` range but are product policy, not provider-defined modes. If the provider rejects a documented field, the backend returns a typed compatibility error; it must not silently choose a different model or profile. Internal `reasoning_content` or hidden chain-of-thought is discarded and is not displayed, logged, persisted, exported, or replayed. The exact dated provider baseline and re-verification triggers are in [provider-contract.md](provider-contract.md).

## 6. Data and lifecycle rules

- A `Conversation` has an opaque ID, title, model, reasoning mode, created timestamp, and updated timestamp.
- A `Message` has an opaque ID, conversation ID, role, content, created timestamp, status, and optional non-sensitive usage metadata.
- Roles accepted from persisted or imported data are limited to the explicitly supported set. System or tool roles are not user-importable in MVP v1.
- `streaming` and `stopping` are ephemeral runtime/UI states and are never valid SQLite, import, or export values. Persisted user messages have `complete` status. Persisted assistant messages use only terminal `complete`, `cancelled`, or `error` status.
- A conversation title may be derived locally from the first user message; the derivation must not make an additional provider call without consent.
- Validated partial assistant content after cancellation or failure may be retained with `cancelled` or `error` status. A crash or failed request must not fabricate or present a complete answer.
- Timestamps are stored in UTC and rendered using the user's locale.
- Exported JSON uses the exact versioned allowlist schema in architecture section 9. It excludes source conversation/message IDs, `conversationId`, derived counts, credentials, internal filesystem paths, request headers, logs, hidden reasoning, and application configuration. Rust generates all local IDs during import.

The detailed storage and IPC ownership model appears in [architecture.md](architecture.md).

## 7. Explicitly out of scope for MVP v1

The following features are deferred and must not be backed by hidden permissions in v1:

- Projects, project instructions, or allowed-directory management.
- File or document attachments and local document indexing/RAG.
- Model-invoked tools, function calling, PowerShell, Command Prompt, process execution, or arbitrary filesystem access.
- Voice input/output, cloud synchronization, shared accounts, or multi-user access control.
- Full-message/global-content search, provider-powered search, token billing estimates, context summarization, or prompt templates. The local title filter in `FR-019` remains in scope.
- A locally hosted GLM model.
- Automatic updates unless a verified signed update chain and rollback plan are implemented.

Navigation previews for a deferred feature may be shown only if disabled or labelled `Coming soon`, keyboard-inert where appropriate, and excluded from claims of completion.

## 8. Product constraints and assumptions

- The user supplies a valid Z.AI API key and accepts the provider's terms, data handling, cost, and availability.
- The client cannot guarantee provider response quality, truthfulness, retention, availability, or latency.
- Local data is protected by Windows account boundaries and least-privilege file permissions. Full database encryption is not claimed in MVP v1; users must be warned that chat history is stored locally.
- A development build is not a production release. Installer signing and verification evidence are required before distribution as a production build.
- The visual reference informs layout density and hierarchy only. The product uses original copy, branding, and assets.

## 9. Success measures

The MVP is acceptable when all `Must` requirements and their mapped security requirements meet the release policy in [acceptance-criteria.md](acceptance-criteria.md). Optional qualitative measures after release may include successful first-session completion, stop reliability, crash-free sessions, and import/export recovery; analytics are not collected in MVP v1.

## 10. Change control

Any proposed scope addition must identify:

1. The new or changed requirement IDs.
2. Data classification and retention impact.
3. New trust boundaries, permissions, or provider calls.
4. Updated threats and mitigations.
5. Acceptance tests and rollback behavior.
6. Whether the capability remains within MVP v1 or requires a later version.

The process in the repository [AGENTS.md](../AGENTS.md) is mandatory.
