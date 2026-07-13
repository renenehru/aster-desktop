# Aster Desktop — MVP v1 Threat Model

**Method:** STRIDE-informed misuse-case analysis

**Status:** Normative; control implementation and evidence tracked separately

**Model version:** 1.0

**Last updated:** 2026-07-12

**Implementation status:** Threat dispositions and target residual scores are design targets; no control is verified without retained revision-specific acceptance evidence.

## 1. Scope and security posture

This threat model covers the Windows 11 Tauri desktop client, its React webview, Rust command/core boundary, Windows credential-store integration, SQLite history, conversation import/export, HTTPS/SSE connection to Z.AI, and the build/release chain.

The model treats the following as untrusted:

- every React IPC argument and every event consumed by React;
- all model and provider content, including errors and SSE framing;
- conversation history loaded from disk and every imported document;
- filenames, dialog results, URLs, Markdown, code blocks, and link labels;
- dependencies, build inputs, and artifacts until their provenance is verified.

The application does not grant model content command, filesystem, process, network-selection, or tool authority. Projects, attachments, document parsing, model-invoked tools, shell access, and arbitrary filesystem access are excluded from MVP v1. Adding any of them requires a new data-flow and threat-model revision before code or permissions are added.

## 2. Assets and adverse outcomes

| Asset                      | Desired property                         | Material adverse outcome                                            |
| -------------------------- | ---------------------------------------- | ------------------------------------------------------------------- |
| Z.AI API key               | Confidentiality, controlled use          | Key theft, unauthorized spend, account abuse                        |
| Prompts/responses/history  | Confidentiality, integrity, availability | Disclosure, cross-conversation mix-up, loss, or silent modification |
| User intent                | Authenticity                             | A model/import causes an action the user did not request            |
| Local database/export      | Integrity, availability, confidentiality | Injection, partial mutation, path overwrite, plaintext exposure     |
| Provider request           | Integrity, authenticity                  | Wrong endpoint/model/mode/context or replayed paid request          |
| UI and application state   | Integrity                                | Spoofed status, stale stream, misleading completion state           |
| Application package/update | Integrity, provenance                    | Trojan installer, compromised dependency, malicious update          |
| Diagnostics                | Confidentiality, integrity               | Secret/content leakage or misleading evidence                       |

## 3. Actors and assumptions

### 3.1 Threat actors

- A malicious or compromised upstream provider response.
- An attacker controlling imported conversation content or a shared export file.
- A network attacker able to observe, block, redirect, or tamper with traffic but unable to break properly validated TLS.
- Malicious web content attempting to navigate or execute inside the webview.
- A compromised or malicious dependency/build input.
- Malware or another process running as the same Windows user.
- An accidental insider or contributor introducing insecure configuration, logging, or permissions.
- A user making a destructive mistake or misunderstanding external processing.

### 3.2 Security assumptions

- Windows 11, WebView2, the certificate trust store, Windows credential store, and OS account isolation are maintained and not already fully compromised.
- The user protects their Windows session and supplies a legitimate Z.AI credential.
- Z.AI processes messages according to its own terms; the client cannot enforce provider-side retention.
- A production signing identity, when used, is protected outside the repository and CI exposes it only to a trusted protected release job.
- The official provider endpoint and current API contract are verified before a release when they change.

If an attacker controls the unlocked user account or runs arbitrary code as that user, plaintext chat history and live-process secrets may be accessible. The MVP reduces exposure but does not claim resistance to full same-user endpoint compromise.

## 4. Trust boundaries and data flows

The detailed diagram is in [architecture.md](architecture.md). This model uses these boundaries:

| Boundary | From → to                                                          | Data                                    | Primary threats                                                          |
| -------- | ------------------------------------------------------------------ | --------------------------------------- | ------------------------------------------------------------------------ |
| `TB-01`  | User/model content → React rendering                               | Prompt/response Markdown, links, code   | XSS, unsafe navigation, spoofing, resource exhaustion                    |
| `TB-02`  | React webview → Tauri/Rust                                         | Typed commands, IDs, text, mode         | IPC abuse, state confusion, oversized input, privilege misuse            |
| `TB-03`  | Native Windows credential prompt → Rust → Windows credential store | Capture/store/read/delete API key       | Buffer misuse, secret disclosure, wrong target, residual memory          |
| `TB-04`  | Rust repository → SQLite                                           | Conversations, messages, migrations     | Injection, corruption, local disclosure, partial commit                  |
| `TB-05`  | Rust provider adapter ↔ Z.AI                                       | HTTPS request and streamed SSE response | MITM, endpoint substitution, malformed stream, replay/cost amplification |
| `TB-06`  | Native dialog/file ↔ import/export service                         | Untrusted JSON/plaintext export         | Traversal, schema abuse, bombs, disclosure                               |
| `TB-07`  | Source/dependencies → build → installed artifact                   | Code, lockfiles, packages, signatures   | Dependency compromise, secret inclusion, unsigned/tampered artifact      |

