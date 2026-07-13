# Aster Desktop Governance

This document describes how people collaborate, make decisions, and maintain accountability in the Aster Desktop repository. It complements, but does not override, [AGENTS.md](AGENTS.md). When documents conflict, the source-of-truth order in `AGENTS.md` applies.

## Principles

Aster Desktop is governed by the following principles:

- **Specification before implementation**: behavior and security decisions begin with stable requirements and acceptance criteria.
- **Least privilege**: the project does not add dormant permissions, hidden capabilities, or speculative trusted interfaces.
- **Evidence over assertion**: review and release decisions use scoped, revision-specific evidence.
- **Transparent collaboration**: technical decisions, tradeoffs, and residual risks are recorded in the repository unless disclosure would create a security or privacy risk.
- **Respectful participation**: all contributors follow [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
- **No special path for automation**: AI-assisted and automated contributions follow the same authorship, review, security, testing, and evidence requirements as human contributions.

## Roles

Roles are based on current repository permissions and demonstrated responsibility; they are not permanent titles.

### Repository owner

The repository owner controls GitHub access, appoints or removes maintainers, and is the final escalation point for project direction and conduct matters. The owner must not override the non-negotiable security controls in `AGENTS.md` or classify an artifact beyond its evidence.

### Maintainers

Maintainers have repository write or maintain permissions and are responsible for:

- Triaging issues and reviewing pull requests.
- Enforcing the specification-driven workflow and trust boundaries.
- Protecting security reports and other sensitive information.
- Ensuring decisions and evidence remain discoverable.
- Managing branches, releases, collaborator access, and repository settings.
- Identifying conflicts of interest and requesting an independent reviewer when necessary.

### Contributors

Contributors propose changes through issues, discussions, or pull requests. They are responsible for keeping changes focused, following `AGENTS.md` and [CONTRIBUTING.md](CONTRIBUTING.md), adding appropriate tests and documentation, and responding constructively to review.

### Reviewers

Reviewers evaluate correctness, specification alignment, security impact, maintainability, accessibility, and evidence quality. A reviewer must not approve a change they do not understand. Authors may review discussion, but they do not provide the independent approval for their own change.

### Security responders and release stewards

Maintainers may designate trusted people to coordinate private vulnerability handling or a release candidate. These assignments are scoped to the specific report or release and do not bypass normal review or evidence requirements.

## Decision process

### Routine changes

Routine, reversible changes are decided through pull-request review. Maintainers should seek consensus among active participants. One independent maintainer approval is the minimum merge decision when repository permissions make that practical.

### Material changes

A change is material when it changes product behavior, a trust boundary, data handling, permissions, an external interface, a dependency class, or the supported security posture. Before implementation, it must follow the workflow in `AGENTS.md`: specify, model, decide when necessary, trace, test, implement, verify, and review drift.

Material changes should receive review from at least one maintainer familiar with the affected boundary. Security-sensitive changes should receive an additional independent security-focused review when another qualified reviewer is available. Lack of a second reviewer must be stated as residual review risk; it is not evidence that a gate passed.

### Architecture decisions

Hard-to-reverse decisions are recorded as Architecture Decision Records under `docs/decisions/`. An accepted ADR may be superseded only by a new ADR that explains the reason, migration, compatibility impact, and rollback plan. Historical ADRs remain in the repository.

### Disagreement and escalation

Participants should first identify the controlling requirement, evidence, and tradeoff. If consensus is not reached, a maintainer records the competing options and makes a reasoned decision consistent with the source-of-truth order. The repository owner resolves remaining project-direction disputes. Security controls and evidence requirements cannot be waived informally.

## Contribution and merge workflow

Changes should use a short-lived branch and a focused pull request. Direct pushes to the default branch are discouraged. The GitHub repository should be configured to require review and applicable status checks before merge; until such protection is verified, maintainers must enforce the same policy manually.

A pull request must:

1. Explain its purpose, scope, and non-goals.
2. List changed requirement and acceptance-criterion identifiers, or explain why the change is non-behavioral.
3. Identify reviewed threat and ADR identifiers where applicable.
4. Include tests and revision-specific evidence with honest `PASS`, `FAIL`, or `NOT RUN` outcomes.
5. Record residual risk, follow-up work, and any release impact.
6. Update user, developer, security, and architecture documentation as required.

Maintainers may request that unrelated changes be split. Merge does not imply that the result is a production release.

## Security and incident decisions

Suspected vulnerabilities are handled privately according to [SECURITY.md](SECURITY.md) and take priority over feature work. Public issues and pull requests must not contain exploit details, real credentials, private conversation data, or sensitive forensic evidence.

An emergency remediation may use an expedited private review, but it still requires a documented threat assessment, regression test where practical, evidence, and a follow-up review. Exceptions must meet every condition in `AGENTS.md`; prohibited exceptions are never acceptable.

## Release governance

A version tag, installer, or successful build is not by itself a release approval. A release steward must review the applicable evidence record, artifact hashes, residual risks, required gate outcomes, and signing status.

- An artifact with required `FAIL` or `NOT RUN` gates must not be described as a production release.
- Unsigned installers and update artifacts are engineering builds only.
- Evidence applies only to the exact source revision and artifact hash recorded.
- Superseded failures remain visible in the evidence history.
- Auto-update remains disabled until a signed update chain and rollback plan are verified.

## Access and succession

Repository access follows least privilege. Maintainers should grant the narrowest GitHub role needed, review collaborator access periodically, and remove access that is no longer required. At least two trusted maintainers should have administrative recovery access when the project grows enough to support it.

When a maintainer steps down, they should transfer open security, release, and review responsibilities without disclosing secrets. The repository owner appoints successors based on sustained, constructive contributions and demonstrated care for the project's security boundary.

## Amendments

Governance changes use a pull request with an explicit rationale and maintainer review. A governance amendment is non-product behavior unless it changes an engineering control; changes to an engineering control must also update the controlling specification or decision record where required by `AGENTS.md`.
