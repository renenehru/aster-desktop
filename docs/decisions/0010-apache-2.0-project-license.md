# ADR-0010: License Project-Owned Work Under Apache-2.0

**Status:** Accepted

**Date:** 2026-07-13

**Decision owners:** `SEC-043`-`SEC-045`, `AC-037`-`AC-039`

## Context

Aster Desktop is intended for public inspection, reuse, modification, and
community collaboration. The repository previously had no open-source license,
so recipients had no general permission to copy, modify, or redistribute the
project. A public collaboration model requires explicit, durable terms that are
compatible with commercial and non-commercial use while preserving attribution,
patent, notice, and warranty boundaries.

The repository also contains separately licensed material. Portions of the Code
of Conduct are adapted from Contributor Covenant 2.1 under CC BY 4.0, and
third-party dependencies retain their own licenses. A project-level license
cannot silently replace those terms.

Licensing a published revision is difficult to reverse because recipients may
continue relying on the license granted for copies they already obtained. The
decision therefore requires an accepted ADR rather than an undocumented metadata
change.

## Decision

Project-owned source code, documentation, configuration, and assets are licensed
under the Apache License, Version 2.0, unless an item is explicitly identified as
separately licensed. The repository carries the unmodified canonical license
text in root `LICENSE` and preserves project and third-party attribution in
`NOTICE`.

JavaScript, Rust, Tauri bundle, source-package, installed-package, and SBOM root
metadata use the SPDX identifier `Apache-2.0`. The npm `private` flag and Cargo
`publish = false` settings remain in place as accidental registry-publication
guards; they do not restrict source rights granted by the license.

Contributions intentionally submitted for inclusion are provided under
Apache-2.0 without additional terms unless the contributor explicitly states
otherwise or a separate written agreement applies. Contributors retain their
copyright and must disclose third-party material and preserve its provenance and
terms. No copyright assignment or separate contributor license agreement is
required for MVP v1.

The Code of Conduct exception remains identified as CC BY 4.0 with the required
Contributor Covenant and Mozilla attribution. Dependency policy reports and
manifest-derived SBOMs inventory license expressions, but they do not by
themselves prove binary redistribution compliance. Public binary or installer
distribution remains blocked until `AC-039` passes with all applicable
third-party notices and license texts packaged and verified.

## Consequences

- Anyone may use, reproduce, modify, and distribute project-owned work subject
  to Apache-2.0, including its license, notice, modified-file, patent,
  trademark, and warranty provisions.
- Published Apache-2.0 grants remain available for already received copies even
  if a future revision adopts different terms.
- Public collaboration can use forks and pull requests without a separate
  copyright-assignment workflow.
- Every release and source archive must preserve `LICENSE`, `NOTICE`, explicit
  exceptions, and consistent package metadata.
- Third-party license obligations remain separate and may require additional
  files before a binary is legally ready for public redistribution.

## Rejected alternatives

- Keep the repository source-available without an explicit license.
- Use a custom license that would add interpretation and compatibility risk.
- Apply Apache-2.0 metadata to third-party material without preserving its
  original license and attribution.
- Treat dependency-license reports or SBOM expressions as a substitute for
  packaging required third-party license texts.
- Require a copyright assignment for routine community contributions in MVP v1.

## Verification

`AC-037` checks the canonical license text, attribution, exceptions, metadata,
and package presence. `AC-038` reconciles component license expressions in both
SBOMs with locked manifest metadata. `AC-039` remains `NOT RUN` until an exact
binary and installer include all reviewed third-party redistribution material.
The static licensing policy and security configuration audit provide negative
fixtures for missing, changed, or conflicting declarations; revision-specific
CI evidence is still required before any release claim.
