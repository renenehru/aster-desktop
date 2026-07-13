# Verification Evidence Policy

This directory defines how revision-specific verification is recorded. This `README.md` is policy, not test evidence. Unless a separate record identifies a source revision and result, every acceptance criterion remains `NOT RUN`.

## 1. Allowed outcomes

Every gate or acceptance result uses exactly one outcome:

- `PASS`: the documented procedure ran against the identified source or artifact and every assertion in its stated scope succeeded.
- `FAIL`: at least one assertion failed, the run was incomplete after starting, or the result is materially ambiguous.
- `NOT RUN`: the procedure or required evidence is missing, stale, unavailable, or from the wrong environment.

No result is inferred from a document, screenshot, generated file, source inspection, build exit code, or another test layer. A result covers only the assertions actually exercised. Missing tools, a missing Windows host, a missing signer, or a missing artifact is `NOT RUN`.

## 2. Required record fields

Name release records `YYYY-MM-DD-<revision>-<release-class>.md`. Each result row records:

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

Secret values, authorization headers, prompts, responses, private imports, database rows, and personal paths are prohibited in evidence. Use unique fake sentinels and redact before retention.

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

**Artifact:** <path and SHA-256, or not applicable>

**Environment:** <details>

**Overall classification:** <browser preview, engineering MVP build, or production release>

| Criterion/gate | Outcome   | Procedure          | Evidence           | Notes          |
| -------------- | --------- | ------------------ | ------------------ | -------------- |
| `AC-001`       | `NOT RUN` | <command or steps> | <location or none> | <reason/scope> |

## Failures and retained retries

<List every failure and any later retry; do not erase the original result.>

## Residual risk and exceptions

<List active risks and valid exception records.>
```

Production classification is prohibited while any required criterion is `FAIL` or `NOT RUN`. An engineering build may have explicitly identified production-only `NOT RUN` items, but runtime controls for credentials, networking, rendering, IPC, storage, and privacy still require evidence before it is called an engineering MVP build.
