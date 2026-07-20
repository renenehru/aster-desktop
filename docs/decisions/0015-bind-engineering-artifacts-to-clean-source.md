# ADR-0015: Bind Engineering Artifacts to One Clean Source Revision

**Status:** Accepted

**Date:** 2026-07-13

## Context

Generated executables, installers, frontend assets, SBOMs, evidence drafts, and
package output are ignored workspace files. A clean Git checkout alone therefore
does not prove that an existing binary or SBOM was produced from the current
`HEAD`. Requiring an evidence record to be tracked in the same commit whose hash
it declares is also circular: adding that record changes the commit hash.

## Decision

- The engineering build starts and ends with the same clean identified Git
  revision. A tracked source change during the build aborts identity creation.
- After a successful build, the build wrapper writes an ignored
  `work/build-identity.json` containing the exact source revision and SHA-256
  identities for the executable, installer, both SBOMs, and deterministic
  production-frontend tree digest. The packager accepts only the canonical
  duplicate-free JSON schema, exact JSON token types, integral byte counts in
  the interoperable safe range, and a real canonical UTC generation timestamp.
- Engineering packaging accepts artifacts only while `HEAD` still equals that
  identity. It recomputes every recorded hash/digest and fails on any mismatch.
- The pre-packaging evidence draft lives outside the source revision under
  ignored `work/evidence/`, has the full revision in its filename, and declares
  that exact revision once in a canonical field. The packager enforces a size
  cap, strict UTF-8, complete result structure, real UTC time window, and shared
  secret, Markdown-aware credential-header, and normalized personal-path scans.
  Each canonical result row repeats the exact source, environment, UTC window,
  and self-declared identity and retains an exact procedure, evidence location,
  artifact/hash status, and scope; malformed or indented table rows are
  rejected, and each gate has one current row. A non-empty, size-bounded,
  strict-UTF-8 sibling log is mandatory, passes the same sensitive-data scans,
  and is copied into the handoff. Every `PASS` row references that exact
  repository-relative log; prior failures and retries remain in the dedicated
  history section. The engineering packager requires exactly one canonical
  `Unsigned engineering build for local evaluation` classification and rejects
  duplicate, contradictory, engineering-MVP, or production classifications.
  The packager's required verifier label is also explicitly self-declared;
  neither label is an authenticated reviewer identity.
- A reviewed durable evidence record may be committed later under
  `docs/evidence/` or retained as a protected CI artifact. That later record
  continues to identify the tested artifact source revision; its storage commit
  is not represented as the artifact source.
- The packager rejects reparse points and junctions in protected paths, assembles
  into a fresh contained staging directory, revalidates every copied identity,
  and compares every source ZIP file, parent-directory entry, external mode, and
  Git blob identity with an ordinal tracked-source inventory. It verifies the
  final clean `HEAD`, exact allowlist, and checksums before moving the staged
  directory to a previously absent, ignored output path on the same volume. All
  validation and fault-injection boundaries operate on the non-final candidate;
  the final same-volume directory rename is the sole publication action and the
  last fallible action that can grant the handoff name. No validation or cleanup
  runs against the published path afterward. A failed rename can leave the
  candidate only under its ignored random staging name, where safe cleanup
  refuses reparse points rather than following them.
- This is engineering-build provenance, not signed or cryptographic build
  attestation. A production release still requires authorized signing and
  verifiable provenance under `AC-031`.

## Consequences

- In the supported, unmodified workflow, ordinary stale, tampered, or mismatched
  executable, installer, SBOM, `dist/` tree, source archive, evidence, or staged
  copy is detected at the implemented verification boundaries.
- This unsigned local workflow does not authenticate the builder, the
  self-declared verifier label, or the scripts themselves and does not claim to
  resist a malicious actor who can modify the workflow and regenerate all local
  metadata consistently.
- Verification, build, evidence drafting, and packaging must occur without a
  source commit change between build and package assembly.
- Durable in-repository evidence is deliberately a later evidence-only change,
  avoiding a self-referential commit hash.
- Missing or mismatched identity is `NOT RUN`/`FAIL` for packaging as applicable;
  it is never inferred from filenames or timestamps.

## Rejected alternatives

- Trusting ignored artifact timestamps or versioned filenames.
- Archiving current `HEAD` while accepting binaries from an unidentified prior
  build.
- Requiring the evidence input to be tracked in the artifact source commit.
- Labelling the unsigned local identity manifest as signed provenance.

## Verification

Primary evidence is `AC-031` and `AC-033`, plus executable temporary-repository
fixtures for clean/dirty/wrong-`HEAD`, stale artifacts, altered SBOMs and staged
copies, duplicate/wrong-type/out-of-range manifest values, malformed evidence,
junctions, destination races with a locked staging child, unexpected output,
exact ZIP directories/modes, complete source inventory, final-state mismatch,
strict UTF-8, evidence size and retention, binary UTF-16 alignments, and shared
sensitive-data patterns.
