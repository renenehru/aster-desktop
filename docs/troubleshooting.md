# Troubleshooting

This guide favors safe, reversible diagnosis. Do not work around a failure by
disabling TLS validation, CSP, input validation, secret scanning, permission
checks, locked dependencies, or signature policy. Preserve user data and record
only non-sensitive evidence.

## First diagnostic pass

Run these commands from the repository root:

```powershell
node --version
pnpm --version
rustc --version
cargo --version
pnpm install --frozen-lockfile
pnpm check
```

When reporting a problem, include the source revision and clean/dirty state,
Windows version and architecture, relevant tool versions, exact command, UTC
time, exit code, and the stable Aster error code if one is displayed. Do not
include an API key, authorization header, prompt, response, conversation title,
database row, provider body, import/export content, or personal filesystem path.

## `pnpm` or Node.js is not found

Install a supported Node.js version and pnpm, open a new PowerShell session, and
confirm both are on `PATH`. `package.json` requires Node.js 22.12.0 or newer and
pnpm 10.0.0 or newer; it pins pnpm 11.7.0 as the repository package manager.

If `pnpm install --frozen-lockfile` reports a manifest/lockfile mismatch, do not
discard or regenerate the lockfile as a shortcut. Confirm that the checkout is
complete. For an intentional dependency change, update and review the manifest
and lockfile together, then run the vulnerability, license, secret, and SBOM
gates.

## Rust uses the wrong version or lacks components

The checkout pins Rust 1.97.0, `rustfmt`, Clippy, and the
`x86_64-pc-windows-msvc` target in `rust-toolchain.toml`. Confirm that `rustup`
honors the file from the repository root:

```powershell
rustup show active-toolchain
rustup component list --installed
rustup target list --installed
```

Do not bypass a toolchain or `--locked` failure. Restore the pinned toolchain or
review an intentional toolchain/dependency change through the specification and
supply-chain workflow.

## `cargo audit` is not found

`scripts/verify.ps1` invokes `cargo audit` for the Rust lockfile. Install the
RustSec `cargo-audit` command in the local tool environment and confirm
`cargo audit --version` succeeds before retrying. If it is unavailable, record
the Rust dependency audit as `NOT RUN`; do not report the aggregate verification
as passed.

## Visual C++ tools or `vswhere.exe` are missing

`scripts/build-engineering.ps1` requires Visual Studio Installer and the
Microsoft Visual C++ x64 toolset. Install Microsoft C++ Build Tools with the
**Desktop development with C++** workload, then retry from a fresh PowerShell
session. The script deliberately locates and initializes the x64 developer
environment; avoid replacing it with an unrecorded toolchain.

Typical symptoms include:

- `Visual Studio Installer could not be located`;
- `A Visual Studio installation with the Visual C++ x64 tools was not found`;
- linker errors referring to `cl.exe`, `link.exe`, Windows SDK libraries, or an
  unavailable MSVC target.

## Port 1420 is already in use

The development server uses loopback port 1420 with `strictPort: true`. Stop the
process already bound to that port, then rerun `pnpm desktop:dev` or `pnpm dev`.
Do not change the host to a public interface: the development CSP and security
assertions intentionally restrict the endpoint to `127.0.0.1`.

## The desktop window is blank or WebView2 cannot start

Confirm Microsoft Edge WebView2 Runtime is installed and up to date, then rerun
the desktop command. Use `pnpm dev` only to separate a frontend rendering problem
from a native startup problem; a successful browser demo does not prove Tauri or
WebView2 integration.

Also run `pnpm build` and `pnpm security:config`. Do not weaken the CSP or enable
remote scripts to diagnose a blank window.

## The application says `Demo mode`

This is expected after `pnpm dev`. The browser adapter is in-memory and cannot
configure a provider key, call Z.AI, DeepSeek, Alibaba Cloud, Google, or NVIDIA,
refresh a balance, open an account-management page, use SQLite, or exercise
native dialogs. Its catalog and Usage data are deterministic UI fixtures. Start
`pnpm desktop:dev` when native behavior is required.

## There is no API-key field in the interface

This is intentional. The webview must never render or transport an API key.
Start the desktop app, open **Settings**, choose the intended provider, and use
its **Add API key** action to invoke the Rust-owned native Windows prompt.
Cancelling the prompt preserves that provider's previous credential status.
Every provider has a separate Windows Credential Manager target; adding or
removing one key must not change another provider's status.

Never add a temporary HTML password input, IPC key argument, environment-file
key, browser-storage key, or logging statement as a diagnostic workaround.

## A message cannot be sent

