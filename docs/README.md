# Aster Documentation

This directory contains the MVP v2 product contract, multi-provider security
model, architecture, verification contract, and contributor runbooks for Aster
Desktop. Read
[`AGENTS.md`](../AGENTS.md) before making any change: it is the repository-wide
engineering governance and has precedence over every document listed here.

## Start here

| If you want to...                             | Read...                                                    |
| --------------------------------------------- | ---------------------------------------------------------- |
| Compare MVP v1 and MVP v2 before downloading  | [Version selection guide](version-selection.md)            |
| Understand the MVP outcome and non-goals      | [Product specification](product-spec.md)                   |
| Understand components and trust boundaries    | [Architecture](architecture.md)                            |
| Set up a Windows development environment      | [Development guide](development.md)                        |
| Work across laptops and collaborate on GitHub | [GitHub collaboration workflow](collaboration-workflow.md) |
| Choose and run the correct verification layer | [Testing guide](testing.md)                                |
| Prepare and classify an artifact              | [Release process](release-process.md)                      |
| Resolve a local build or runtime problem      | [Troubleshooting](troubleshooting.md)                      |
| Contribute a change                           | [Contributing guide](../CONTRIBUTING.md)                   |
| Report a vulnerability                        | [Security policy](../SECURITY.md)                          |
| Understand reuse and redistribution terms     | [Apache-2.0 license](../LICENSE) and [NOTICE](../NOTICE)   |

## Normative product and security documents

These documents define required behavior and verification. Contributor guides
summarize how to work with them but do not replace them.

| Document                                          | Purpose                                                                                   |
| ------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| [Product specification](product-spec.md)          | Product outcomes, functional behavior, UX, accessibility, and non-functional requirements |
| [Security requirements](security-requirements.md) | Credential, network, rendering, IPC, storage, privacy, and supply-chain controls          |
| [Architecture](architecture.md)                   | Components, data flows, runtime profiles, persistence, and trust boundaries               |
| [Threat model](threat-model.md)                   | Assets, threat register, misuse cases, residual risks, and review triggers                |
| [Acceptance criteria](acceptance-criteria.md)     | Executable `AC-` scenarios and requirement-to-evidence traceability                       |
| [Provider contract](provider-contract.md)         | Dated contracts for the closed five-provider, 17-model catalog and response mappings      |
| [Architecture decisions](decisions/README.md)     | Accepted, durable design decisions and their requirement/threat mappings                  |

When sources conflict, use the precedence order in
[`AGENTS.md`](../AGENTS.md): governance, accepted ADRs, security requirements,
product specification, acceptance criteria, then implementation and tests.

## Engineering and operations guides

- [Version selection](version-selection.md) compares the historical MVP v1
  source baseline with the current MVP v2 development line, including exact
  source links, migration limits, support status, and licensing caveats.
- [Development](development.md) explains prerequisites, repository layout,
  fresh-laptop bootstrap, desktop and isolated browser-demo workflows, and the
  spec-first change process.
- [GitHub collaboration](collaboration-workflow.md) explains multi-laptop
  synchronization, branches, pull requests, review, and collaborator access.
- [Testing](testing.md) maps commands to the evidence they can and cannot
  produce, including local and CI gates.
- [Release process](release-process.md) covers engineering packaging, evidence,
  artifact identity, and the additional controls required for production.
- [Troubleshooting](troubleshooting.md) contains safe recovery guidance that
  preserves user data, provider credential separation, and security boundaries.

## Evidence

The [evidence policy](evidence/README.md) defines the only allowed outcomes:
`PASS`, `FAIL`, and `NOT RUN`. A result is revision- and artifact-specific and
proves only the procedure that actually ran. Browser screenshots, source
inspection, and unit tests cannot be promoted into evidence for native IPC,
Windows Credential Manager, provider networking, SQLite persistence, installer
behavior, or signing.

Reviewed durable records may live under [`docs/evidence/`](evidence/) or as
protected CI artifacts. An ignored local packaging draft lives under
`work/evidence/` so it does not change the source revision it identifies.
Retain failed attempts alongside successful retries, and never place secrets,
conversation content, provider payloads, database rows, or personal paths in an
evidence record.

## Documentation conventions

- Product decisions, documentation, UI copy, configuration, tests, commits, and
  evidence are written in English.
- Normative requirements use permanent IDs owned by their canonical document.
- A behavior-changing documentation update follows the same specify, model,
  decide, trace, test, implement, verify, and drift-review workflow as code.
- A contributor guide may explain current tooling, but it must not claim that a
  gate passed. Only a compliant revision-specific evidence record may do that.
