# Z.AI `glm-5.1` Provider Contract Baseline

**Purpose:** External contract input for MVP v1

**Documentation reviewed:** 2026-07-12

**Implementation evidence:** Not asserted by this document

**Supersedes:** The attached source plan's unaccepted `glm-5.2`, `enable_thinking`, and `reasoning_effort` assumptions

## 1. Authority and evidence boundary

This record captures the official Z.AI interface used to write requirements and contract tests. It is not a substitute for request snapshots, fake-server integration tests, packaged network-policy tests, or an authorized live compatibility check. It does not assign `PASS` to any `AC-` criterion.

The official sources reviewed were:

- [Chat Completion API reference](https://docs.z.ai/api-reference/llm/chat-completion)
- [GLM-5.1 guide](https://docs.z.ai/guides/llm/glm-5.1)
- [Deep Thinking guide](https://docs.z.ai/guides/capabilities/thinking)
- [Streaming Messages guide](https://docs.z.ai/guides/capabilities/streaming)

If these sources disagree, change materially, disappear, or cease to list `glm-5.1`, the provider contract is unverified until this record, affected requirements, threat `TM-020`, and contract fixtures are reviewed together.

At the review date, the general API reference also lists `glm-5.2` and a `reasoning_effort` field restricted to that model. That listing does not change this product decision or authorize a model upgrade. The MVP sends the explicit `glm-5.1` identifier and never relies on the provider's default model. Moving to `glm-5.2` requires a separate compatibility, cost, behavior, privacy, threat, specification, ADR, and acceptance-fixture change.

## 2. Verified provider facts

| Item                   | Verified value for this baseline                                                                                                                     |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| HTTPS origin           | `https://api.z.ai`                                                                                                                                   |
| Method and path        | `POST /api/paas/v4/chat/completions`                                                                                                                 |
| Authentication         | `Authorization: Bearer <API key>`                                                                                                                    |
| Content type           | `application/json`                                                                                                                                   |
| Model identifier       | `glm-5.1`                                                                                                                                            |
| Model selection        | Explicit on every request; the mutable provider default is not used                                                                                  |
| Streaming request      | `stream: true`                                                                                                                                       |
| Stream form            | Server-Sent Events; a normal stream ends with `data: [DONE]`                                                                                         |
| Visible streamed text  | `choices[0].delta.content` when present                                                                                                              |
| Thinking switch        | `thinking.type` accepts `enabled` or `disabled`                                                                                                      |
| Prior-thinking policy  | `thinking.clear_thinking` defaults to `true`; `false` enables preserved-thinking behavior and requires complete unmodified prior `reasoning_content` |
| Sampling default       | `temperature` defaults to `1.0` for `glm-5.1`; the MVP omits the field                                                                               |
| Reasoning-effort field | The reviewed reference limits `reasoning_effort` to `glm-5.2`; it is excluded from the `glm-5.1` MVP contract                                        |
| Output limit field     | `max_tokens`, documented range `1` through `131072` for `glm-5.1`                                                                                    |

The provider may return `reasoning_content`. Its availability does not authorize the application to display, persist, log, export, or replay it.

## 3. Fixed MVP request policy

This section restates the externally constrained portions of `FR-007`, `FR-013`, `SEC-006`, `SEC-009`, and `SEC-010`; those stable IDs remain the owners of normative behavior.

Rust owns the request and the endpoint. React, imported content, persisted content, and model output cannot choose the origin, path, authorization header, proxy, model, or arbitrary request fields.

The MVP request includes these application-owned values:

```json
{
  "model": "glm-5.1",
  "messages": [{ "role": "user", "content": "Visible conversation content" }],
  "stream": true,
  "thinking": { "type": "enabled" },
  "max_tokens": 8192
}
```

Only the relevant, supported visible `user` and `assistant` messages are sent. The exact top-level key set is `model`, `messages`, `stream`, `thinking`, and `max_tokens`; the exact `thinking` key set is `type`. `thinking.clear_thinking` is omitted, relying on its documented `true` default. Temperature is omitted, relying on the documented `glm-5.1` default. Tool definitions, tool messages, system messages, attachments, files, device data, unrelated conversations, prior `reasoning_content`, `reasoning_effort`, `enable_thinking`, `stream_options`, and caller-selected provider fields are outside MVP v1.

## 4. Honest three-profile mapping

| UI profile | `thinking.type` | `max_tokens` | Contract meaning                                      |
| ---------- | --------------- | -----------: | ----------------------------------------------------- |
| Fast       | `disabled`      |        4,096 | Thinking off with the shortest application output cap |
| Standard   | `enabled`       |        8,192 | Thinking on with the default application output cap   |
| Deep       | `enabled`       |       16,384 | Thinking on with the longest application output cap   |

The provider documentation verifies the thinking switch and the permitted `max_tokens` range. It does not define Fast, Standard, and Deep as three reasoning-effort levels. The names, token caps, default selection, and guidance are application policy. Standard and Deep have the same provider thinking value.

The UI must not claim that Deep invokes a provider `max` effort, that Standard invokes a provider `high` effort, or that a larger output cap guarantees more reasoning, higher quality, a larger input context, or a particular cost.

## 5. Stream acceptance policy

The parser accepts only the supported chat-completion stream shape needed for visible assistant text and terminal status. It must:

- process legal UTF-8 and SSE transport chunk boundaries incrementally;
- accept bounded `choices[0].delta.content` text in order;
- discard bounded `reasoning_content` without emitting or storing it;
- treat tool-call output and unsupported event shapes as unsupported rather than executing them;
- recognize `[DONE]` only as a terminal sentinel in a valid SSE data record;
- fail closed on the first malformed or unsupported event;
- bound headers, lines, events, deltas, total content, idle time, and overall time;
- map provider failures and finish reasons to stable safe application states without exposing raw response bodies.

The documented finish-reason mapping is exact: `stop` and `length` are supported terminal completions; `sensitive` is a non-retryable content rejection; `model_context_window_exceeded` is a non-retryable context-limit error; and `network_error` is provider-unavailable and may be retried only before any visible delta under the bounded retry policy. An unknown finish reason is a malformed stream. No finish reason authorizes tool execution or raw provider text in the UI.

## 6. Re-verification triggers

Re-verify and version this record before changing any of the following:

- model identifier, provider origin, API path, authentication, or SDK/HTTP client behavior;
- request or response fields, roles, thinking policy, output caps, finish handling, or streaming parser;
- retry, redirect, timeout, proxy, TLS, or certificate policy;
- support for tools, preserved thinking, attachments, multimodal input, web search, or another provider.

A re-verification handoff records the review date, official source links, observed differences, affected requirement and threat IDs, updated fixtures, and retained test evidence. Documentation review alone remains specification input, not release evidence.
