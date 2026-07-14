# Contributing to Aster Desktop

Thank you for helping improve Aster. The project follows Spec-Driven Development and SecDevOps. Read [AGENTS.md](AGENTS.md) before changing code, configuration, dependencies, permissions, documentation, or evidence.

## Before you begin

1. Read the [documentation index](docs/README.md), [product specification](docs/product-spec.md), and [architecture](docs/architecture.md).
2. Review open issues and pull requests to avoid duplicate work.
3. Synchronize the current `main` branch with `git pull --ff-only`, then use a
   dedicated branch.
4. Report suspected vulnerabilities privately under [SECURITY.md](SECURITY.md); do not open a public issue with exploit details.

Recommended branch names:

- `feature/<short-name>`
- `fix/<short-name>`
- `security/<short-name>`
- `docs/<short-name>`
- `maintenance/<short-name>`

## Set up the project

Follow [docs/development.md](docs/development.md) for the supported Windows toolchain and fresh-laptop setup.

```powershell
pnpm install --frozen-lockfile
powershell -ExecutionPolicy Bypass -File scripts/verify.ps1
```

Never use a real provider credential in tests. Default test and browser-demo paths must not call a billable endpoint or touch the user's real credential target or database.

For work across laptops, commit and push the feature branch before changing
machines, then fetch and check out that branch on the next laptop. Do not copy or
commit `node_modules`, Cargo output, application databases, conversation exports,
provider credentials, balance information, signing material, or private
evidence. Each Windows account has independent Credential Manager entries and
local conversation data.

## Required change workflow

For any behavior, data-flow, dependency, permission, or security-sensitive change:

1. **Specify** the intended behavior and non-goals with stable `PR-`, `FR-`, `UX-`, `NFR-`, or `SEC-` identifiers.
2. **Model** changes to actors, assets, data flows, permissions, parsers, persistence, or external services in the architecture and threat model.
3. **Decide** difficult-to-reverse or trust-boundary changes with an ADR.
4. **Trace** every normative requirement to an `AC-` criterion and verification method.
5. **Test first** when automation is practical; otherwise document a deterministic manual procedure and why automation is unavailable.
6. **Implement minimally** without dormant permissions or speculative privileged APIs.
7. **Verify** the applicable gates and retain evidence with exact scope.
8. **Review drift** across specification, code, tests, configuration, and documentation.

Documentation-only changes should state that they are non-behavioral and why no requirement or threat-model update applies.

## Engineering rules

- Write source, UI text, tests, documentation, configuration, and commit messages in English.
- Keep TypeScript strict and avoid `any` at application boundaries.
- Keep Rust free of warnings; `unsafe` requires an ADR, safety invariant, focused tests, and security review.
- Use parameterized SQL and transactional repository operations.
- Treat model output, Markdown, imports, IPC arguments, events, database content, and provider responses as untrusted.
- Keep the catalog closed to the exact 5 providers and 17 model IDs in the dated
  provider contract. Do not add free-form, remotely discovered, placeholder, or
  unavailable entries.
- Keep one native Windows credential target per provider. A key must never enter
  React, IPC, SQLite, logs, fixtures, environment files, or another provider's
  authorization header.
- Preserve the provider/model conversation lock after the first persisted
  message. A different pair starts a new chat; context is not silently moved.
- Treat local seven-day Usage and its configurable budget as advisory. Preserve
  visible partial coverage and the red-plus-text/icon warning at 10% remaining
  or below; never describe it as billing or use it to authorize or block a send.
- Keep balance refresh explicit and DeepSeek-only. Account management accepts a
  typed provider/action pair and opens a fixed official page in the default
  browser; Aster never buys credits or changes a plan.
- Do not introduce remote active content, automatic provider retries, arbitrary
  shell execution, broad Tauri permissions, certificate bypasses,
  caller-controlled provider origins, or caller-controlled account URLs.
- Pin dependency versions exactly and review both lockfiles.
- Never commit secrets, personal conversations, local databases, exports, generated installers, signing material, or diagnostic output containing sensitive data.

## Tests and evidence

Use [docs/testing.md](docs/testing.md) to select the correct verification layer. At minimum, run:

```powershell
pnpm check
powershell -ExecutionPolicy Bypass -File scripts/verify.ps1
```

A pull request must report each applicable gate as `PASS`, `FAIL`, or `NOT RUN`. A result is valid only for the exact source, command, environment, time, and artifact exercised. Do not use a browser demo or screenshot as evidence for desktop IPC, Windows Credential Manager, SQLite restart persistence, provider networking, native dialogs, installer behavior, or signing.

## Commit guidance

Keep commits focused and reviewable. Use an imperative English subject; a Conventional Commits-style prefix is recommended:

```text
feat: add bounded conversation-title filtering
fix: reject stale stream events after cancellation
docs: clarify native credential verification
security: narrow the Tauri capability surface
```

Do not combine unrelated formatting, dependency, product, and security changes in one commit.

## Pull requests

Complete the repository pull-request template. The description must include:

- Requirement IDs changed or a non-behavioral justification.
- Threat IDs reviewed and any changed residual risk.
- Architecture and ADR impact.
- Exact verification commands and evidence locations.
- Migration, compatibility, privacy, and rollback considerations.
- Provider-contract/catalog impact, including Alibaba Cloud's fixed US-region
  boundary and NVIDIA's hosted-prototype classification when applicable.
- Every `FAIL` and required `NOT RUN` outcome.

At least one maintainer review is expected before merge. Security-sensitive changes require explicit security review. Merge only after required checks pass and specification drift has been resolved.

## Licensing

Except where otherwise noted, the project-owned work is licensed under the [Apache License 2.0](LICENSE). Under section 5 of that license, a contribution intentionally submitted for inclusion in Aster Desktop is provided under Apache-2.0 without additional terms unless the contributor explicitly states otherwise or a separate written agreement applies. No copyright assignment is required.

By submitting a contribution, you confirm that you have the right to provide it under those terms. Identify copied, generated, or third-party material and preserve its provenance, license, copyright, and attribution requirements. Do not submit material with terms that are incompatible with the repository or that you are not authorized to license. See [NOTICE](NOTICE) for the Contributor Covenant exception and existing third-party attribution.
