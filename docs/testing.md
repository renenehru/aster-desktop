# Testing and Verification Guide

Aster uses layered verification. Each layer proves only the boundary it
actually exercises; no lower-layer result is a substitute for packaged Windows
end-to-end evidence. The normative scenarios and traceability live in
[acceptance-criteria.md](acceptance-criteria.md), and result recording follows
the [evidence policy](evidence/README.md).

## Test safety rules

- Automated tests must not use a real API key, a real user credential-store
  target, a real user database, personal conversation content, or a billable
  provider endpoint.
- Provider integration tests use controlled fixtures or a fake SSE server unless
  an explicitly authorized, secret-scoped end-to-end job is selected.
- Test errors and retained artifacts must not contain prompts, responses,
  authorization headers, credential values, provider bodies, database rows, or
  personal paths.
- Any fixed vulnerability receives a focused security regression test.
- A test result is `PASS`, `FAIL`, or `NOT RUN`; incomplete and ambiguous runs
  are `FAIL`, while missing tools or evidence are `NOT RUN`.

## Verification layers

| Layer                          | Current repository coverage                                                                                                       | What it does not prove by itself                                                           |
| ------------------------------ | --------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Frontend unit/component        | React state, dialogs, keyboard/focus behavior, safe Markdown, browser-demo behavior                                               | Rust, SQLite, Credential Manager, native dialogs, provider transport, package behavior     |
| Frontend desktop-adapter tests | IPC command mapping, raw-body size checks, response/event validation, UI reconciliation using mocks                               | A running Tauri process or native IPC authorization                                        |
| Rust unit/repository/contract  | Validators, SQLite repository behavior, import rules, provider request/SSE logic, cancellation units, credential input boundaries | Packaged UI, real Credential Manager, production provider availability, installer behavior |
| Static security/configuration  | CSP, capabilities, prohibited renderer APIs, secret patterns, unsafe-code confinement, dependency policy                          | Runtime behavior beyond the inspected source/configuration                                 |
| Browser visual QA              | Demo layout and the exact interactions recorded                                                                                   | Any native desktop, persistence, provider, credential, or package boundary                 |
| Packaged Windows E2E           | Only the exact artifact, profile, scale, and procedure recorded                                                                   | Signing or provenance unless separately verified                                           |

## Frontend tests

Run the deterministic frontend suite once:

```powershell
pnpm test:run
```

Run in watch mode while developing:

```powershell
pnpm test
```

Run one suite:

```powershell
pnpm exec vitest run src/components/Markdown.test.tsx
```

Generate coverage and enforce the configured thresholds:

```powershell
pnpm test:coverage
```

`vite.config.ts` currently requires at least 70% lines, functions, and
statements and 60% branches. Reports are written to the ignored `coverage/`
directory. Coverage percentage is not acceptance evidence for an unexercised
trust boundary.

The frontend suites are organized as follows:

| Suite                                         | Primary scope                                                                                       |
| --------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| `src/App.test.tsx`                            | Browser-demo workflows and visible isolation                                                        |
| `src/App.tauri.test.tsx`                      | Desktop UI reconciliation with a mocked adapter                                                     |
| `src/components/Dialog.test.tsx`              | Focus, Escape, backdrop, and modal behavior                                                         |
| `src/components/Markdown.test.tsx`            | Inert Markdown, unsafe-link rejection, streaming, and plain-text copy                               |
| `src/services/assistantAdapter.test.ts`       | In-memory adapter CRUD, Unicode streaming, cancellation, regeneration, and import/export validation |
| `src/services/assistantAdapter.tauri.test.ts` | Typed IPC mapping and hostile result/event validation                                               |
| `src/security-boundaries.test.ts`             | Static renderer and credential-permission assertions                                                |

## Rust tests

Run all locked workspace tests:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --workspace --all-targets --all-features --locked
```

Run the compiler and lint gates used by CI:

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo check --manifest-path src-tauri/Cargo.toml --workspace --all-targets --all-features --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --all-features --locked -- -D warnings
```