Check the visible state without inspecting sensitive payloads:

1. Confirm that the conversation is bound to the intended provider and exact
   catalog model.
2. Confirm the credential status for that provider. A key configured for another
   provider does not satisfy this check.
3. Review and acknowledge the selected provider's current external-processing
   notice. For Alibaba Cloud, confirm the fixed US-region disclosure.
4. Check normal network reachability to the documented provider without changing
   the fixed origin or bypassing certificate validation.
5. Use the stable UI error code and retry guidance. Authentication failures are
   not automatically retried; replace or remove the credential through Settings.

Aster performs no automatic provider retry. Selecting retry, resend, or
regenerate is a new explicit operation and may consume additional tokens.

Startup reachability for each provider is intentionally `unknown` until a real
attempt completes. Do not interpret that state as evidence that a provider or
model is online, offline, available, or compatible.

## The provider or model cannot be changed in the current chat

This is intentional after the first message is persisted. Provider and model are
fixed for that conversation so edit, resend, regenerate, events, credentials,
and Usage cannot cross boundaries. Choose **Start a new chat with this model**
and confirm the action. Aster does not copy the old conversation context into the
new provider request.

An empty conversation should allow a pair from the closed catalog. If a verified
pair is missing or an unexpected entry appears, capture only the safe IDs and
source revision, run the frontend catalog snapshot
`pnpm test:run -- src/services/providerCatalog.test.ts`, the Rust catalog tests
`cargo test --manifest-path src-tauri/Cargo.toml --workspace catalog`, and
`pnpm security:config` for the surrounding capability/origin boundary. Report
catalog drift; do not add a free-form model, endpoint, or unavailable
placeholder as a workaround.

## Usage appears partial or differs from a provider bill

Aster Usage is a local advisory total for the trailing seven days, not provider
billing or credit. It uses only validated usage metadata returned for Aster
operations. A cancelled or failed attempt, missing provider fields, historical
v1 total, or operation outside Aster may make the breakdown partial or different
from the provider website.

The configured weekly token budget is also local. It does not stop a request or
represent purchased credit. When 10% or less remains, the UI should show a red
warning plus explicit text and an icon. If the threshold appears wrong, report
the provider ID, model ID, safe aggregate counts, configured budget, UTC time,
and source revision; do not include conversation text or raw provider data.

## Balance refresh or account management fails

Only DeepSeek has an MVP v2 balance refresh, and it runs only after an explicit
user action. The value is held for the current session and is not stored. Other
providers direct the user to their official website; they do not have an Aster
balance value.

Usage, billing, add-credit, spend, and deployment actions open fixed official
pages in the operating system's default browser. Aster never buys credit or
changes a plan. If nothing opens, verify the Windows default-browser association
and the stable error code. Do not paste an account URL into the renderer, broaden
Tauri shell permissions, or embed an account page to bypass the fixed action map.

## A conversation, database, or migration error occurs

Do not delete, overwrite, or silently recreate the database. Aster stores
conversations in user-scoped plaintext SQLite and is designed to fail safely on
corruption or migration failure.

- Preserve the original data file and capture only the safe error code.
- A v1-to-v2 upgrade must retain existing conversations as `zai` / `glm-5.1`
  and preserve each historical total only on its message, with the breakdown
  left unknown. It must not seed the v2 usage ledger. Do not edit provider/model
  columns manually.
- If the UI remains usable, make an explicit export and treat the JSON as
  sensitive plaintext.
- Reproduce with a temporary test database or isolated Windows profile rather
  than a real user's content.
- Add a deterministic migration or repository regression test before changing
  recovery behavior.

Do not attach the database, exports, journal files, conversation screenshots, or
paths containing a user identity to a public issue.

## Import is rejected

Import uses a strict, versioned schema and validates the complete document before
starting a transaction. Confirm the file matches the version 2 schema in
`src-tauri/IPC_CONTRACT.md`, contains a registered provider/model pair, contains
only allowed fields/enums, observes count/size/depth limits, and has valid RFC
3339 timestamps. A legacy version 1 Aster export is accepted only through its
documented migration path as `zai` / `glm-5.1`; its old token value remains a
total with an incomplete breakdown.

Do not loosen validation to accept source IDs, filesystem paths, HTML execution
state, credentials, headers, tool messages, unknown fields, or unsupported
versions. A rejected import must leave the database unchanged.

## Export is cancelled or fails

Cancellation from the native save dialog is a normal no-op. For other failures,
choose a writable user-selected location and retain only the safe error code.
The frontend must never construct a path or receive the native path over IPC.
Exports contain plaintext conversation content and must be handled as sensitive
files.

