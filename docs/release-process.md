# Release Process

This runbook describes the repository's current engineering-build workflow and
the evidence required before any stronger release claim. It does not claim that
a particular revision or artifact has passed. The normative release classes and
criteria are defined in [acceptance-criteria.md](acceptance-criteria.md), and
evidence format is defined in [evidence/README.md](evidence/README.md).

## Release classes

| Class                  | Permitted description                            | Boundary                                                                                                   |
| ---------------------- | ------------------------------------------------ | ---------------------------------------------------------------------------------------------------------- |
| Browser visual preview | `Demo mode UI preview`                           | Visual/component evidence only; no desktop or security claim                                               |
| Engineering MVP build  | `Windows engineering build for local evaluation` | Runtime product/security criteria must have evidence; production-only gaps remain explicit                 |
| Production release     | `Signed production release`                      | Every in-scope product, security, supply-chain, package, Windows E2E, SBOM, and signature criterion passes |

The current automation creates an unsigned engineering executable and NSIS
installer. It contains no authorized signing workflow or enabled updater. An
unsigned artifact must not be labelled production-ready, signed, generally
distributable, or update-secure.

Publishing source code to GitHub is not the same as publishing an application
release. Keep build artifacts out of ordinary commits; `outputs/`, `work/`,
`dist/`, and `src-tauri/target/` are intentionally ignored.

## 1. Establish scope and source identity

Before building:

1. Ensure every intended behavior and non-goal is represented by stable
   requirement IDs and current acceptance criteria.
2. Review changes to architecture, data flow, provider contract, permissions,
   dependencies, persistence, threats, and ADRs.
3. Record the exact Git commit and clean/dirty state. Do not release from an
   unidentified or unexpectedly dirty checkout.
4. Create a revision-specific evidence record named
   `YYYY-MM-DD-<revision>-<release-class>.md` under `docs/evidence/`.
5. List changed requirements, reviewed threats, test/evidence locations,
   exceptions, and residual risks.

If the application version changes, keep `package.json`,
`src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, lockfiles, and the
version-specific filenames in the engineering packaging scripts consistent.
The current scripts expect version `0.1.0` and its corresponding Tauri/NSIS
filenames.

## 2. Verify the source revision

Install locked dependencies and run the aggregate local gate from an appropriate
Windows environment:

```powershell
pnpm install --frozen-lockfile
powershell -ExecutionPolicy Bypass -File scripts/verify.ps1
```

Do not use `-SkipRust` for a release candidate. Record exact commands, tool and
Windows versions, UTC timestamps, reviewer identity, evidence paths, and all
results. A later successful retry does not erase an earlier failure.

Review the revision-specific CI run separately. The configured workflow runs on
Windows and retains manifest-derived frontend and Rust SBOMs, but the workflow
file itself is not execution evidence.

## 3. Build an engineering artifact

From the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/build-engineering.ps1
```

The script locates Visual Studio x64 C++ tools, initializes their developer
environment, remaps local Rust and AWS-LC C/C++ source prefixes, runs the
production frontend build through Tauri, and creates the release executable and
NSIS bundle expected by the packaging workflow.

The output paths are:

- `src-tauri/target/release/aster-desktop.exe`
- `src-tauri/target/release/bundle/nsis/Aster_0.1.0_x64-setup.exe`

A successful build proves only that the build command completed. It does not
prove native behavior, package cleanliness, installation, signing, or release
acceptance.

## 4. Audit the package inputs

