# Development Guide

This guide explains how to work on Aster Desktop on Windows 11. It is an
operational companion to the normative [product specification](product-spec.md),
[architecture](architecture.md), [security requirements](security-requirements.md),
and repository governance in [`AGENTS.md`](../AGENTS.md).

This guide describes the current MVP v2 development line. Developers who are
evaluating the historical Z.AI-only MVP v1 baseline should first read the
[version selection guide](version-selection.md), check out its exact revision,
and then use the README and toolchain instructions from that revision. Do not
apply v2 commands or evidence claims retroactively to v1.

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

## Bootstrap a checkout on each laptop

Clone or update the repository through GitHub. Do not copy a working directory,
`node_modules`, Cargo output, local database, credential material, or generated
artifacts between laptops.

```powershell
git clone https://github.com/renenehru/aster-desktop.git
Set-Location aster-desktop
git switch main
git pull --ff-only
```

For an existing checkout, commit or stash only intentional source changes before
switching machines. Push the feature branch from the first laptop, fetch it from
the next laptop, and continue there:

```powershell
git fetch --prune origin
git switch --track origin/feature/<short-name>
```

Each laptop has its own Windows Credential Manager and local Aster database.
Provider keys and conversation history are not synchronized by Git.

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
access the current user's Aster database and provider-scoped credential-store
targets. Do not use
a personal or production credential in automated tests, screenshots, fixtures,
or retained evidence. Native credential tests require an isolated Windows
target and an unmistakably fake sentinel as defined by the acceptance criteria.

### Browser demo

Use the browser profile for fast visual and component work:

```powershell
pnpm dev
```

The browser profile is intentionally labelled **Demo mode**. It uses an
in-memory catalog and synthetic Usage data, has no provider credential method,
makes no provider, balance, or account-management request, and provides no
evidence for desktop IPC, credential storage, native dialogs, SQLite persistence,
provider networking, packaging, or signing.

## Repository map

| Path                               | Responsibility                                                                                         |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------ |
| `src/App.tsx`                      | Application shell and UI state orchestration                                                           |
| `src/components/`                  | Reusable UI controls and safe Markdown presentation                                                    |
| `src/services/assistantAdapter.ts` | Browser demo adapter and typed desktop IPC adapter                                                     |
| `src/types/`                       | Shared frontend domain types                                                                           |
| `src-tauri/src/lib.rs`             | Tauri command registration, command validation, stream coordination, and application assembly          |
| `src-tauri/src/api/`               | Exact per-provider request mapping, bounded SSE parsing, one-attempt transport, and cancellation logic |
| `src-tauri/src/database/`          | SQLite migrations, parameterized repository operations, import, and export validation                  |
| `src-tauri/src/credentials/`       | Provider-scoped Windows credential-store adapter                                                       |
| `src-tauri/credential-prompt/`     | Isolated native Windows credential prompt and its reviewed FFI boundary                                |
| `src-tauri/capabilities/main.json` | Least-privilege permissions for the main window                                                        |
| `src-tauri/IPC_CONTRACT.md`        | Exact renderer-to-Rust command and event contract                                                      |
| `scripts/`                         | Verification, policy, SBOM, build, audit, and engineering-packaging automation                         |
| `docs/`                            | Normative specifications, ADRs, evidence policy, and contributor guides                                |

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

The MVP v2 catalog is a closed Rust-owned registry. It contains exactly these
provider/model pairs:

| Provider           | Exact model identifiers                                                                         |
| ------------------ | ----------------------------------------------------------------------------------------------- |
| Z.AI               | `glm-4.7`, `glm-5`, `glm-5.1`, `glm-5.2`                                                        |
| DeepSeek           | `deepseek-v4-flash`, `deepseek-v4-pro`                                                          |
| Alibaba Cloud (US) | `qwen3.5-plus`, `qwen3.5-flash`, `qwen3.6-plus`, `qwen3.6-flash`, `qwen3.7-plus`, `qwen3.7-max` |
| Google Gemini      | `gemini-2.5-flash`, `gemini-2.5-flash-lite`, `gemini-2.5-pro`                                   |
| NVIDIA             | `nvidia/nemotron-3-super-120b-a12b`, `nvidia/nemotron-3-ultra-550b-a55b`                        |

Alibaba Cloud is fixed to the US-region API and must be identified as such
before content is sent. NVIDIA entries are hosted prototypes for evaluation.
The exact endpoints, authentication, request fields, stream formats, usage
mapping, and Fast/Standard/Deep behavior are dated in
[provider-contract.md](provider-contract.md). A model appears only when its full
contract is verified; do not add placeholders or unavailable entries.

Each provider key is captured by the Rust-owned native prompt and stored in a
separate Windows Credential Manager target. React carries only a provider ID and
safe configured/not-configured status. Never add a generic credential target or
a frontend, IPC, SQLite, log, fixture, or environment path for a key.

Fast, Standard, and Deep are Aster response profiles. The backend applies the
exact model-specific mapping and disables a profile when no mapping is verified.
An empty conversation may change provider/model; after its first message, the
pair is immutable and a different selection starts a new conversation.

Every explicit send makes at most one provider attempt. Automatic retries are
prohibited because an interrupted or hidden attempt may already have consumed
tokens. An origin, model, request field, stream parser, usage mapping, timeout,
or profile change requires contract verification plus specification, privacy,
architecture, threat, and test updates before implementation.

### Usage, balance, and account actions

The Usage ledger is local advisory state over the trailing seven days. Provider
metadata is normalized into non-cached input, cached input, output, and total
tokens. Missing fields remain visibly partial. The optional weekly token budget
is per provider, never blocks a request, and is not a credit or billing value.
The UI reports 10% remaining or less with red styling plus explicit text and an
icon.

DeepSeek balance refresh is a separate, explicit read-only call and the result
is not persisted. Other providers direct the user to their account website.
Account management takes only a typed provider/action pair; Rust selects the
fixed official HTTPS page and opens the default browser. Do not accept a URL,
model ID, path, or command from React for this operation. Aster never purchases
credits or changes a plan.

### Database migration

Schema v2 adds the immutable provider/model pair and normalized usage ledger.
The v1-to-v2 migration runs transactionally: existing conversations become
`zai` / `glm-5.1`, and an old total-token value remains a total with an incomplete
breakdown. Test successful migration, rollback on failure, restart behavior, and
legacy import before changing the schema. Never delete or recreate a user's
database to make a migration pass.

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
