# Development Guide

This guide explains how to work on Aster Desktop on Windows 11. It is an
operational companion to the normative [product specification](product-spec.md),
[architecture](architecture.md), [security requirements](security-requirements.md),
and repository governance in [`AGENTS.md`](../AGENTS.md).

## Prerequisites

Aster targets the Windows desktop and the MSVC Rust toolchain.

| Tool               | Repository requirement                                                                       |
| ------------------ | -------------------------------------------------------------------------------------------- |
| Windows            | Windows 11 with Microsoft Edge WebView2 Runtime                                              |
| Node.js            | 22.12.0 or newer; CI currently uses 24.14.0                                                  |
| pnpm               | 10.0.0 or newer; `package.json` pins 11.7.0 as the package manager                           |
| Rust               | 1.97.0 with `rustfmt`, Clippy, and `x86_64-pc-windows-msvc`, pinned by `rust-toolchain.toml` |
| Native build tools | Microsoft C++ Build Tools with the **Desktop development with C++** workload and x64 tools   |
| PowerShell         | Windows PowerShell or PowerShell capable of running the repository `.ps1` scripts            |

The full verification script also invokes `cargo audit`. Ensure that executable
is installed and available on `PATH` before running the Rust audit locally.

Confirm the active environment from PowerShell:

```powershell
node --version
pnpm --version
rustc --version
cargo --version
```

## Bootstrap a checkout

From the repository root:

```powershell
pnpm install --frozen-lockfile
pnpm check
```

`--frozen-lockfile` is deliberate. Do not regenerate a lockfile merely to make
an installation succeed. If dependencies intentionally change, review both
`package.json` and `pnpm-lock.yaml`, keep versions exact-pinned, document the
runtime dependency decision, and run the applicable supply-chain gates.

The Rust toolchain is selected automatically from `rust-toolchain.toml` when
`rustup` manages the installation. Rust dependencies are locked in
`src-tauri/Cargo.lock`.

## Choose the correct runtime

### Desktop development

Use the Tauri desktop process to exercise native commands, SQLite, Windows
Credential Manager, native dialogs, and the Rust-owned provider boundary:

```powershell
pnpm desktop:dev
```

