# Architecture Decision Records

Accepted decisions define the deliberate MVP architecture. `Accepted` means the decision is normative; it does not mean implementation or acceptance evidence exists.

| ID         | Decision                                                                                                                            | Primary requirements                                                   | Primary threats                                  |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- | ------------------------------------------------ |
| `ADR-0001` | [Keep credentials and provider networking behind the Rust boundary](0001-rust-trust-boundary.md)                                    | `PR-002`, `FR-006`-`FR-008`, `SEC-001`-`SEC-011`, `SEC-017`-`SEC-021`  | `TM-001`-`TM-004`, `TM-012`-`TM-020`             |
| `ADR-0002` | [Store conversations locally and credentials separately](0002-local-data-and-secrets.md)                                            | `FR-004`, `FR-006`, `FR-015`, `SEC-001`-`SEC-004`, `SEC-022`-`SEC-032` | `TM-001`-`TM-005`, `TM-022`-`TM-029`             |
| `ADR-0003` | [Keep privileged tools and attachments outside MVP v1](0003-mvp-capability-boundary.md)                                             | `FR-001`, `SEC-017`, `SEC-021`, `SEC-030`, `SEC-039`                   | `TM-007`, `TM-009`, `TM-010`, `TM-012`, `TM-033` |
| `ADR-0004` | [Isolate the browser demo from desktop capabilities and secrets](0004-browser-demo-isolation.md)                                    | `SEC-005`, `SEC-031`, `SEC-039`                                        | `TM-006`, `TM-029`, `TM-033`, `TM-034`           |
| `ADR-0005` | [Pin the verified `glm-5.1` contract and use honest application response profiles](0005-provider-contract-and-response-profiles.md) | `FR-007`, `FR-013`, `SEC-006`-`SEC-010`                                | `TM-017`-`TM-021`                                |
| `ADR-0006` | [Open validated HTTPS links through a Rust-scoped system opener](0006-rust-scoped-external-links.md)                                | `FR-009`, `SEC-015`, `SEC-017`, `AC-024`, `AC-025`                     | `TM-006`, `TM-012`, `TM-014`                     |
| `ADR-0007` | [Use a narrow custom Windows title bar](0007-custom-windows-titlebar.md)                                                            | `FR-001`, `UX-001`, `UX-005`, `SEC-017`, `AC-001`, `AC-025`, `AC-035`  | `TM-006`, `TM-012`, `TM-033`                     |
| `ADR-0008` | [Capture provider credentials outside the webview](0008-native-credential-capture.md)                                               | `FR-006`, `SEC-001`-`SEC-005`, `SEC-017`, `SEC-018`, `SEC-020`         | `TM-001`-`TM-003`, `TM-006`, `TM-012`, `TM-035`  |
| `ADR-0009` | [Persist no application diagnostics in MVP v1](0009-no-persistent-diagnostics.md)                                                   | `SEC-003`, `SEC-020`, `SEC-023`, `SEC-029`-`SEC-031`                   | `TM-002`, `TM-004`, `TM-029`, `TM-034`           |

## ADR rules

- IDs are permanent. A superseded ADR remains in the repository and links to its replacement.
- An ADR cannot silently weaken `AGENTS.md` or a security requirement.
- A change to a trust boundary, dependency class, persistence strategy, provider contract, privilege, or release security posture requires an ADR review.
- Evidence belongs in a revision-specific record under [../evidence/](../evidence/); it is not inferred from ADR status.
