# Aster Desktop Roadmap

**Status:** non-normative planning document

**Last updated:** 2026-07-12

This roadmap communicates likely sequencing. It does not authorize implementation, grant permissions, or override [AGENTS.md](../AGENTS.md), accepted ADRs, security requirements, or the product specification. Every behavior-changing item starts with stable requirements and acceptance criteria.

## Phase 1 — MVP acceptance hardening

The highest priority is converting existing lower-layer evidence into complete acceptance evidence for the Windows engineering build.

- Run clean-profile Windows 11 install, launch, restart, uninstall, and data-retention scenarios.
- Exercise the native credential prompt and an isolated Windows Credential Manager target with unmistakably fake sentinels.
- Add controlled HTTPS/SSE integration for certificate, redirect, timeout, retry, malformed stream, partial UTF-8, cancellation, and terminal-race cases.
- Complete packaged IPC, CSP, external-navigation, import/export-dialog, SQLite migration, and package-inventory verification.
- Execute keyboard-only, screen-reader, reduced-motion, contrast, and 100%/150%/200% scaling matrices.
- Record repeatable responsiveness, stop-latency, streaming, and bounded large-data measurements.

Exit condition: every criterion required for the Engineering MVP classification has revision-specific `PASS` evidence, with residual risks explicit.

## Phase 2 — Production release readiness

- Select and document an open-source or proprietary licensing model.
- Establish protected GitHub collaboration, required reviews, and retained CI evidence.
- Provision an authorized Windows signing identity and document secure key custody.
- Produce a binary-derived package inventory and reconcile it with CycloneDX dependency SBOMs.
- Verify installer and artifact signatures after packaging and demonstrate tamper-negative behavior.
- Create a signed, provenance-bound release workflow.
- Keep automatic updates disabled until signed metadata, artifact, origin, rollback, and tamper tests pass.

Exit condition: every in-scope production gate passes for the exact signed artifact and identified source revision.

## Phase 3 — Post-MVP product discovery

These ideas are explicitly outside MVP v1 and require separate product discovery, privacy review, architecture decisions, threat modeling, and acceptance criteria before implementation:

- Project/workspace organization.
- File and document attachments.
- Local retrieval-augmented generation.
- Voice input and output.
- Optional additional verified models or providers.
- Signed automatic updates.

Arbitrary shell execution, unrestricted filesystem access, hidden model-invoked tools, and silent data transmission are not roadmap commitments.

## Decision principles

Prioritize work in this order:

1. Vulnerability remediation and trust-boundary defects.
2. Missing evidence for existing `MUST` requirements.
3. Accessibility, reliability, recovery, and maintainability.
4. Production supply-chain assurance.
5. New product capability.

An implementation does not become in scope merely because it appears on this roadmap.