Alternatively, the repository wrapper installs locked frontend dependencies
before starting the same desktop command:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/run-dev.ps1
```

Desktop development overlays `src-tauri/tauri.dev.conf.json` on the production
configuration. The overlay permits only the loopback Vite endpoint on port 1420. The production `src-tauri/tauri.conf.json` deliberately contains no
development URL or development CSP.

Interactive desktop development uses the normal application adapters and may
access the current user's Aster database and credential-store entry. Do not use
a personal or production credential in automated tests, screenshots, fixtures,
or retained evidence. Native credential tests require an isolated Windows
target and an unmistakably fake sentinel as defined by the acceptance criteria.

### Browser demo

Use the browser profile for fast visual and component work:

```powershell
pnpm dev
```

The browser profile is intentionally labelled **Demo mode**. It uses an
in-memory adapter, has no provider credential method, makes no provider request,
and provides no evidence for desktop IPC, credential storage, native dialogs,
SQLite persistence, provider networking, packaging, or signing.

## Repository map

| Path                               | Responsibility                                                                                |
| ---------------------------------- | --------------------------------------------------------------------------------------------- |
| `src/App.tsx`                      | Application shell and UI state orchestration                                                  |
| `src/components/`                  | Reusable UI controls and safe Markdown presentation                                           |
| `src/services/assistantAdapter.ts` | Browser demo adapter and typed desktop IPC adapter                                            |
| `src/types/`                       | Shared frontend domain types                                                                  |
| `src-tauri/src/lib.rs`             | Tauri command registration, command validation, stream coordination, and application assembly |
| `src-tauri/src/api/`               | Fixed-origin Z.AI request mapping, bounded SSE parsing, retry, and cancellation logic         |
| `src-tauri/src/database/`          | SQLite migrations, parameterized repository operations, import, and export validation         |
| `src-tauri/src/credentials/`       | Windows credential-store adapter                                                              |
| `src-tauri/credential-prompt/`     | Isolated native Windows credential prompt and its reviewed FFI boundary                       |
| `src-tauri/capabilities/main.json` | Least-privilege permissions for the main window                                               |
| `src-tauri/IPC_CONTRACT.md`        | Exact renderer-to-Rust command and event contract                                             |
| `scripts/`                         | Verification, policy, SBOM, build, audit, and engineering-packaging automation                |
| `docs/`                            | Normative specifications, ADRs, evidence policy, and contributor guides                       |

The React webview is a presentation layer. It must not own secrets, provider
networking, filesystem paths, SQL, or security decisions. The Rust process owns
those boundaries and revalidates every untrusted input.

## Spec-driven change workflow

Before changing behavior, permissions, persistence, external interfaces, data
flow, or a security-sensitive dependency:

1. Read [`AGENTS.md`](../AGENTS.md) and identify the affected `PR-`, `FR-`,
   `UX-`, `NFR-`, and `SEC-` IDs.
2. Update the owning normative requirement and its `AC-` criterion before
   implementation.
3. Update the architecture and threat model when an asset, actor, permission,
   parser, persistence path, trust boundary, or external service changes.
4. Add or revise an ADR for a difficult-to-reverse decision, new dependency
   class, privilege, or security-posture change.
5. Update the traceability matrix in `acceptance-criteria.md`.
6. Add a failing automated test when practical; otherwise define a deterministic
   manual procedure and explain why automation is not practical.
7. Implement only the specified capability. Do not add dormant commands or
   permissions for future work.
8. Run the applicable gates, record scoped evidence, and review specification,
   configuration, implementation, and test drift.

For a non-behavioral documentation-only change, state that no normative
requirement applies and still run the formatting, secret-scan, and link checks
appropriate to the files changed.

## Working on security boundaries

### Frontend and Markdown

Model output and imported content remain hostile. Keep raw HTML disabled,
construct React elements without an HTML sink, validate external links, and do
not add remote scripts, fonts, styles, analytics, or provider networking. Add a
security regression test for any vulnerability fix.

### Desktop IPC

The frontend sends JSON as a UTF-8 `Uint8Array` through the single raw-body IPC
helper. The backend enforces a 320 KiB byte ceiling before JSON parsing and then
performs shape and semantic validation. A command change requires coordinated
updates to the requirement, tests, Rust handler registration, capability,
generated permission metadata, `src-tauri/IPC_CONTRACT.md`, and security
configuration assertions. Never carry an API key, arbitrary path, or arbitrary
URL through a new command.

### Provider behavior

The verified MVP contract is `glm-5.1` at the fixed Z.AI HTTPS origin documented
in [provider-contract.md](provider-contract.md). Fast disables provider thinking;
Standard and Deep both enable it and differ in application output-token caps and
guidance. A model, origin, request-field, streaming, or timeout change requires
contract verification plus specification, privacy, architecture, threat, and
test updates before implementation.

### Dependencies

Keep JavaScript and Rust versions exact-pinned and both lockfiles current. A new
runtime dependency needs a documented purpose and review of license,
maintenance, transitive footprint, and security history. Do not add runtime code
downloads or untrusted lifecycle scripts.

## Everyday commands

```powershell
# Check frontend behavior, types, formatting, configuration, secrets, and build
pnpm check

# Apply frontend and Markdown formatting; review every resulting change
pnpm format

# Check Rust formatting without modifying files
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check

# Run the aggregate local verification workflow
powershell -ExecutionPolicy Bypass -File scripts/verify.ps1
```

See [testing.md](testing.md) for targeted tests and evidence boundaries,
[release-process.md](release-process.md) for packaging, and
[troubleshooting.md](troubleshooting.md) for safe recovery steps.
