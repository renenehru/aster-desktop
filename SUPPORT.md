# Aster Desktop Support

Aster Desktop is currently an MVP v2 engineering implementation for Windows 11. Support is community-based and provided on a best-effort basis; no response time, compatibility, uptime, or service-level commitment is promised.

## Version support

MVP v2 is the current development and community-support line. The Apache-2.0
MVP v1 source baseline remains available for reproducibility and personal forks,
but it is historical and is not promised ongoing compatibility or security
backports. Use the [version selection guide](docs/version-selection.md) to compare
capabilities, obtain the exact source snapshots, and review migration and
downgrade limits before requesting support.

Neither source baseline is a signed production release. Support applies only to
the scope actually documented and verified for the exact revision reported by
the requester.

## Before requesting help

1. Read [README.md](README.md) for prerequisites, setup, and known scope limits.
2. Review [docs/product-spec.md](docs/product-spec.md) to confirm that the behavior is part of MVP v2.
3. Search existing issues for the same symptom or proposal.
4. Reproduce the issue using the latest source revision you can safely test.
5. Remove API keys, authorization data, personal conversations, local paths, and other sensitive content from all evidence.

The browser-only demo supports visual development and uses in-memory data. It is not evidence for desktop IPC, credential storage, provider networking, SQLite persistence, native dialogs, installation, or signing.

## Where to ask

- **Defect in supported behavior**: use the Bug report issue form.
- **Proposed product behavior**: use the Feature request form. A proposal may require specification, threat-model, acceptance-criteria, or ADR updates before implementation.
- **Documentation problem**: use the Documentation issue form.
- **Security vulnerability**: do not open a public issue. Follow [SECURITY.md](SECURITY.md).
- **Conduct concern**: follow [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

General questions may be opened only when they do not contain sensitive information. Maintainers may redirect support requests, close duplicates, or decline requests outside the documented product boundary.

## Information to provide

For a reproducible support request, include:

- The exact source revision, version, or engineering artifact name.
- Whether the working tree was modified.
- Windows edition, version, build, architecture, display scale, and WebView2 version when relevant.
- The exact command or deterministic steps that produced the issue.
- Expected and actual behavior.
- A minimal reproduction using synthetic, non-sensitive data.
- Stable Aster error codes and safe metadata, if displayed.
- The applicable requirement or acceptance-criterion identifiers, when known.

Screenshots and logs must be reviewed before upload. Do not include credentials, prompts, responses, exported conversations, database rows, authorization headers, or private filesystem paths. A screenshot demonstrates only what is visible in that captured state.

## Supported scope

The documented MVP targets supported 64-bit Windows 11 editions with Microsoft Edge WebView2 and the toolchain versions described in `README.md`. Projects, document attachments, voice, RAG, tool execution, arbitrary filesystem or shell access, cloud synchronization, and automatic updates are outside MVP v2.

Provider availability, model output quality, account status, billing, retention, and service limits are controlled by each selected provider. Do not share a provider API key with maintainers. Credential changes must use Aster's Rust-owned native Windows prompt. Aster opens fixed official account pages in the operating system's default browser, but never purchases credits or changes a plan.

## Engineering builds

Unless a release record proves otherwise, locally built or unsigned packages are engineering builds. They must not be represented as signed, production-ready, or fully verified. Check `docs/evidence/` for revision-specific gate outcomes and artifact hashes before relying on a build.

## Collaboration help

Development setup and contribution expectations are documented in [CONTRIBUTING.md](CONTRIBUTING.md), [GOVERNANCE.md](GOVERNANCE.md), and [AGENTS.md](AGENTS.md). Keep pull requests focused and use the pull-request template so reviewers can evaluate requirements, threats, tests, and residual risk efficiently.
