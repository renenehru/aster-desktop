# Changelog

All notable project changes are recorded in this file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html) where applicable.

## [Unreleased]

### Documentation

- Added a developer-focused MVP v1 versus MVP v2 selection guide with an exact
  capability matrix, commit-pinned source links and archives, migration and downgrade
  guidance, support status, release caveats, and Apache-2.0 reuse obligations.

## [0.2.0] - 2026-07-13

### Added

- Closed, Rust-owned MVP v2 catalog with 5 direct providers and 17 documented
  provider/model pairs.
- Provider/model selector with an immutable conversation pair after the first
  persisted message; changing the selection starts a new conversation.
- Separate Rust-owned native API-key setup and Windows Credential Manager target
  for every provider.
- Model-specific Fast, Standard, and Deep mappings backed by the dated provider
  contract rather than a universal reasoning-level assumption.
- Local seven-day token Usage view with normalized input, cached-input, output,
  and total counts, visible partial-coverage state, and per-provider advisory
  budgets.
- Accessible red warning with text and icon when a configured budget has 10% or
  less remaining.
- Explicit read-only DeepSeek balance refresh and typed provider account actions
  that open fixed official pages in the default browser.
- Transactional schema-v2 migration that retains MVP v1 conversations as
  `zai` / `glm-5.1` and preserves historical message totals without seeding the
  v2 usage ledger.
- Professional multi-laptop contributor onboarding, governance, support,
  testing, release, troubleshooting, and roadmap documentation.

### Changed

- Replaced the Z.AI-only product boundary with a curated direct-provider
  architecture. React remains a presentation layer and cannot select an origin,
  carry a key, or call a provider.
- Alibaba Cloud requests use a fixed US-region contract with an explicit user
  disclosure. NVIDIA catalog entries are labelled hosted prototypes for
  evaluation rather than production deployments.
- Provider requests use one attempt per explicit operation; Aster performs no
  automatic retry that could hide duplicate token consumption.
- Browser demo behavior now models catalog and Usage interactions in memory
  while remaining visibly isolated from credentials, native storage, and all
  external networking.
- Project-owned source code, documentation, configuration, and assets are
  available under Apache License 2.0, with separately licensed material retained
  under its original terms and package/SBOM metadata aligned.
- Rust CI installs locked frontend dependencies and builds production web assets
  before invoking Tauri macros, avoiding dependence on an untracked local
  `dist/` directory.

### Security

- Provider credentials are scoped to distinct native credential targets; a key
  for one provider cannot be reused for another through application IPC.
- Provider/model identifiers, profile mappings, inference origins, balance
  operations, and account URLs are fixed Rust-owned policy and are validated at
  the command boundary.
- Local Usage is advisory and cannot authorize, block, purchase, or change a
  provider plan. DeepSeek balance data is not persisted.
- CI actions require reviewed full-SHA pins and read-only workflow permissions.
  Rust dependency auditing uses the reviewed `cargo-audit` release without
  granting a third-party action a GitHub token or write permission.
- Source handoff archives require a clean identified commit and tracked-file-only
  `git archive` generation, preventing ignored or untracked workspace data from
  entering a release through an incomplete denylist.
- Environment configuration explicitly rejects credentials, provider overrides,
  certificate overrides, balance data, and diagnostic settings.

### Verification status

- Version `0.2.0` does not by itself assert live compatibility with any provider,
  clean-profile Windows acceptance, complete package inventory, Authenticode
  signing, or production readiness. Consult revision-specific evidence for exact
  `PASS`, `FAIL`, and `NOT RUN` outcomes.

## [0.1.0] - 2026-07-12

### Added

- Original Windows 11 desktop shell and in-memory browser demo.
- Local conversation creation, title search, rename, deletion, edit/resend, and
  regeneration.
- Streamed Z.AI chat through the Rust backend with ordered events and
  cancellation.
- Fast, Standard, and Deep application response profiles pinned to the verified
  `glm-5.1` contract.
- Rust-owned native Windows credential prompt and Credential Manager integration.
- User-scoped SQLite persistence with transactional repository behavior.
- Safe Markdown, code-copy feedback, and scoped external HTTPS navigation.
- Versioned, bounded conversation import and plaintext JSON export through native
  dialogs.
- Raw, bounded application IPC and least-privilege Tauri capabilities.
- Product specification, architecture, threat model, security requirements,
  acceptance criteria, provider contract, ADRs, and evidence policy.
- Frontend and Rust tests, coverage thresholds, dependency audits, license policy
  checks, secret/configuration scans, SBOM generation, and Windows engineering
  build scripts.

### Security

- API keys were excluded from the webview, application IPC, SQLite, browser
  storage, exports, fixtures, logs, and packaged assets.
- Provider traffic was fixed to the documented Z.AI HTTPS origin with platform
  certificate validation.
- Raw HTML, remote active content, wildcard permissions, arbitrary filesystem
  access, and shell execution were outside the MVP boundary.

### Known limitations

- The executable and installer were unsigned engineering artifacts.
- The formal clean-profile Windows 11 E2E, native credential, fake TLS/SSE,
  accessibility/scaling, performance, package inventory, provenance, and signing
  criteria were not verified in the historical snapshot.
- No open-source license had been selected for the original `0.1.0` engineering
  snapshot; Apache-2.0 was adopted afterward.
