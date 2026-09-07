# Release Process

This runbook describes Aster's engineering-build workflow and the evidence
required before any stronger release claim. It does not claim that a particular
revision, provider, model, or artifact passed. Normative release classes and
criteria are in [acceptance-criteria.md](acceptance-criteria.md); evidence format
is in [evidence/README.md](evidence/README.md).

## Release classes

| Class                  | Permitted description                            | Boundary                                                                                                    |
| ---------------------- | ------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| Browser visual preview | `Demo mode UI preview`                           | Visual/component evidence only; no desktop, provider, credential, persistence, or security claim            |
| Engineering MVP build  | `Windows engineering build for local evaluation` | Runtime product/security criteria need evidence; production-only gaps and every `NOT RUN` remain explicit   |
| Production release     | `Signed production release`                      | Every in-scope product, security, provider, package, Windows E2E, SBOM, provenance, and signing gate passes |

Current repository automation creates unsigned engineering artifacts. It has no
authorized signing workflow or enabled updater. An unsigned artifact must not be
labelled production-ready, signed, generally distributable, or update-secure.

Publishing source code to GitHub is not the same as publishing an application
release. Keep ordinary commits free of `outputs/`, `work/`, `dist/`,
`src-tauri/target/`, user databases, credentials, exports, and signing material.

## 1. Establish scope and source identity

Before building:

1. Ensure every intended behavior and non-goal is represented by stable
   requirement IDs and current acceptance criteria.
2. Review architecture, provider contracts, permissions, dependencies,
   persistence/migration, threats, and ADR changes.
3. Confirm the closed catalog still contains exactly the 5 providers and 17
   provider/model pairs in [provider-contract.md](provider-contract.md).
4. Record the exact Git commit and clean/dirty state. Do not release from an
   unidentified or unexpectedly dirty checkout.
5. Commit the exact source revision before verification. Do not add a tracked
   evidence file to that commit and then claim it already identified its own
   final hash.
6. Prepare an ignored local evidence draft under `work/evidence/` after the
   commit is known. List changed requirements, reviewed threats, evidence
   locations, exceptions, and residual risks.

For an MVP v2 artifact, all version-bearing files and output names must agree on
`0.2.0`, including `package.json`, Rust package metadata and lockfile entries,
Tauri configuration, package-audit defaults, and engineering-package filenames.
Any mismatched build input or package name discovered during this preflight is a
version-coherence failure; correct it in a reviewed source change before
building. Do not rename a generated file to simulate a coherent version.

## 2. Verify the source revision

Install locked dependencies and run the aggregate local gate in the supported
Windows environment:

```powershell
pnpm install --frozen-lockfile
powershell -ExecutionPolicy Bypass -File scripts/verify.ps1
```

Do not use `-SkipRust` for a release candidate. Record exact commands, tool and
Windows versions, UTC timestamps, reviewer identity, evidence paths, and every
result. A later successful retry does not erase an earlier failure.

Review the revision-specific CI run separately. Workflow configuration is not
evidence that the run passed. Default verification must use controlled fixtures
and must not contact a live or billable provider endpoint.

## 3. Build an engineering artifact

From the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/build-engineering.ps1
```

The wrapper requires the same clean Git revision before and after the build,
initializes the Visual Studio x64 developer environment, applies the documented
local path remapping, builds the production frontend through Tauri, and creates
the release executable and NSIS bundle. It then writes
`work/build-identity.json` with that source revision and hashes for the
executable, installer, both previously generated SBOMs, and deterministic
`dist/` tree.

For a coherent `0.2.0` build, the expected primary outputs are:

- `src-tauri/target/release/aster-desktop.exe`
- `src-tauri/target/release/bundle/nsis/Aster_0.2.0_x64-setup.exe`

A successful build and matching identity prove only that those local files were
observed after the command ran from the recorded clean revision. The unsigned
manifest is not signed provenance and does not prove native behavior, database
migration, package cleanliness, installation, provider availability,
credentials, signing, or release acceptance.

## 4. Audit the package inputs

Run the deterministic byte-pattern and frontend-bundle audit:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/package-audit.ps1
```