## 5. Risk method

Likelihood (`L`) and impact (`I`) are scored from 1 (low) to 5 (very high). Risk is `L × I`:

- 1–4: Low
- 5–9: Moderate
- 10–16: High
- 17–25: Critical

`Inherent` assumes the application feature exists without the listed controls. `Target residual` is the expected risk only after all mapped requirements pass. A target score is not a claim that implementation is verified. Until acceptance evidence passes, the disposition remains `Open — verification required`.

## 6. Threat register

### 6.1 Credentials and data disclosure

| ID / STRIDE                                  | Threat and abuse case                                                                                                                                                       |        Inherent | Required controls                                                 | Target residual | Disposition                                                                                 |
| -------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------: | ----------------------------------------------------------------- | --------------: | ------------------------------------------------------------------------------------------- |
| `TM-001` / Information disclosure            | An API key is embedded in frontend code, browser storage, SQLite, an export, fixture, environment asset, or package and is recovered by inspection.                         | 5×5=25 Critical | `SEC-001`, `SEC-002`, `SEC-005`, `SEC-039`                        |  1×5=5 Moderate | Open — scans and artifact evidence required                                                 |
| `TM-002` / Information disclosure            | An authorization value leaks through an exception, debug formatter, provider trace, UI event, crash log, or diagnostic file.                                                | 4×5=20 Critical | `SEC-002`, `SEC-003`, `SEC-020`, `SEC-031`, `SEC-042`             |  1×5=5 Moderate | Open — sentinel error/log tests required                                                    |
| `TM-003` / Spoofing, information disclosure  | A credential is saved under a generic/wrong credential-store target, read by the wrong app context, or remains after the user removes it.                                   |     3×5=15 High | `SEC-001`, `SEC-004`                                              |       1×4=4 Low | Open — isolated Windows integration evidence required                                       |
| `TM-004` / Information disclosure            | The client sends an unrelated conversation, local title-filter query, path, file, log, or device detail as context, exposing data beyond user intent.                       |     4×4=16 High | `FR-019`, `SEC-029`, `SEC-030`, `SEC-031`                         |       1×4=4 Low | Open — request and local-filter network/storage/log sentinel tests required                 |
| `TM-005` / Information disclosure            | A user mistakes local storage or provider processing for private/offline/encrypted handling and sends sensitive material.                                                   |     4×4=16 High | `FR-016`, `SEC-029`, `SEC-032`                                    |  2×4=8 Moderate | Open — copy and first-run E2E required; provider risk remains                               |
| `TM-006` / Information disclosure            | Browser demo mode solicits or persists a real key, or silently calls a provider, despite lacking desktop controls.                                                          |     3×5=15 High | `SEC-005`, `SEC-031`, `SEC-039`                                   |  1×5=5 Moderate | Open — browser profile negative assertions required                                         |
| `TM-035` / Information disclosure, tampering | The isolated native credential FFI uses an invalid pointer/length, retains a password buffer, accepts caller-controlled dialog data, or returns secret material across IPC. | 4×5=20 Critical | `ADR-0008`, `SEC-001`, `SEC-002`, `SEC-004`, `SEC-017`, `SEC-020` |  1×5=5 Moderate | Open — focused unsafe review, static inventory, sentinel and packaged prompt tests required |

### 6.2 Rendering, navigation, and content confusion

