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
configure an API key, call Z.AI, use SQLite, or exercise native dialogs. Start
`pnpm desktop:dev` when native behavior is required.

## There is no API-key field in the interface

This is intentional. The webview must never render or transport an API key.
Start the desktop app, open **Settings**, and use **Add API key** to invoke the
Rust-owned native Windows prompt. Cancelling the prompt preserves the previous
credential status.

Never add a temporary HTML password input, IPC key argument, environment-file
key, browser-storage key, or logging statement as a diagnostic workaround.

## A message cannot be sent

Check the visible state without inspecting sensitive payloads:

1. Confirm the desktop credential status is configured.
2. Review and acknowledge the external-processing notice for the current app
   session; the acknowledgement intentionally does not persist across launches.
3. Check normal network reachability to the documented provider without changing
   the fixed origin or bypassing certificate validation.
4. Use the stable UI error code and retry guidance. Authentication failures are
   not automatically retried; replace or remove the credential through Settings.

Startup provider reachability is intentionally `unknown` until a real attempt
completes. Do not interpret that state as evidence that the provider is online or
offline.

## A conversation, database, or migration error occurs

Do not delete, overwrite, or silently recreate the database. Aster stores
conversations in user-scoped plaintext SQLite and is designed to fail safely on
corruption or migration failure.

- Preserve the original data file and capture only the safe error code.
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
starting a transaction. Confirm the file matches the version 1 schema in
`src-tauri/IPC_CONTRACT.md`, uses `model: "glm-5.1"`, contains only allowed
fields/enums, observes count/size/depth limits, and has valid RFC 3339 timestamps.

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
- a repository-relative evidence record tracked by that revision;
- the release executable and NSIS installer;
- both SBOMs under `work/`;
- the selected evidence record;
- `outputs/Aster-MVP-v1-preview.png`;
- the production `dist/` bundle.

Run full verification to generate SBOMs, run the engineering build, create and
commit the revision-specific evidence record, and provide the non-sensitive
preview before retrying. Commit or remove every source change, then pass the
current repository-relative evidence path with `-EvidenceRecord`. The script
uses `git archive` so ignored and untracked workspace data cannot enter the
source handoff.

## Authenticode reports `NotSigned`

That status is expected for the current engineering workflow. Label the files as
unsigned engineering artifacts for local evaluation and record the production
signing gate as `NOT RUN`. Do not suppress Windows warnings, fabricate a
signature result, or publish the artifact as production. Production requires an
authorized signing identity and retained positive and negative tamper evidence.

## There are no application log files

This is intentional under `ADR-0009`. Aster MVP v1 creates no application
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
