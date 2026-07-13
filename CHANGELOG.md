# Changelog

All notable project changes are recorded in this file. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html) where applicable.

## [Unreleased]

### Added

- Professional contributor onboarding, governance, support, issue, pull-request, testing, release, troubleshooting, and roadmap documentation.

### Changed

- Rust CI now installs the locked frontend dependencies and builds the production web assets before invoking Tauri macros, so clean runners do not depend on an untracked local `dist/` directory.
- Repository publication and collaboration guidance now distinguishes source publication from unsigned engineering artifacts.

### Security

- CI actions now require reviewed full-SHA pins with hosted-runner-supported runtimes. Rust dependency auditing installs the reviewed `cargo-audit` 0.22.2 release with upstream lock metadata and does not grant a third-party action a GitHub token or write permission.
- Source handoff archives now require a clean identified Git commit and tracked-file-only `git archive` generation, preventing ignored or untracked workspace data from entering a release through an incomplete denylist.
- Removed the unsupported `RUST_LOG` environment example and added a configuration regression assertion so secrets, endpoints, certificate overrides, and diagnostic settings are not advertised through environment files.

## [0.1.0] - 2026-07-12

### Added

- Original Windows 11 desktop shell and in-memory browser demo.
- Local conversation creation, title search, rename, deletion, edit/resend, and regeneration.
- Streamed Z.AI chat through the Rust backend with ordered events and cancellation.
- Fast, Standard, and Deep application response profiles pinned to the verified `glm-5.1` contract.
- Rust-owned native Windows credential prompt and Credential Manager integration.
- User-scoped SQLite persistence with transactional repository behavior.
- Safe Markdown, code-copy feedback, and scoped external HTTPS navigation.
- Versioned, bounded conversation import and plaintext JSON export through native dialogs.
- Raw, bounded application IPC and least-privilege Tauri capabilities.
- Product specification, architecture, threat model, security requirements, acceptance criteria, provider contract, ADRs, and evidence policy.
- Frontend and Rust tests, coverage thresholds, dependency audits, license policy checks, secret/configuration scans, SBOM generation, and Windows engineering build scripts.

### Security

- API keys are excluded from the webview, application IPC, SQLite, browser storage, exports, fixtures, logs, and packaged assets.
- Provider traffic is fixed to the documented Z.AI HTTPS origin with platform certificate validation.
- Raw HTML, remote active content, wildcard permissions, arbitrary filesystem access, and shell execution are outside the MVP boundary.

### Known limitations

- The executable and installer are unsigned engineering artifacts.
- The formal clean-profile Windows 11 E2E, native credential, fake TLS/SSE, accessibility/scaling, performance, packaging-inventory, provenance, and signing criteria remain unverified as recorded in the evidence report.
- No open-source license has been selected.