| ID / STRIDE                                  | Threat and abuse case                                                                                                                                           |        Inherent | Required controls                                         | Target residual | Disposition                                                            |
| -------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------: | --------------------------------------------------------- | --------------: | ---------------------------------------------------------------------- |
| `TM-007` / Elevation of privilege, tampering | Model or imported Markdown executes script/HTML in the webview, invokes an IPC scheme, reads UI data, or changes application state.                             | 5×5=25 Critical | `SEC-012`, `SEC-013`, `SEC-014`, `SEC-017`, `SEC-018`     |  1×5=5 Moderate | Open — packaged CSP and XSS corpus required                            |
| `TM-008` / Spoofing, elevation of privilege  | A rendered link uses `javascript:`, `data:`, `file:`, a custom protocol, opener access, or deceptive label to execute or escape expected navigation.            |     4×4=16 High | `SEC-012`, `SEC-015`                                      |  2×3=6 Moderate | Open — protocol matrix and E2E required; phishing remains              |
| `TM-009` / Spoofing                          | Model output imitates a trusted system prompt, security warning, connection status, or approval and convinces the user to disclose a secret or run copied code. |     4×4=16 High | `SEC-016`, `SEC-021`, `UX-004`, clear app-owned chrome    |  3×3=9 Moderate | Open — UI distinction review required; social-engineering risk remains |
| `TM-010` / Elevation of privilege            | Prompt injection in a conversation/export instructs the model or client to execute tools, read files, or bypass policy.                                         | 4×5=20 Critical | `SEC-021`, `SEC-030`; no MVP tool/attachment capabilities |       1×4=4 Low | Open — command/capability absence must be verified                     |
| `TM-011` / Tampering                         | Copying a response or code block copies hidden executable markup or triggers execution/transmission without an explicit later user action.                      |     3×4=12 High | `SEC-014`, `SEC-016`                                      |       1×3=3 Low | Open — clipboard tests required                                        |

### 6.3 IPC, stream, and state integrity

| ID / STRIDE                                  | Threat and abuse case                                                                                                                                                                         |        Inherent | Required controls                               | Target residual | Disposition                                                        |
| -------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------: | ----------------------------------------------- | --------------: | ------------------------------------------------------------------ |
| `TM-012` / Elevation of privilege            | A compromised webview invokes undocumented/wildcard Tauri commands, arbitrary paths/URLs, shell/process capability, or commands from an untrusted window.                                     | 5×5=25 Critical | `SEC-006`, `SEC-017`, `SEC-018`, `SEC-021`      |  1×5=5 Moderate | Open — packaged capability inventory required                      |
| `TM-013` / Tampering                         | Malformed IDs, enum values, lengths, or state transitions bypass frontend validation and corrupt or expose another conversation.                                                              |     4×4=16 High | `SEC-018`, `SEC-019`, `SEC-020`, `SEC-022`      |       1×4=4 Low | Open — backend boundary/property tests required                    |
| `TM-014` / Tampering, information disclosure | Late, duplicated, out-of-order, or cross-request SSE events append one response to another conversation or change a terminal answer.                                                          |     4×4=16 High | `SEC-011`, `SEC-019`                            |       1×4=4 Low | Open — concurrency/race evidence required                          |
| `TM-015` / Denial of service                 | A provider sends an unterminated line, endless non-content events, unknown finish reason, huge delta, invalid UTF-8, or unbounded response and exhausts resources or confuses terminal state. |     4×4=16 High | `SEC-008`, `SEC-010`, `SEC-040`                 |  2×3=6 Moderate | Open — bounds, finish-reason, fuzzing, and resource tests required |
| `TM-016` / Tampering, denial of service      | Stop changes only the UI while the backend continues a paid request, or a cancellation race records a complete/failed answer incorrectly.                                                     |     4×4=16 High | `SEC-011`, `SEC-019`, `NFR-003`                 |       1×3=3 Low | Open — network and persistence cancellation tests required         |
| `TM-017` / Denial of service, repudiation    | Aggressive retries replay a paid prompt, amplify rate limiting, duplicate output, or make it unclear which response is authoritative.                                                         |     4×4=16 High | `SEC-008`, `SEC-019`, deterministic request IDs |       1×4=4 Low | Open — retry/failure contract tests required                       |

### 6.4 Network and provider authenticity

| ID / STRIDE                                 | Threat and abuse case                                                                                                         |        Inherent | Required controls                                              | Target residual | Disposition                                      |
| ------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- | --------------: | -------------------------------------------------------------- | --------------: | ------------------------------------------------ |
| `TM-018` / Spoofing, information disclosure | DNS/proxy/config/IPC manipulation directs the API key or prompts to an attacker-controlled endpoint.                          | 4×5=20 Critical | `SEC-006`, `SEC-007`, `SEC-017`                                |  1×5=5 Moderate | Open — invalid endpoint and TLS tests required   |
| `TM-019` / Tampering                        | Certificate validation is disabled for development and accidentally ships, enabling interception or response modification.    |     3×5=15 High | `SEC-007`, `SEC-034`, `SEC-039`                                |  1×5=5 Moderate | Open — package/config assertion required         |
| `TM-020` / Tampering, spoofing              | An unverified model name or reasoning field silently selects different provider behavior, cost, or output than the UI claims. |     3×4=12 High | `FR-013`, `SEC-009`, provider contract tests                   |       1×3=3 Low | Open — request snapshots required                |
| `TM-021` / Information disclosure           | Provider-side compromise, retention, or policy behavior exposes sent content beyond client control.                           |     3×5=15 High | `SEC-029`, `SEC-030`, data minimization and user policy review |     3×4=12 High | Accepted residual only with explicit user notice |