## Formatting, lint, or type checking fails

Run the specific command first:

```powershell
pnpm lint
pnpm format:check
pnpm typecheck
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
```

For an intentional frontend/Markdown formatting update, use `pnpm format` and
review every changed file. For Rust, run `cargo fmt --manifest-path
src-tauri/Cargo.toml --all`, review the diff, and rerun the check. Do not suppress
Clippy, ESLint, TypeScript, or formatting failures with unexplained exceptions.

## Secret scanning reports a finding

Treat the finding as a potential incident until reviewed. Do not add the value to
an allowlist or weaken the pattern merely to pass the gate.

1. Stop copying or publishing the value.
2. Preserve safe evidence without reproducing the secret.
3. Remove the value from source, fixtures, generated files, and artifacts.
4. Rotate a real exposed credential outside the repository.
5. Add a regression test and update the threat model when a vulnerability was
   present.

Tests use unmistakably fake sentinels; they must never resemble a real provider
credential or appear in packaged output.

## Security configuration assertions fail

`pnpm security:config` intentionally detects drift in CSP, Tauri capabilities,
renderer networking/storage, raw IPC, credential capture, unsafe-code
confinement, production/dev separation, and diagnostic dependencies. Identify
the changed trust boundary before editing the assertion. If the change is
intended, update its requirement, acceptance criterion, architecture, threat
model, ADR when required, implementation, and focused test together. Never make
the check less strict simply to accommodate an implementation defect.

## Package audit reports a builder path or development endpoint

Build candidate artifacts with the repository wrapper:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/build-engineering.ps1
powershell -ExecutionPolicy Bypass -File scripts/package-audit.ps1
```

The wrapper applies Rust and AWS-LC native path remapping. If old native objects
remain after a build-configuration change, remove only generated Cargo output
with the standard Cargo clean operation, rebuild through the wrapper, and retain
the failed audit plus retry in the evidence history. Do not narrow a valid
package signature to hide a real endpoint, credential, source map, or personal
path.

## Engineering packaging reports a missing input

`scripts/package-engineering.ps1` requires:

- Git, an identified `HEAD`, and a completely clean working tree;
- `work/build-identity.json` generated from the same clean revision;
- an ignored evidence draft named
  `work/evidence/YYYY-MM-DD-<full-revision>-engineering-build.md` that declares the
  full build revision;
- the release executable and NSIS installer;
- both SBOMs under `work/`;
- the selected evidence record;
- tracked `assets/Aster-MVP-v2-preview.png` as the non-sensitive preview input;
- the production `dist/` bundle.

Commit the source revision first, run full verification to generate SBOMs, run
the engineering build to generate its identity, create the ignored
revision-specific evidence draft, and provide the non-sensitive preview before
retrying. Do not commit the draft before packaging; that would change `HEAD`.
Move any unexpected file or subdirectory out of `outputs/`; the packager rejects
it instead of silently including it. Pass the `work/evidence/` path with
`-EvidenceRecord` and a bounded self-declared reviewer or CI job label with
`-VerifierIdentity`. That label is self-declared and is not an authenticated
identity. The script recomputes the recorded hashes and uses
`git archive` only for the matched clean revision, so stale artifacts and
ignored/untracked workspace data cannot enter the supported handoff silently.

For an MVP v2 handoff, confirm every manifest, audit default, package path, and
output name uses `0.2.0`. A mismatch is a packaging failure, not a reason to
rename an artifact after the build.

## Authenticode reports `NotSigned`

That status is expected for the current engineering workflow. Label the files as
unsigned engineering artifacts for local evaluation and record the production
signing gate as `NOT RUN`. Do not suppress Windows warnings, fabricate a
signature result, or publish the artifact as production. Production requires an
authorized signing identity and retained positive and negative tamper evidence.

## There are no application log files

This is intentional under `ADR-0009`. Aster MVP v2 creates no application
diagnostic log, crash-report upload, telemetry, or analytics. Diagnose with
deterministic tests, safe machine error codes, controlled fixtures, and
non-sensitive environment metadata. Adding diagnostics is a separate privacy,
retention, architecture, threat-model, ADR, and acceptance change.

## Reporting a defect or vulnerability

Use the repository's contribution process for ordinary defects and follow
[`SECURITY.md`](../SECURITY.md) for suspected vulnerabilities. A useful report
contains minimal deterministic steps, expected and observed safe behavior,
source/artifact identity, environment, and evidence scope. Redact before
retention, and never place exploit details or real user data in public logs,
screenshots, or issues.
