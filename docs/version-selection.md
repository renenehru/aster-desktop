# Choosing Between Aster MVP v1 and MVP v2

**Status:** Non-normative developer decision guide

**Last updated:** 2026-07-14

This guide explains the practical differences between the two public Aster
source baselines. It is intended for developers evaluating Aster as a personal
application, a learning reference, or the starting point for an Apache-2.0
fork.

The names **MVP v1** and **MVP v2** identify engineering source baselines. They
do not identify signed production releases. The normative behavior of MVP v2
is defined by the [product specification](product-spec.md),
[security requirements](security-requirements.md), accepted
[architecture decisions](decisions/README.md), and
[acceptance criteria](acceptance-criteria.md). If this guide conflicts with a
normative document, follow the source-of-truth order in
[`AGENTS.md`](../AGENTS.md).

## Short recommendation

- Choose **MVP v1** only when you deliberately want the narrower Z.AI-only
  `glm-5.1` implementation as a historical reference or a smaller product
  surface for a specialized fork.
- Choose **MVP v2** for new Aster development, multiple providers, model
  selection, independently scoped provider credentials, local token Usage,
  advisory budgets, and provider-account navigation.
- Choose **neither as a production binary** when your project requires a signed
  installer, verified automatic updates, a completed clean-profile Windows 11
  acceptance campaign, or a supported service-level commitment. No such Aster
  release exists yet.

MVP v2 is the current development and community-support line. MVP v1 remains
available as an Apache-2.0 source snapshot, but it is historical and is not
promised ongoing fixes.

## Public availability and download status

As of 2026-07-14, the repository has no Git tags, GitHub Releases, or public
release binaries. The links below are commit-pinned GitHub-generated **source
archives**, not installers. Their selected Git tree is fixed by the commit;
their generated archive bytes are not presented as a reproducible artifact hash.

