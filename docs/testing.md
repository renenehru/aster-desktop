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
- A default test run must not contact Z.AI, DeepSeek, Alibaba Cloud, Google, or
  NVIDIA. Live compatibility is a separate, explicitly authorized result and is
  never inferred from a contract fixture.
- Test errors and retained artifacts must not contain prompts, responses,
  authorization headers, credential values, provider bodies, database rows, or
  personal paths.
- Any fixed vulnerability receives a focused security regression test.
- A test result is `PASS`, `FAIL`, or `NOT RUN`; incomplete and ambiguous runs
  are `FAIL`, while missing tools or evidence are `NOT RUN`.

## Verification layers

| Layer                          | Current repository coverage                                                                                                | What it does not prove by itself                                                       |
| ------------------------------ | -------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| Frontend unit/component        | React state, catalog/model lock, Usage thresholds, dialogs, keyboard/focus behavior, safe Markdown, browser-demo isolation | Rust, SQLite, Credential Manager, native dialogs, provider transport, package behavior |
| Frontend desktop-adapter tests | IPC command mapping, raw-body size checks, response/event validation, UI reconciliation using mocks                        | A running Tauri process or native IPC authorization                                    |
| Rust unit/repository/contract  | Catalog validation, schema migration, usage normalization, provider request/SSE logic, cancellation, credential boundaries | Packaged UI, real Credential Manager, live provider availability, installer behavior   |
| Static security/configuration  | CSP, capabilities, prohibited renderer APIs, secret patterns, unsafe-code confinement, dependency policy                   | Runtime behavior beyond the inspected source/configuration                             |
| Browser visual QA              | Demo layout and the exact interactions recorded                                                                            | Any native desktop, persistence, provider, credential, or package boundary             |
| Packaged Windows E2E           | Only the exact artifact, profile, scale, and procedure recorded                                                            | Signing or provenance unless separately verified                                       |

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
| `src/App.test.tsx`                            | Browser-demo workflows, catalog selection, model lock, Usage alerts, and visible isolation          |
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
credential-prompt crate. They exercise the exact 5-provider/17-model catalog,
model-specific profile mapping, provider-scoped credential selection, database
operations and v1 migration, usage normalization and seven-day aggregation,
DeepSeek balance validation, import transactions, request mapping, SSE parser
behavior, one-attempt transport, cancellation logic, fixed account-action URL
policy, IPC validation, and native-prompt input boundaries with controlled
inputs. They must remain independent of the real credential store, real
application database, and live provider APIs.

Catalog tests should assert both inclusion and exclusion: only the exact pairs in
[provider-contract.md](provider-contract.md) may be returned, and no placeholder,
remote discovery result, or caller-supplied model may enter a request. Profile
contract tests must prove the exact wire mapping for every supported pair rather
than assuming that Fast, Standard, or Deep means the same provider field.

Usage tests use a fixed clock and cover the trailing seven-day boundary,
non-cached versus cached input, output and total invariants, partial metadata,
duplicate terminal events, cancellation/failure observations, numeric limits,
per-provider isolation, budget clear/update, and the `<= 10%` warning boundary.
DeepSeek balance tests use a fake bounded response and prove no persistence or
background polling. Account-action tests accept only registered provider/action
pairs and prove that React cannot supply a URL.

## Frontend quality and security gates

`pnpm check` is the normal source-change gate:

```powershell
pnpm check
```

It runs linting with zero warnings, formatting check, strict TypeScript checking,
the deterministic frontend tests, positive/negative build-identity fixtures,
temporary-repository package-provenance abuse fixtures, shared secret-pattern
fixtures, repository secret scanning, security configuration assertions, and
the production frontend build.

Run individual security and supply-chain checks when investigating a failure:

```powershell
pnpm security:build-identity
pnpm security:package-audit
pnpm security:package-provenance
pnpm security:patterns
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

Neither the aggregate script nor CI should call a live provider. Passing these
gates demonstrates deterministic source and controlled-contract behavior only;
it does not establish that current credentials, quotas, model availability, or
provider production services work.

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

`pnpm security:package-provenance` creates isolated temporary Git repositories
and executes the package workflow against positive and negative cases. It covers
dirty/wrong revisions, duplicate/wrong-type/fractional manifest values, stale or
post-copy-tampered inputs, malformed/duplicated/indented evidence, missing or
invalid/oversized/sensitive retained logs, junctions, an existing output, exact
Git ZIP directory/file/mode inventory, final-state mismatch, a destination-
junction race, and a separate locked-child failure immediately before atomic
publication. All validation precedes the final rename. The fixture removes only
verified paths inside its unique system-temporary sandbox and never calls a live
provider.

`pnpm security:package-audit` proves that binary credential patterns are found
when UTF-16LE or UTF-16BE text starts at either byte alignment. This closes the
odd-offset blind spot without treating that byte scan as general binary
decompilation or runtime evidence.

## Continuous integration

`.github/workflows/ci.yml` is configured for pull requests and pushes to
`main`. Its two Windows jobs run:

- frontend lint, formatting, coverage, type check, production build,
  clean-revision build-identity, binary package-audit alignment,
  package-provenance abuse, and shared secret-pattern fixtures, repository
  secret and configuration checks, dependency audit, license policy, and SBOM
  generation;
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
ignored draft under `work/evidence/`, reviewed durable record under
`docs/evidence/`, or protected CI artifact identifies the source revision and
clean/dirty state, exact command or procedure, environment, UTC start/end,
reviewer or CI identity, evidence location, result, and artifact SHA-256 when an
artifact was tested. Preserve superseded failures and retries. Never broaden the
recorded scope beyond the assertions that actually ran, and never confuse a
later evidence-storage commit with the artifact's source revision.