### 6.5 Persistence and file handling

| ID / STRIDE                                  | Threat and abuse case                                                                                                                                                                                 |        Inherent | Required controls                                     | Target residual | Disposition                                                    |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------: | ----------------------------------------------------- | --------------: | -------------------------------------------------------------- |
| `TM-022` / Tampering                         | A title/content/identifier performs SQL injection or breaks relational integrity, altering unrelated data.                                                                                            | 4×5=20 Critical | `SEC-018`, `SEC-022`                                  |       1×4=4 Low | Open — injection and repository tests required                 |
| `TM-023` / Tampering, denial of service      | Crash, terminalization, migration, edit/resend, delete, or import failure partially commits state, persists an ephemeral stream state, fabricates a complete answer, or silently replaces a database. |     4×4=16 High | `SEC-022`, `SEC-023`, `SEC-025`, `SEC-028`, `NFR-005` |       1×4=4 Low | Open — fault injection/rollback and status-enum tests required |
| `TM-024` / Denial of service                 | A deeply nested, huge, high-count, malformed, or incompatible import exhausts resources or leaves unusable state.                                                                                     |     4×4=16 High | `SEC-024`, `SEC-025`, `SEC-040`                       |       1×4=4 Low | Open — abuse corpus and resource limits required               |
| `TM-025` / Elevation of privilege, tampering | Imported fields restore internal IDs, unsupported roles/status, configuration, paths, HTML execution state, or a credential.                                                                          | 4×5=20 Critical | `SEC-024`, `SEC-025`, `SEC-027`                       |       1×4=4 Low | Open — negative schema tests required                          |
| `TM-026` / Tampering, information disclosure | Crafted title/path traverses directories, overwrites an unintended file, or causes the app to read an arbitrary file during import/export.                                                            | 4×5=20 Critical | `SEC-026`, native scoped dialogs                      |       1×4=4 Low | Open — traversal and dialog-adapter tests required             |
| `TM-027` / Information disclosure            | A plaintext export or local SQLite database is read by another process/user, backed up unintentionally, or shared without realizing it contains sensitive content.                                    |     4×4=16 High | `SEC-023`, `SEC-027`, `SEC-032`                       |  3×3=9 Moderate | Open — permissions/warning evidence; endpoint residual remains |
| `TM-028` / Tampering, repudiation            | A delete action lacks confirmation or transaction scope and removes the wrong conversation without recovery clarity.                                                                                  |     3×4=12 High | `SEC-028`, `UX-003`, `UX-008`                         |       1×3=3 Low | Open — repository/E2E evidence required                        |

### 6.6 Supply chain, diagnostics, and release