Record exact artifact hashes and the script's stated scope. Confirm packaged
frontend assets contain no API key, authorization header, provider payload,
source map, development endpoint, caller-defined origin, or personal builder
path. This audit does not install or unpack the NSIS installer, execute Aster,
inspect Credential Manager, derive a complete binary inventory, verify live
provider behavior, or prove Authenticode status.

## 5. Execute release-class acceptance

Use [acceptance-criteria.md](acceptance-criteria.md) rather than substituting an
aggregate test count. For an MVP v2 desktop candidate, include as applicable:

- clean Windows 11 profile, identified WebView2 runtime, install, launch,
  restart, uninstall, and local-data retention;
- separate isolated Windows Credential Manager targets for Z.AI, DeepSeek,
  Alibaba Cloud, Google, and NVIDIA, using unmistakably fake values;
- proof that React and IPC never contain a key and that one provider key cannot
  authorize another provider request;
- catalog inclusion/exclusion and exact request/profile contract fixtures for
  all 17 registered provider/model pairs;
- conversation selection while empty and enforced new-chat behavior after the
  first message locks the provider/model pair;
- provider-specific disclosure, including Alibaba Cloud's fixed US-region
  boundary and NVIDIA's hosted-prototype label;
- one-attempt provider transport, bounded SSE parsing, TLS/redirect/timeout
  policy, malformed streams, partial UTF-8, explicit cancellation, duplicate
  terminal events, and absence of automatic retry;
- temporary-profile SQLite v1-to-v2 migration, restart, rollback, import/export,
  usage-ledger idempotency, and corruption-safe failure;
- local trailing-seven-day Usage calculations, partial coverage, per-provider
  advisory budget, and accessible red-plus-text/icon warning at exactly and below
  10% remaining;
- explicit read-only DeepSeek balance refresh, session-only handling, and proof
  that no other provider exposes a balance call;
- typed provider/account actions opening only fixed official HTTPS pages in the
  default browser, with no credit purchase, plan change, embedded page, or
  renderer-supplied URL;
- keyboard, screen-reader, focus, clipboard, reduced-motion, contrast, and 100%,
  150%, and 200% Windows scaling procedures;
- installed-package inventory, CSP/capability inspection, source-map/credential
  checks, and performance limits;
- executable and installer signature/tamper verification for production.

Live-provider compatibility requires a separately authorized, secret-scoped
procedure for the exact revision, account, region, and model. If that procedure
is absent, its result is `NOT RUN`; contract fixtures, documentation, browser
demo, or source inspection cannot promote it to `PASS`.

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
- precise assertions, exclusions, failures, retries of the verification
  procedure, exceptions, and residual risks.

Never retain credentials, authorization headers, prompts, responses, balances,
imports, database rows, provider payloads, or personal paths. Classify the
result only after reviewing every mapped requirement and criterion.

For local engineering packaging, save the draft as
`work/evidence/YYYY-MM-DD-<full-source-revision>-engineering-build.md`. The
revision segment is the complete lowercase 40-to-64-character revision, and the
record contains exactly one canonical `**Source revision:** <revision>` line.
Create the non-empty strict-UTF-8 sibling
`work/evidence/YYYY-MM-DD-<full-source-revision>-engineering-build.log` as the
retained command/procedure log. The Markdown record is at most 256 KiB and the
log is at most 4 MiB. Both must contain no shared secret match,
Markdown-prefixed credential header, or normalized personal path. Each row
repeats the canonical source, environment, UTC window, and identity and
identifies its exact command/procedure, artifact/hash status, and scope. Every
`PASS` row names the exact repository-relative sibling-log path. Each gate has
one current result; prior failures and retries remain in the dedicated history
section. Every pipe-delimited result row, including an indented row, must match
the canonical schema. `work/` is ignored so the evidence inputs do not alter the
revision they describe.
After review, a redacted durable copy may be committed later under
`docs/evidence/` or retained as a protected CI artifact; that storage commit is
not the artifact source revision.

## 7. Assemble an engineering handoff

The packaging run requires:

- the version-coherent release executable and NSIS installer;
- `work/sbom-frontend.cdx.json` and `work/sbom-rust.cdx.json` from verification;
- `work/build-identity.json` from the clean-source engineering build;
- the canonical `LICENSE` and attribution `NOTICE` files;
- an ignored revision-specific evidence draft under `work/evidence/` that
  declares the exact build source revision and contains exactly
  `**Overall classification:** Unsigned engineering build for local evaluation`;