Run the deterministic byte-pattern and frontend-bundle audit:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/package-audit.ps1
```

Record its exact artifact hashes and scope. This audit does not install or
unpack the installer, run the application, verify a clean-profile lifecycle, or
prove Authenticode status.

## 5. Execute release-class acceptance

Use [acceptance-criteria.md](acceptance-criteria.md) as the checklist rather
than substituting an aggregate test count. For a desktop release candidate this
includes, as applicable:

- a clean Windows 11 profile and identified WebView2/runtime environment;
- native prompt and isolated Windows Credential Manager sentinel tests;
- real Tauri IPC authorization and stream-event association;
- temporary-profile SQLite lifecycle, migration, restart, and fault recovery;
- controlled HTTPS/SSE transport, TLS, redirects, timeouts, retry, malformed
  streams, and cancellation races without a billable production endpoint;
- native import/export dialogs, hostile schema/path cases, and transaction
  atomicity;
- keyboard, screen-reader, focus, clipboard, and 100%, 150%, and 200% Windows
  scaling procedures;
- installed-package inventory, uninstall behavior, CSP/capability inspection,
  source-map/credential checks, and performance limits;
- installer and executable signature/tamper verification for production.

Any missing environment, tool, identity, procedure, or evidence is `NOT RUN`,
not `PASS`. Required `FAIL` and `NOT RUN` results block the corresponding release
classification.

## 6. Complete the evidence record

Follow [evidence/README.md](evidence/README.md). Each result identifies:

- source revision and clean/dirty state;
- artifact filename and SHA-256 when an artifact was exercised;
- OS, architecture, runtime/tool versions, and display/accessibility settings;
- exact command or deterministic procedure;
- UTC start and end;
- protected evidence path and reviewer/CI identity;
- precise assertions, exclusions, failures, retries, exceptions, and residual
  risks.

Never retain credentials, authorization headers, prompts, responses, imports,
database rows, provider payloads, or personal paths. Classify the result only
after reviewing every mapped requirement and criterion.

## 7. Assemble an engineering handoff

The packaging script requires all of the following inputs:

- the release executable and NSIS installer from the engineering build;
- `work/sbom-frontend.cdx.json` and `work/sbom-rust.cdx.json` from verification;
- the canonical `LICENSE` and attribution `NOTICE` files;
- the selected evidence record;
- `outputs/Aster-MVP-v1-preview.png` as the current preview input;
- a production `dist/` bundle for the package audit.

Run it with the evidence record for the current revision rather than relying on
the historical default:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/package-engineering.ps1 `
  -EvidenceRecord "docs\evidence\YYYY-MM-DD-<revision>-engineering-build.md"
```

The script requires a clean Git working tree and a tracked evidence record. It
reruns the package audit, copies the executable, installer, both SBOMs,
`LICENSE`, and `NOTICE`,
creates the source snapshot from the exact `HEAD` tracked-file inventory with
`git archive`, appends the source revision and artifact identity to
`outputs/verification-report.md`, and writes `outputs/SHA256SUMS.txt`. Ignored
and untracked workspace files are excluded by construction; do not replace this
with directory-wide archiving and a denylist.

The current engineering packaging script does not generate or verify a
consolidated bundle of all third-party copyright notices and license texts.
The `SEC-045` / `AC-039` redistribution-compliance gate is therefore `NOT RUN` and blocks public
distribution of binary or installer packages until the applicable material is
collected, reviewed, packaged, and recorded as revision-specific evidence. It
does not block publication of the source repository under Apache-2.0.

Review the generated report and hashes before sharing. Verify an artifact by
recomputing its hash with PowerShell and comparing it with the checksum file:

```powershell
Get-FileHash -Algorithm SHA256 outputs\Aster-0.1.0-x64-engineering.exe
Get-AuthenticodeSignature outputs\Aster-0.1.0-x64-engineering.exe
Get-AuthenticodeSignature outputs\Aster-0.1.0-x64-engineering-setup.exe
```

An observed `NotSigned` status is expected for the current engineering workflow.
The inspection itself may pass while the production signing gate remains
`NOT RUN`.

## 8. Production promotion boundary

Do not promote an engineering bundle to production. Production additionally
requires an authorized Windows signing identity, verifiable installer and binary
signatures, post-signing artifact hashes, signed provenance, the complete
Windows E2E and package evidence, and all other in-scope acceptance criteria to
pass. The repository currently has no signing automation, and automatic updates
remain disabled until a signed update chain is specified and verified.

If any release requirement cannot be met, retain the result as `FAIL` or
`NOT RUN`, document the residual risk, and stop at the permitted lower release
class. Exceptions must satisfy every field and prohibition in `AGENTS.md`.

## Engineering handoff contents

When sharing an engineering build for explicitly local evaluation, keep these
items together:

- clearly labelled unsigned installer and portable executable;
- source revision and source snapshot;
- revision-specific verification report with all `NOT RUN` items;
- SHA-256 checksum file obtained through a trusted channel;
- frontend and Rust SBOMs with their stated manifest-derived scope;
- the Apache-2.0 `LICENSE`, attribution `NOTICE`, and all applicable third-party license material;
- residual risks, valid exceptions, and safe installation/removal instructions.