Rust tests live beside the implementation in the main crate and isolated
credential-prompt crate. They exercise model validation, database operations,
import transactions, request mapping, SSE parser behavior, cancellation logic,
external URL policy, IPC validation, and native-prompt input boundaries with
controlled inputs. They must remain independent of the real credential store,
real application database, and production provider.

## Frontend quality and security gates

`pnpm check` is the normal source-change gate:

```powershell
pnpm check
```

It runs linting with zero warnings, formatting check, strict TypeScript checking,
the deterministic frontend tests, repository secret scanning, security
configuration assertions, and the production frontend build.

Run individual security and supply-chain checks when investigating a failure:

```powershell
pnpm security:secrets
pnpm security:config
pnpm audit:frontend
pnpm audit:rust
pnpm license:frontend
pnpm license:rust
pnpm sbom:frontend
pnpm sbom:rust
```

The license-policy commands assert that the npm project and every Rust workspace
package declare `Apache-2.0`, then evaluate third-party dependency expressions
against the approved policy. The SBOM scripts write CycloneDX JSON to
`work/sbom-frontend.cdx.json` and `work/sbom-rust.cdx.json`, including the root
application license and manifest-reported dependency licenses. Those files are
dependency-manifest inventories; do not describe them as a binary-derived
inventory of every packaged native component or as complete redistribution
compliance evidence.

## Aggregate local verification

The Windows wrapper installs locked frontend dependencies, runs frontend checks
and coverage, audits both ecosystems, enforces both license policies, generates
both SBOMs, and then runs Rust formatting, check, Clippy, tests, and audit:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/verify.ps1
```

The optional frontend-only mode is useful when Cargo is unavailable:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/verify.ps1 -SkipRust
```

Record the Rust gates as `NOT RUN` when `-SkipRust` is used. A successful
frontend-only invocation is not a successful aggregate or release verification.
The Rust-enabled script requires `cargo audit` to be available.

## Package audit

After an engineering build, inspect the release executable, NSIS installer, and
production frontend bundle:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/package-audit.ps1
```

The script checks its stated byte-pattern and bundle scope, including source
maps, development endpoints, private-key/header/test sentinels, the current
builder profile path, and unsafe packaged configuration patterns. It does not
install or unpack the NSIS installer, execute the application, inspect Windows
Credential Manager, reconcile a binary-derived SBOM, or prove signing.

## Continuous integration

`.github/workflows/ci.yml` is configured for pull requests and pushes to
`main`. Its two Windows jobs run:

- frontend lint, formatting, coverage, type check, production build, secret and
  configuration checks, dependency audit, license policy, and SBOM generation;
- Rust formatting, Clippy with warnings denied, all-target check and tests,
  RustSec audit, license policy, and SBOM generation.

Actions are commit-SHA pinned, workflow permissions are read-only, and each SBOM
is configured as a retained CI artifact. Workflow configuration is not evidence
that a particular revision passed; link the actual run and artifacts in the
revision-specific evidence record.

## Adding or changing tests

1. Map the test to a stable requirement and acceptance criterion.
2. Put pure UI and adapter behavior in a focused frontend suite; put backend
   validation and persistence near the Rust implementation.
3. Include normal, boundary, failure, cancellation, and abuse paths appropriate
   to the change.
4. Use deterministic time, IDs, temporary databases, fixtures, and fake
   transports where the boundary permits.
5. Assert safe machine error codes and user outcomes, never sensitive payload
   text.
6. If automation cannot exercise an OS boundary, define exact manual steps,
   environment, sentinels, expected results, and artifact identity in the
   acceptance/evidence procedure.

## Recording evidence

Running a command does not automatically create release evidence. A compliant
record under `docs/evidence/` identifies the source revision and clean/dirty
state, exact command or procedure, environment, UTC start/end, reviewer or CI
identity, evidence location, result, and artifact SHA-256 when an artifact was
tested. Preserve superseded failures and retries. Never broaden the recorded
scope beyond the assertions that actually ran.
