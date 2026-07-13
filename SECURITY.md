# Security Policy

## Supported versions

There is currently no supported production release.

| Version or branch          | Support status                                                                                    |
| -------------------------- | ------------------------------------------------------------------------------------------------- |
| `main`                     | Best-effort security fixes for the active engineering source; not a production-support commitment |
| Unsigned `0.1.x` artifacts | Engineering evaluation only; not a supported production release                                   |
| Signed production releases | None published                                                                                    |

## Reporting a vulnerability

Do not open an issue containing exploit details, API keys, conversation content, or personal data. Use [GitHub private vulnerability reporting](https://github.com/renenehru/aster-desktop/security/advisories/new) when it is available. If that intake is unavailable, contact the repository owner privately through GitHub and wait for confirmation of a secure reporting channel before sending technical details. A maintainer will preserve the report in a restricted security record and coordinate remediation without moving sensitive details into an issue or pull request.

Include only the minimum non-sensitive information needed to triage:

- The affected version and Windows build.
- Reproduction steps with synthetic data.
- The expected and observed security boundary.
- Impact and any known mitigations.

Never include a real provider credential. Revoke any credential that may have been exposed before reporting it.

## Response objectives

- Acknowledge a complete report within three business days.
- Triage severity and affected requirements within five business days.
- Block releases for confirmed critical or high-severity findings without an approved, time-bounded exception.
- Record the fix, regression test, and requirement traceability before release.

## Release security gates

A release candidate must satisfy `AGENTS.md`, including:

- No unresolved critical or high dependency findings.
- Frontend and Rust verification gates pass from a clean checkout.
- No secrets in source, artifacts, logs, or test fixtures.
- Threat-model and security-requirement deltas are reviewed.
- Windows binaries and update manifests are signed for production distribution.

Local unsigned development builds must not be presented as production releases.
