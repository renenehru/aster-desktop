# ADR-0014: Keep Balance Read-Only and Account Changes in the Default Browser

**Status:** Accepted

**Date:** 2026-07-13

**Extends:** `ADR-0006` with a separate account-action command

**Decision owners:** `PR-005`, `FR-027`, `FR-028`, `SEC-050`, `SEC-051`

## Context

Users need a way to assess provider usage and obtain more capacity without giving Aster payment, plan, or arbitrary browser authority. DeepSeek documents a read-only exact balance endpoint; the other catalog providers do not expose an equivalent verified MVP endpoint. Account websites require URLs, but accepting a renderer/model-provided URL would create a phishing and privilege boundary.

## Decision

- Only an explicit user action may refresh exact balance, and only provider `deepseek` may call `GET https://api.deepseek.com/user/balance`.
- Rust applies the DeepSeek key, exact-origin TLS, tight response/time bounds, decimal-safe validation, and safe errors. There is no body mutation, automatic retry, or background polling.
- The most recent successful balance is memory-only for the current session. If a later refresh fails, it may remain visibly stale with its last-success time; it never enters SQLite, logs, exports, or browser storage. Each refresh has a Rust-owned credential generation and latest-operation authority. A successful DeepSeek credential replace/delete atomically invalidates earlier refreshes, clears memory, and resets reachability; only the current authority can commit balance and reachability together.
- Other providers show `Check balance on provider website` and have no balance-network path.
- The account command accepts only provider ID and the exact JSON action enum `usage | billing | addCredits | spend | deployment`.
- Rust maps each supported pair to the fixed official URL in the provider contract. NVIDIA `deployment` uses one general NVIDIA Build destination. React supplies neither model ID nor URL.
- The operating system opens the destination in the default browser. Aster does not embed the page, authenticate the browser, submit a form, buy credits, alter a plan, observe completion, or report success for an external transaction.

## Consequences

- Balance semantics are accurate where verified and explicitly absent elsewhere.
- Provider website changes remain outside Aster's control; the fixed map requires periodic review.
- Account action testing is finite and separate from untrusted Markdown link handling in `ADR-0006`.
- Users complete financial/account operations under provider controls in their normal browser.

## Rejected alternatives

- In-app purchase, plan management, payment fields, automatic top-up, or provider mutation APIs.
- Background balance polling or inferring money from local tokens.
- A generic `open_url` account command, renderer-supplied model ID/URL, or embedded provider webview.
- Claiming an external transaction succeeded because the browser opened.

## Verification

Primary evidence is `AC-046` and `AC-047`, plus `AC-019`, `AC-021`, `AC-024`, `AC-025`, and packaged Windows browser-boundary evidence.
