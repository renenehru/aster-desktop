# Contributing to Aster Desktop

Thank you for helping improve Aster. The project follows Spec-Driven Development and SecDevOps. Read [AGENTS.md](AGENTS.md) before changing code, configuration, dependencies, permissions, documentation, or evidence.

## Before you begin

1. Read the [documentation index](docs/README.md), [product specification](docs/product-spec.md), and [architecture](docs/architecture.md).
2. Review open issues and pull requests to avoid duplicate work.
3. Use a dedicated branch from the current `main` branch.
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
- Do not introduce remote active content, arbitrary shell execution, broad Tauri permissions, certificate bypasses, or caller-controlled provider origins.
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
- Every `FAIL` and required `NOT RUN` outcome.

At least one maintainer review is expected before merge. Security-sensitive changes require explicit security review. Merge only after required checks pass and specification drift has been resolved.

## Licensing

The repository currently has no open-source license. Do not redistribute project source or binaries, and confirm contribution and licensing terms with the project owner before contributing code intended for public distribution.