| ID / STRIDE                                  | Threat and abuse case                                                                                                                                                                                                                         |        Inherent | Required controls                                               | Target residual | Disposition                                                       |
| -------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------: | --------------------------------------------------------------- | --------------: | ----------------------------------------------------------------- |
| `TM-029` / Information disclosure            | Logs, telemetry, crash reports, source maps, or diagnostic exports contain conversation content, title-filter queries, keys, headers, paths, or direct identifiers.                                                                           | 4×5=20 Critical | `FR-019`, `SEC-003`, `SEC-013`, `SEC-020`, `SEC-031`, `SEC-039` |  1×5=5 Moderate | Open — sentinel/package/network evidence required                 |
| `TM-030` / Elevation of privilege, tampering | A compromised direct/transitive dependency or lifecycle script changes application behavior or exfiltrates secrets.                                                                                                                           | 4×5=20 Critical | `SEC-033`, `SEC-034`, `SEC-035`, `SEC-036`, `SEC-039`           |     2×5=10 High | Open — audit/provenance evidence required; ecosystem risk remains |
| `TM-031` / Spoofing, tampering               | A malicious or modified installer is presented as the product, or an unsigned engineering artifact is mistaken for a verified release.                                                                                                        | 4×5=20 Critical | `SEC-035`, `SEC-037`, explicit engineering-build labelling      |  1×5=5 Moderate | Open — signature/provenance evidence required                     |
| `TM-032` / Spoofing, tampering               | Automatic update metadata or payload is redirected, rolled back, or modified to install attacker code.                                                                                                                                        | 4×5=20 Critical | `SEC-038`; auto-update disabled until full evidence             |  1×5=5 Moderate | Controlled by absence until signed update design exists           |
| `TM-033` / Elevation of privilege            | Dormant permissions or backend handlers for future tools/files/projects become reachable despite the UI saying `Coming soon`.                                                                                                                 |     3×5=15 High | `SEC-017`, `SEC-021`, `SEC-039`                                 |  1×5=5 Moderate | Open — negative command/capability assertion required             |
| `TM-034` / Repudiation                       | A release is declared secure or complete despite missing/failed scans, Windows tests, SBOM, or signing evidence.                                                                                                                              |     4×4=16 High | `SEC-034`–`SEC-041`, three-state gate reporting                 |       1×4=4 Low | Open — release evidence review required                           |
| `TM-036` / Information disclosure, tampering | A source handoff archive is created from the working directory with an incomplete denylist and silently includes ignored/untracked credentials, databases, exports, signing material, generated binaries, or other private workspace content. | 5×5=25 Critical | `SEC-039`, clean identified commit, tracked-file-only archive   |  1×5=5 Moderate | Open — archive sentinel/inventory evidence required               |

## 7. Misuse cases that must be tested

1. A user submits a prompt containing SQL metacharacters, Unicode edge cases, HTML, custom protocol links, and an extremely long code fence.
2. A fake provider splits UTF-8 and SSE delimiters across arbitrary chunks, sends duplicate/out-of-order events, never terminates a line, then sends a large error body.
3. The user stops immediately before connection, during headers, after the first delta, and concurrently with stream completion.
4. Two conversations attempt to generate concurrently; events are deliberately crossed and replayed.
5. An import contains a credential-looking sentinel, internal paths, prohibited source/local ID fields, unsupported roles, too many messages, excessive nesting, unknown version, and trailing data.
6. Storage fails midway through create, edit/resend, delete, import, migration, and response finalization.
7. A credential-store adapter fails on create, replace, read, and delete; every error/log/UI artifact is scanned for the sentinel key.
8. Native credential-prompt success, cancellation, invalid length, Win32 failure, and credential-store failure exercise every buffer cleanup path; the reviewed FFI site and IPC/DOM/package inventories contain no secret-bearing route.
9. A webview attempts every registered and common unregistered Tauri command from the wrong origin/window.
10. The packaged app is inspected for forbidden remote origins, wildcard capabilities, source maps, secrets, test endpoints, and auto-update configuration.
11. A release artifact is tampered after signing and signature verification must fail.

## 8. Residual risks and user-facing boundaries

Even after all controls pass:

- Z.AI receives the conversation context the user sends and may retain/process it under provider policy (`TM-021`).
- A fully compromised or unlocked Windows user account can read local plaintext history and may inspect process memory (`TM-027`).
- A user can choose to copy and execute unsafe model-generated code; the application prevents automatic execution but cannot guarantee copied content is safe (`TM-009`, `TM-011`).
- Dependency and platform compromise cannot be reduced to zero (`TM-030`).
- Network/provider outages and rate limits remain possible even with bounded recovery.

These risks require accurate notices and must not be obscured by claims such as “private,” “encrypted,” “safe code,” or “secure AI.”

## 9. Review triggers

Review and version this threat model before merging any change that adds or changes:

- a model, provider endpoint, request field, authentication mechanism, or data sent externally;
- a Tauri command, capability, window, protocol, navigation path, or CSP directive;
- Markdown/HTML/diagram rendering or external link behavior;
- database schema, migration, retention, import/export format, or storage location;
- file attachments, project directories, RAG, function calling, tools, process/shell access, voice, telemetry, synchronization, or updates;
- a parser, serializer, runtime dependency, installer mechanism, signing path, or CI trust relationship;
- a security finding, abuse report, or changed platform/provider assumption.

The change must update the threat register, mapped `SEC-` controls, acceptance criteria, and residual-risk statement as applicable.

## 10. Evidence and closure

A threat is not “closed” merely because a control is documented or code exists. It becomes verified for a source revision only when every mapped mandatory acceptance criterion passes in the required environment and the result is retained. Production release policy is defined in [acceptance-criteria.md](acceptance-criteria.md); the process and exception rules are defined in [AGENTS.md](../AGENTS.md).