| Source baseline | Version metadata | Exact revision                             | Public source                                                                                                                                                                                                                                                                                                              | Repository status                                                                                                                                   |
| --------------- | ---------------- | ------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| MVP v1          | `0.1.0`          | `39c6cc74f6115b4ba68b474648f8314d8be9f03e` | [Browse](https://github.com/renenehru/aster-desktop/tree/39c6cc74f6115b4ba68b474648f8314d8be9f03e) / [ZIP](https://github.com/renenehru/aster-desktop/archive/39c6cc74f6115b4ba68b474648f8314d8be9f03e.zip) / [tar.gz](https://github.com/renenehru/aster-desktop/archive/39c6cc74f6115b4ba68b474648f8314d8be9f03e.tar.gz) | Apache-2.0-licensed v1 baseline immediately preceding PR #10                                                                                        |
| MVP v2          | `0.2.0`          | `a582ce6ff11d56d5cf1815e5af03fd124cd0144e` | [Browse](https://github.com/renenehru/aster-desktop/tree/a582ce6ff11d56d5cf1815e5af03fd124cd0144e) / [ZIP](https://github.com/renenehru/aster-desktop/archive/a582ce6ff11d56d5cf1815e5af03fd124cd0144e.zip) / [tar.gz](https://github.com/renenehru/aster-desktop/archive/a582ce6ff11d56d5cf1815e5af03fd124cd0144e.tar.gz) | Implementation baseline with both required [PR #10](https://github.com/renenehru/aster-desktop/pull/10) CI checks passing; current development line |

The MVP v2 revision above is the exact implementation baseline that passed the
protected frontend/security and Rust/security checks before this
documentation-only update. The public PR branch can contain newer documentation
commits. Inspect the PR and run `git rev-parse HEAD` before recording evidence
or comparing artifacts.

Do not download an executable or installer from an issue, comment, fork, or
unverified third-party site and assume that it was produced by this repository.
An unsigned locally built executable is an engineering build for evaluation,
not a production release.

## Capability comparison

| Area                                | MVP v1                                                                          | MVP v2                                                                                                                                     |
| ----------------------------------- | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Primary purpose                     | Focused Windows 11 client for Z.AI `glm-5.1`                                    | Curated direct multi-provider Windows 11 client                                                                                            |
| Provider scope                      | Z.AI only                                                                       | Z.AI, DeepSeek, Alibaba Cloud US, Google Gemini, and NVIDIA hosted prototypes                                                              |
| Model catalog                       | One fixed model: `glm-5.1`                                                      | Closed Rust-owned catalog of 17 exact model identifiers                                                                                    |
| User-defined providers or endpoints | Not supported                                                                   | Not supported; unverified entries are omitted rather than shown as unavailable                                                             |
| Model selection                     | No selector; every conversation uses `glm-5.1`                                  | Searchable provider-grouped selector with a default of Z.AI `glm-5.1`                                                                      |
| Conversation identity               | Model is effectively global and fixed                                           | Provider/model pair is stored per conversation and becomes immutable after the first persisted message                                     |
| Changing models                     | Requires changing the implementation contract                                   | Empty conversations may change selection; a non-empty conversation requires a new chat                                                     |
| Provider networking                 | Rust sends directly to one fixed Z.AI HTTPS API                                 | Rust sends directly to one of five fixed provider-hosted HTTPS APIs; there is no aggregator or silent fallback                             |
| Automatic provider retries          | Historical v1 policy allowed a bounded pre-content transient retry              | Exactly one provider attempt per explicit operation; a retry is a new user operation                                                       |
| Credential storage                  | One Rust-owned native prompt and one Windows Credential Manager target for Z.AI | One Rust-owned native prompt and distinct Credential Manager target per provider; the legacy Z.AI target is preserved                      |
| React secret access                 | Prohibited                                                                      | Prohibited; React receives provider-scoped configured/not-configured status only                                                           |
| Response profiles                   | Fast, Standard, and Deep mapped to the verified `glm-5.1` contract              | Same Aster-owned labels with exact model-specific mappings; unsupported mappings must be disabled                                          |
| Local Usage dashboard               | Not included                                                                    | Trailing-seven-day input, cached-input, output, and total-token observations with visible partial coverage                                 |
| Advisory token budget               | Not included                                                                    | Optional per-provider local budget; it never blocks a request and is not provider credit                                                   |
| Low-remaining warning               | Not included                                                                    | Red styling plus text and an icon at exactly 10% remaining or less                                                                         |
| Provider balance                    | Not included                                                                    | Explicit read-only DeepSeek balance refresh only; held in Rust memory for the current session                                              |
| Credits and plans                   | No dedicated workflow                                                           | Fixed provider account actions open the operating system's default browser; Aster never purchases credits or changes plans                 |
| External-processing notice          | General Z.AI notice for the session                                             | Provider- and notice-version-scoped acknowledgement, including the fixed Alibaba Cloud US boundary                                         |
| NVIDIA status                       | Not applicable                                                                  | Hosted prototype entries are labelled for evaluation and are not represented as production NIM deployments                                 |
| SQLite schema                       | Schema v1                                                                       | Schema v2 with provider/model identity, normalized usage observations, provider budgets, and typed finish reasons                          |
| Conversation export                 | Version 1 JSON                                                                  | Version 2 JSON with provider/model and optional normalized per-message usage                                                               |
| Conversation import                 | Strict version 1 JSON                                                           | Strict version 1 and version 2 JSON; v1 content maps to Z.AI `glm-5.1`                                                                     |
| Browser demo                        | Credential-free, in-memory visual QA for the v1 interface                       | Credential-free, in-memory catalog and Usage visual QA; still no native or provider evidence                                               |
| Supply-chain and package evidence   | Initial CI, audits, SBOMs, and engineering build scripts                        | Stronger clean-revision build identity, package provenance fixtures, license policy, retained failure history, and source-archive controls |
| Project support status              | Historical source baseline                                                      | Current development and community-support line                                                                                             |

## Capabilities shared by both versions

MVP v2 extends rather than replaces the core desktop chat lifecycle. Both
baselines include:

- a Windows 11 Tauri desktop shell with a React and TypeScript presentation
  layer and a Rust trust boundary;
- local conversation creation, title filtering, rename, deletion, edit/resend,
  and regeneration;
- streamed assistant content with ordered events and backend cancellation;
- user-scoped SQLite conversation history;
- Rust-owned native credential capture with no API-key field in the webview;
- safe Markdown and code rendering without raw HTML execution;
- bounded native import/export dialogs that do not accept a renderer-supplied
  filesystem path;
- restrictive Tauri permissions and Content Security Policy;
- an isolated browser demo that accepts no credentials and performs no provider
  networking; and
- explicit exclusion of shell execution, arbitrary filesystem access, hidden
  model-invoked tools, attachments, RAG, cloud synchronization, and automatic
  updates.

## MVP v2 model catalog

MVP v2 contains exactly the following pairs. The identifiers are application
contract values, not marketing aliases that may be substituted freely.

| Provider         | Exact model identifiers                                                                         |
| ---------------- | ----------------------------------------------------------------------------------------------- |
| Z.AI             | `glm-4.7`, `glm-5`, `glm-5.1`, `glm-5.2`                                                        |
| DeepSeek         | `deepseek-v4-flash`, `deepseek-v4-pro`                                                          |
| Alibaba Cloud US | `qwen3.5-plus`, `qwen3.5-flash`, `qwen3.6-plus`, `qwen3.6-flash`, `qwen3.7-plus`, `qwen3.7-max` |
| Google Gemini    | `gemini-2.5-flash`, `gemini-2.5-flash-lite`, `gemini-2.5-pro`                                   |
| NVIDIA           | `nvidia/nemotron-3-super-120b-a12b`, `nvidia/nemotron-3-ultra-550b-a55b`                        |

The dated [provider contract](provider-contract.md) records the exact endpoints,
authentication, request shapes, stream rules, usage fields, and response-profile
mappings used by Aster. A documented contract and passing fake-server tests do
not by themselves prove current live-provider compatibility. Provider terms,
model availability, pricing, regions, and account entitlements remain external
to Aster.

## Important design changes for developers

### 1. The provider boundary is a closed registry

MVP v1 is useful when a fork intentionally needs only one fixed Z.AI adapter.
MVP v2 introduces a larger but still closed adapter boundary. React sends typed
provider/model identifiers; Rust owns the origin, path, authorization placement,
request mapping, parser, usage normalization, and allowed account action.

MVP v2 is not an OpenAI-compatible endpoint switcher, plugin host, remote model
discovery client, or arbitrary API proxy. Adding a provider or model is a
specification, privacy, threat-model, contract, migration, and test change.

### 2. Credentials are isolated by provider

MVP v1 stores only the Z.AI key. MVP v2 keeps the compatible Z.AI credential
target and adds a distinct Windows Credential Manager target for each additional
provider. A credential for one provider must never be sent to another.

This matters to forks: a generic `apiKey` field, environment-variable shortcut,
or webview credential form would violate the v2 trust boundary even if it seems
convenient for development.

### 3. Provider/model identity is conversation state

In MVP v2, provider and model are persistent conversation properties. Once the
first message is stored, the pair cannot be mutated. The UI offers to start a
new conversation when the user selects a different pair. This avoids silently
sending existing context to a different provider.

Code that creates, imports, exports, edits, regenerates, cancels, or reconciles a
conversation must preserve the original pair. Integrations written against the
v1 IPC or database shape require adaptation rather than a string-only model
replacement.

### 4. Fast, Standard, and Deep are application profiles

Neither version treats the three profile names as universal quality tiers. In
v1, they map to the one verified `glm-5.1` contract. In v2, Rust applies the
specific mapping recorded for each catalog model. A fork must not copy a field
from one provider to another, silently drop an unsupported field, or substitute
a different model.

### 5. Usage is advisory, not billing

Immediately before an otherwise validated chat reaches provider networking,
MVP v2 creates one local usage observation for that provider attempt. A
pre-network credential, notice, validation, persistence, or cancellation failure
creates no observation. Validated provider metadata is normalized into
non-cached input, cached input, output, and total tokens. Missing data remains
visibly partial. The seven-day window is a local rolling window, not a provider
billing period.

The configurable budget is also local. It does not represent money, quota,
credit, price, authorization, or a request limit. Only DeepSeek has a documented
explicit balance refresh, and that value is not persisted. All purchase and plan
changes remain on the provider's external website.

### 6. Retries are more conservative

The historical v1 specification permitted one bounded automatic retry for
selected transient failures before any response content became visible. MVP v2
does not retry a provider chat or balance operation automatically. An interrupted
request may already have consumed billable tokens, so another attempt must be a
new explicit user operation.

### 7. Security policy covers the expanded boundary

MVP v1 already excluded credentials from React, IPC, SQLite, browser storage,
exports, and logs; fixed the Z.AI origin; disabled raw HTML and remote active
content; and denied shell and arbitrary filesystem access.

MVP v2 adds controls for provider-scoped credentials, a compiled catalog and
account-action map, per-provider disclosures, immutable conversation pairs,
usage and balance validation, provider-specific stream terminal rules, and
clean-source package provenance. These controls add assurance for the expanded
feature set; they do not make an unsigned engineering build a production
release.

## Data migration and compatibility

### Moving from MVP v1 to MVP v2

MVP v2 defines a transactional schema migration for a valid MVP v1 database:

1. Rust creates and verifies a WAL-consistent SQLite backup before a destructive
   schema step.
2. Existing conversations become `zai` / `glm-5.1` conversations.
3. Existing Fast, Standard, and Deep profile values are preserved.
4. A legacy total-token value remains attached to its historical message, but
   unavailable input/cache/output fields remain unknown.
5. Historical totals do not seed the v2 Usage ledger or an advisory budget
   because they are not proven local v2 consumption.
6. Migration failure does not silently replace or delete the original database.

Export important conversations before evaluating a migration and preserve a
separate backup. Do not delete a database merely to hide a migration error.

### Import and export compatibility

- MVP v2 accepts a strict MVP v1 export and maps it to Z.AI `glm-5.1`.
- MVP v2 accepts the strict v2 format with an allowlisted provider/model pair.
- MVP v2 always exports version 2.
- MVP v1 accepts version 1 only and therefore cannot import a v2 export.
- Credentials, aggregate Usage, budgets, balances, local identifiers, and hidden
  reasoning are excluded from both versioned export paths.

### Returning to MVP v1

There is no supported in-place downgrade from a schema-v2 database to MVP v1.
Do not run MVP v1 against a database that MVP v2 has migrated. To evaluate v1
again, use a separately preserved pre-migration v1 data set or an isolated clean
test environment. Reinstalling older source is not a data rollback strategy.

## Which version fits a personal project?

### Choose MVP v1 when

- the project deliberately targets only Z.AI `glm-5.1`;
- a fixed single-provider adapter is more useful than a catalog abstraction;
- Usage, balance, account navigation, and model-selection UI are unnecessary;
- the code is being studied as a historical example rather than adopted as the
  supported Aster development line; and
- the maintainer accepts responsibility for backporting future security and
  compatibility fixes.

A narrower feature set is not proof that v1 is safer, more stable, or more
verified. It simply contains fewer product concepts.

### Choose MVP v2 when

- the project needs any supported provider other than Z.AI;
- users need a curated model selector without arbitrary endpoint entry;
- provider credentials must remain independently scoped;
- provider/model identity must be preserved per conversation;
- local token observations and an advisory budget are useful;
- DeepSeek balance or fixed external account actions are required;
- v1 conversation migration and v1/v2 import compatibility matter; or
- the project should follow the current Aster specification, tests, and
  community-maintained line.

### Choose another foundation when

- the project requires macOS, Linux, a web-hosted credential architecture, or a
  mobile client;
- users must enter arbitrary endpoints or model identifiers;
- the application needs attachments, RAG, agent tools, shell execution, cloud
  synchronization, or multi-user access control;
- provider aggregation, automatic model discovery, cost forecasting, or in-app
  billing is a core requirement; or
- a signed, supported, production-ready installer is required immediately.

## Obtain a reproducible source checkout

### MVP v1

```powershell
git clone https://github.com/renenehru/aster-desktop.git aster-desktop-mvp-v1
Set-Location aster-desktop-mvp-v1
git switch --detach 39c6cc74f6115b4ba68b474648f8314d8be9f03e
git rev-parse HEAD
```

The final command must print
`39c6cc74f6115b4ba68b474648f8314d8be9f03e`.

### MVP v2 implementation baseline

```powershell
git clone https://github.com/renenehru/aster-desktop.git aster-desktop-mvp-v2
Set-Location aster-desktop-mvp-v2
git fetch origin refs/pull/10/head:mvp-v2-pr
git switch --detach a582ce6ff11d56d5cf1815e5af03fd124cd0144e
git rev-parse HEAD
```

The final command must print
`a582ce6ff11d56d5cf1815e5af03fd124cd0144e`.

Fetching the GitHub pull-request ref makes the pre-squash implementation commit
reachable even if its feature branch is later deleted. To review documentation
commits newer than that implementation baseline, inspect
[PR #10](https://github.com/renenehru/aster-desktop/pull/10) and its public
`feature/mvp-v2-multi-provider` branch while that branch exists. A branch is a
moving development reference; record its exact commit before testing. After the
PR merges, `main` will become the normal starting point for current v2
development.

For either checkout, follow that revision's `README.md` and development guide,
install the exact locked dependencies, and run its documented verification
commands. Do not apply v2 commands or evidence claims retroactively to v1.

## Reuse and license obligations

The Apache-2.0-licensed MVP v1 baseline is the exact revision listed above. The
original earlier v0.1.0 engineering snapshot predated selection of an
open-source license and should not be used as the public reuse baseline.

For both baselines, project-owned source code, documentation, configuration, and
assets are licensed under the [Apache License 2.0](../LICENSE), except where an
item is explicitly identified as separately licensed. Apache-2.0 permits use,
modification, and redistribution subject to its conditions, including providing
the license, preserving applicable notices, and identifying modified files. It
does not grant Aster trademark rights and provides the work without warranty.

Third-party dependencies and separately licensed material retain their own
licenses. Review [`NOTICE`](../NOTICE), both lockfiles, package metadata, and
generated SBOM/license evidence before redistribution. Do not describe every
third-party component as relicensed under Apache-2.0.

The npm package is marked private and the Rust crates are marked `publish =
false`. Aster is a desktop application source repository, not a stable npm or
crates.io SDK. Forks may reuse code under the applicable licenses, but internal
React, IPC, database, and Rust adapter interfaces can change between MVP
versions.

## Verification and release caveats

- A browser preview proves only the visible browser-demo state.
- Unit, component, or contract tests do not prove live provider availability,
  native credential storage, packaged migration, or installer behavior.
- Provider documentation review does not prove that a live API still accepts a
  request today.
- Local Usage is not provider billing evidence.
- Source publication does not approve a binary release.
- A version string, source archive, passing CI run, tag, or successful local
  build is not by itself production-release approval.
- Unsigned executables and installers remain engineering artifacts for local
  evaluation.

Consult the [evidence policy](evidence/README.md) and
[release process](release-process.md) before making a security, compatibility,
or distribution claim.

## Further reading

- [MVP v2 product specification](product-spec.md)
- [MVP v2 architecture](architecture.md)
- [MVP v2 provider contract](provider-contract.md)
- [Security requirements](security-requirements.md)
- [Acceptance criteria and traceability](acceptance-criteria.md)
- [Development guide](development.md)
- [Testing guide](testing.md)
- [Release process](release-process.md)
- [Changelog](../CHANGELOG.md)