- its required revision-specific sibling log, referenced by every `PASS` row
  and copied into the handoff as `verification-evidence.log`;
- the production `dist/` bundle as a pre-packaging audit input only; it is not
  copied into the handoff;
- the tracked `assets/Aster-MVP-v2-preview.png`, labelled as a non-sensitive UI
  preview with its exact evidence scope and copied into `outputs/` by the
  packaging script.

Run the packaging script only after its version-specific input and output names
have been reviewed for `0.2.0`:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/package-engineering.ps1 `
  -EvidenceRecord "work\evidence\YYYY-MM-DD-<full-revision>-engineering-build.md" `
  -VerifierIdentity "self-declared-reviewer-or-ci-label"
```

The script requires the same clean Git revision recorded by the build identity,
validates every artifact/SBOM/`dist/` hash and the bounded evidence draft, and
labels the caller-provided verifier/job value as self-declared. It rejects a
missing, duplicate, contradictory, engineering-MVP, or production
classification; this unsigned packager cannot grant a higher release class. It
also rejects any existing output path and reparse point in protected paths,
reruns the package audit, and assembles into a fresh contained staging
directory. After copying, it
revalidates artifacts, SBOMs, controlled source files, and the identity; creates
the source snapshot from the exact revision with `git archive`; validates every
ZIP file, exact parent-directory entry, external Git mode, and Git blob against
`source-inventory.json`; verifies the final clean `HEAD`, exact allowlist, and
checksums while the candidate still has a non-final staging name. The final
same-volume directory rename is the sole publication action and the last
fallible action that can grant the final handoff name; no verification or
cleanup is attempted against the published path afterward. A failed rename may
leave an unpublished candidate only under its ignored random staging name;
safe cleanup refuses reparse points rather than following them. Ignored and
untracked workspace files remain excluded by construction. The output directory
must be absent before each run.
The handoff includes `source-inventory.json` beside the source ZIP so the exact
ordinal path, mode, and Git blob mapping remains reviewable.

The engineering workflow does not by itself establish that all third-party
copyright notices and license texts required for binary redistribution have been
collected and packaged. Record that gate according to the actual evidence; a
required `NOT RUN` blocks public binary distribution but does not block source
publication under Apache-2.0.

Verify copied `0.2.0` artifacts against the checksum file. The project helper
does not depend on the optional `Get-FileHash` cmdlet:

```powershell
Import-Module .\scripts\Aster.BuildIdentity.psm1 -Force
Get-AsterFileIdentity -RepositoryRoot . -Path outputs\Aster-0.2.0-x64-engineering.exe
Get-AuthenticodeSignature outputs\Aster-0.2.0-x64-engineering.exe
Get-AuthenticodeSignature outputs\Aster-0.2.0-x64-engineering-setup.exe
```

An observed `NotSigned` status is expected from the unsigned engineering
workflow. If `Microsoft.PowerShell.Security` cannot be loaded, record the
signature procedure as `NOT RUN (verifier unavailable)`; do not treat it as
signature evidence. The checksum/inspection procedure may pass while the
production signing gate remains `NOT RUN`.

## 8. Production promotion boundary

Do not promote an engineering bundle to production. Production additionally
requires an authorized Windows signing identity, verifiable installer and binary
signatures, post-signing hashes, signed provenance, complete Windows E2E and
package evidence, and every other in-scope criterion to pass. Automatic updates
remain disabled until a signed update chain is specified and verified.

If a release requirement cannot be met, retain the result as `FAIL` or `NOT RUN`,
document the residual risk, and stop at the permitted lower release class.
Exceptions must satisfy every field and prohibition in [`AGENTS.md`](../AGENTS.md).

## Engineering handoff contents

For explicitly local evaluation, keep these items together:

- clearly labelled unsigned installer and portable executable;
- identified source revision and tracked-file source snapshot;
- revision-specific report with every `FAIL` and `NOT RUN` item;
- SHA-256 checksum file obtained through a trusted channel;
- frontend and Rust SBOMs with their manifest-derived scope;
- Apache-2.0 `LICENSE`, `NOTICE`, and applicable third-party license material;
- residual risks, valid exceptions, and safe installation/removal instructions.
