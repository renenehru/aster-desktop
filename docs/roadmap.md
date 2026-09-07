# Aster Desktop Roadmap

**Status:** non-normative planning document

**Last updated:** 2026-07-13

This roadmap communicates likely sequencing. It does not authorize
implementation, grant permissions, or override [`AGENTS.md`](../AGENTS.md),
accepted ADRs, security requirements, or the product specification. Every
behavior-changing item starts with stable requirements and acceptance criteria.

## Phase 1 — MVP v2 implementation and acceptance

The immediate objective is to implement and verify the specified direct
multi-provider boundary without broadening its closed catalog. MVP v2 contains
exactly 5 providers and 17 model identifiers documented in the
[provider contract](provider-contract.md).

- Complete the Rust-owned catalog, exact provider adapters, profile mappings,
  bounded stream/usage parsers, cancellation, and one-attempt request policy.
- Complete provider-scoped native credential setup with one Windows Credential
  Manager target per provider and no secret-bearing React or IPC path.
- Complete accessible catalog selection and enforce the new-conversation flow
  when a non-empty conversation has locked its provider/model pair.
- Complete the local trailing-seven-day Usage ledger, partial-coverage states,
  per-provider advisory budget, and red-plus-text/icon warning at 10% remaining
  or below.
- Complete explicit read-only DeepSeek balance refresh and fixed account actions
  that open official pages in the default browser without buying credits or
  changing plans.
- Verify the transactional v1-to-v2 migration, including retained
  `zai` / `glm-5.1` conversations and partial historical token breakdowns.
- Exercise contract fixtures for every catalog pair without contacting a
  billable endpoint in default tests.
- Run clean-profile Windows 11 install, launch, restart, uninstall, database
  migration, and data-retention scenarios.
- Exercise distinct isolated credential targets with unmistakably fake values.
- Complete controlled HTTPS/SSE tests for certificates, redirects, bounds,
  timeout, malformed streams, partial UTF-8, cancellation, and terminal races.
- Execute keyboard-only, screen-reader, reduced-motion, contrast, and
  100%/150%/200% Windows scaling matrices, including the model and Usage dialogs.
- Record live-provider compatibility only in explicitly authorized,
  credential-scoped evidence. Do not infer it from fixture tests or a browser
  preview.

Exit condition: every criterion required for the Engineering MVP classification
has revision-specific `PASS` evidence, and all residual risks and `NOT RUN`
outcomes are explicit.

## Phase 2 — Production release readiness

- Maintain Apache License 2.0 metadata and third-party attribution for every
  source and binary handoff.
- Maintain protected GitHub collaboration, required reviews, and retained CI
  evidence across contributor laptops.
- Provision an authorized Windows signing identity and document secure key
  custody outside the repository.
- Produce a binary-derived package inventory and reconcile it with CycloneDX
  dependency SBOMs.
- Verify executable and installer signatures after packaging and demonstrate
  tamper-negative behavior.
- Create a signed, provenance-bound release workflow.
- Keep automatic updates disabled until signed metadata, artifact, origin,
  rollback, and tamper tests pass.

Exit condition: every in-scope production gate passes for the exact signed
artifact and identified source revision. An unsigned engineering build cannot be
promoted by relabelling it.

## Phase 3 — Post-MVP product discovery

These ideas remain outside MVP v2 and require separate product discovery,
privacy review, architecture decisions, threat modeling, and acceptance criteria
before implementation:

- Project and workspace organization.
- File and document attachments.
- Local retrieval-augmented generation.
- Voice input and output.
- Additional providers or models only after their exact official contracts meet
  the catalog admission policy.
- Signed automatic updates.

Arbitrary shell execution, unrestricted filesystem access, hidden model-invoked
tools, caller-defined endpoints, and silent data transmission are not roadmap
commitments.

## Decision principles

Prioritize work in this order:

1. Vulnerability remediation and trust-boundary defects.
2. Specification/implementation drift in the existing MVP v2 scope.
3. Missing evidence for existing `MUST` requirements.
4. Accessibility, reliability, recovery, and maintainability.
5. Production supply-chain assurance.
6. New product capability.

An implementation does not become in scope merely because it appears on this
roadmap.
