# Verification Evidence Policy

This directory defines how revision-specific verification is recorded. This `README.md` is policy, not test evidence. Unless a separate record identifies a source revision and result, every acceptance criterion remains `NOT RUN`.

## 1. Allowed outcomes

Every gate or acceptance result uses exactly one outcome:

- `PASS`: the documented procedure ran against the identified source or artifact and every assertion in its stated scope succeeded.
- `FAIL`: at least one assertion failed, the run was incomplete after starting, or the result is materially ambiguous.
- `NOT RUN`: the procedure or required evidence is missing, stale, unavailable, or from the wrong environment.

No result is inferred from a document, screenshot, generated file, source inspection, build exit code, or another test layer. A result covers only the assertions actually exercised. Missing tools, a missing Windows host, a missing signer, or a missing artifact is `NOT RUN`.

## 2. Required record fields

Name release records `YYYY-MM-DD-<revision>-<release-class>.md`. A local
pre-packaging draft lives under ignored `work/evidence/` so creating it does not
change the clean revision it identifies. A reviewed durable copy may be
committed later under `docs/evidence/` or retained as a protected CI artifact;
the later storage commit must not be represented as the tested artifact source
revision. The record-level source, environment, time window, and procedure
identity are canonical summary values. Every result row repeats those exact
values so a row remains attributable when quoted, transformed, or reviewed
separately. It also records an exact command/procedure, evidence location,
artifact/hash status, and scope:

| Field          | Required content                                                                                      |
| -------------- | ----------------------------------------------------------------------------------------------------- |
| Criterion/gate | Stable `AC-` ID or exact gate name                                                                    |
| Outcome        | `PASS`, `FAIL`, or `NOT RUN`                                                                          |
| Source         | Commit/revision identifier and clean/dirty state                                                      |
| Artifact       | Exact file name and SHA-256 when an artifact is tested                                                |
| Environment    | OS, architecture, relevant runtime/tool versions, and display/accessibility settings where applicable |
| Procedure      | Exact command or deterministic manual steps                                                           |
| Time           | Start/end UTC timestamps                                                                              |
| Evidence       | Repository-relative or protected CI artifact location                                                 |
| Identity       | CI job or reviewer who performed the procedure                                                        |
| Scope/notes    | Assertions covered, exclusions, failures, and linked exception if any                                 |

Secret values, authorization headers, prompts, responses, private imports, database rows, and personal paths are prohibited in evidence. This includes Markdown-prefixed credential headers and Windows user paths written with forward slashes, doubled JSON escapes, or Unicode slash escapes. Use unique fake sentinels and redact before retention.

For engineering packaging, the filename's hexadecimal revision segment must be
the complete 40-to-64-character lowercase `Source revision`, and the record must
contain exactly one canonical `**Source revision:** <revision>` line. The draft
is strict UTF-8 without a byte-order mark, is at most 256 KiB, retains the
minimal record structure below, and passes the shared secret, credential-header,
and normalized personal-path scans. A non-empty strict-UTF-8 sibling log named
`YYYY-MM-DD-<revision>-engineering-build.log` is mandatory, is limited to 4 MiB,
passes the same sensitive-data scans, and is copied into the handoff as
`verification-evidence.log`. Every `PASS` row must name that exact
repository-relative sibling-log path; descriptive, missing, temporary, or
unretained evidence text cannot support `PASS`. Every draft supplies one real
ordered UTC time window and one explicitly self-declared procedure identity.
Each result row repeats the canonical source, environment, UTC window, and
identity and supplies a non-placeholder exact procedure, evidence location,
artifact/hash status, and scope note. A gate has exactly one current result row;
earlier failures and retries remain in `Failures and retained retries`. Indented
or otherwise malformed pipe-delimited rows are rejected. The clean-source build identity binds the executable,
installer, SBOMs, and production frontend tree to the same commit. A filename,
timestamp, version string, self-declared verifier label, or later evidence
commit is never a substitute for that binding or authenticated provenance.

## 3. Evidence-scope rules

| Evidence                       | What it may support                                                                                 | What it cannot support by itself                                                                    |
| ------------------------------ | --------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| Browser demo screenshot/test   | Layout, copy, keyboard, and narrowly scoped accessibility behavior                                  | Desktop IPC, provider, credential, SQLite, native dialog, package, installer, or signature behavior |
| Source/configuration review    | Presence or absence of an implementation/configuration pattern                                      | Runtime behavior or packaged effective policy                                                       |
| Unit/component test            | Isolated logic or rendered component assertion                                                      | OS integration, real package, or cross-process behavior                                             |
| Rust integration/contract test | Backend validation, repository, fake provider, parser, or cancellation assertion actually exercised | Packaged Windows UI or production provider availability                                             |
| Packaged Windows E2E           | Identified artifact behavior in the recorded environment                                            | Signing/provenance unless separately verified                                                       |
| Signature/SBOM/audit artifact  | The exact artifact/revision and tool scope recorded                                                 | Functional correctness or another revision                                                          |

## 4. Minimal record template

```markdown
# Verification Record: <revision and release class>

**Source revision:** <identifier; clean or dirty>

**Artifact:** <path; SHA-256: 64-lowercase-hex, or Not applicable — reason>

**Environment:** <OS, architecture, runtimes/tools, and relevant settings>

**Started UTC:** <real yyyy-MM-ddTHH:mm:ssZ timestamp>

**Completed UTC:** <real yyyy-MM-ddTHH:mm:ssZ timestamp not before start>

**Procedure identity (self-declared):** <CI job or reviewer label>

**Overall classification:** <browser preview, engineering MVP build, or production release>

| Criterion/gate | Outcome   | Source revision/state | Environment   | Started/completed UTC | Procedure identity | Exact command or procedure | Evidence           | Artifact/hash               | Scope/notes    |
| -------------- | --------- | --------------------- | ------------- | --------------------- | ------------------ | -------------------------- | ------------------ | --------------------------- | -------------- |
| `AC-001`       | `NOT RUN` | <full revision> clean | <environment> | <start> to <complete> | <identity>         | <exact command or steps>   | <location or none> | <SHA-256 or Not applicable> | <reason/scope> |

## Failures and retained retries

<List every failure and any later retry; do not erase the original result.>

## Residual risk and exceptions

<List active risks and valid exception records.>
```

Production classification is prohibited while any required criterion is `FAIL` or `NOT RUN`. An engineering build may have explicitly identified production-only `NOT RUN` items, but runtime controls for credentials, networking, rendering, IPC, storage, and privacy still require evidence before it is called an engineering MVP build.

`scripts/package-engineering.ps1` is deliberately more conservative than the
general template: its input must contain exactly
`**Overall classification:** Unsigned engineering build for local evaluation`.
It rejects duplicate, `engineering MVP build`, and `production release`
classification lines. A higher classification requires its separate complete
acceptance and release process; this unsigned packager cannot grant it.
