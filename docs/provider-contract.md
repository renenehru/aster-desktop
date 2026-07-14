# MVP v2 Curated Provider Contract Baseline

**Status:** Normative external-interface record

**Verified:** 2026-07-14

**Catalog version:** 2

**Evidence boundary:** This document records dated official interfaces used to write requirements and contract fixtures. It does not prove that a source revision or package implements them and does not assign `PASS` to any acceptance criterion.

## 1. Catalog rule

Aster ships a closed Rust-owned catalog. An entry exists only when its exact model identifier, official provider-hosted HTTPS endpoint, authentication mechanism, streaming request/response shape, token-usage mapping, and response-profile mapping are recorded below. React, imports, user configuration, remote model-list responses, and conversation content cannot add or override an entry.

The catalog contains no unavailable state and no arbitrary model or provider entry. If a contract is no longer verifiable, the affected entry must be removed in a specification, migration, threat-model, ADR, and acceptance-fixture change before a release.

## 2. Fixed providers, origins, and models

| Provider ID  | Credential scope                                 | Exact chat endpoint                                                                                             | Exact model IDs                                                                                 |
| ------------ | ------------------------------------------------ | --------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `zai`        | Z.AI account                                     | `POST https://api.z.ai/api/paas/v4/chat/completions`                                                            | `glm-4.7`, `glm-5`, `glm-5.1`, `glm-5.2`                                                        |
| `deepseek`   | DeepSeek account                                 | `POST https://api.deepseek.com/chat/completions`                                                                | `deepseek-v4-flash`, `deepseek-v4-pro`                                                          |
| `alibaba-us` | Alibaba Cloud Model Studio US-region account/key | `POST https://dashscope-us.aliyuncs.com/compatible-mode/v1/chat/completions`                                    | `qwen3.5-plus`, `qwen3.5-flash`, `qwen3.6-plus`, `qwen3.6-flash`, `qwen3.7-plus`, `qwen3.7-max` |
| `google`     | Google AI Studio / Gemini API account            | `POST https://generativelanguage.googleapis.com/v1beta/models/{allowlistedModel}:streamGenerateContent?alt=sse` | `gemini-2.5-flash`, `gemini-2.5-flash-lite`, `gemini-2.5-pro`                                   |
| `nvidia`     | NVIDIA API Catalog account                       | `POST https://integrate.api.nvidia.com/v1/chat/completions`                                                     | `nvidia/nemotron-3-super-120b-a12b`, `nvidia/nemotron-3-ultra-550b-a55b`                        |

`{allowlistedModel}` is not a caller-supplied URL segment. Rust substitutes only one of the three exact Google model IDs after registry validation. Redirects may not escape the registered origin. Every provider uses normal platform certificate validation.

Z.AI, DeepSeek, Alibaba Cloud, and NVIDIA use a Bearer authorization header. Google uses the documented `x-goog-api-key` header. Aster stores one independent key per provider ID; Alibaba credentials are region-specific and the MVP always uses the US endpoint.

## 3. Dated official sources

### 3.1 Z.AI

- [Chat completion API](https://docs.z.ai/api-reference/llm/chat-completion)
- [GLM-5.1 guide](https://docs.z.ai/guides/llm/glm-5.1)
- [GLM-5.2 guide](https://docs.z.ai/guides/llm/glm-5.2)
- [Model migration and current identifiers](https://docs.z.ai/guides/overview/migrate-to-glm-new)
- [Pricing](https://docs.z.ai/guides/overview/pricing)

### 3.2 DeepSeek

- [API overview and base URL](https://api-docs.deepseek.com/)
- [Create chat completion](https://api-docs.deepseek.com/api/create-chat-completion)
- [Thinking mode](https://api-docs.deepseek.com/guides/thinking_mode)
- [List models](https://api-docs.deepseek.com/api/list-models/)
- [Get user balance](https://api-docs.deepseek.com/api/get-user-balance/)
- [Models and pricing](https://api-docs.deepseek.com/quick_start/pricing/)

### 3.3 Alibaba Cloud Model Studio

- [OpenAI-compatible chat API and regional endpoints](https://www.alibabacloud.com/help/en/model-studio/qwen-api-via-openai-chat-completions)
- [Deep-thinking controls and supported model identifiers](https://www.alibabacloud.com/help/en/model-studio/deep-thinking)
- [Supported models](https://www.alibabacloud.com/help/en/model-studio/models)
- [Model inference pricing and US availability](https://www.alibabacloud.com/help/en/model-studio/model-pricing)

### 3.4 Google Gemini

- [Gemini models](https://ai.google.dev/gemini-api/docs/models)
- [Gemini thinking budgets](https://ai.google.dev/gemini-api/docs/generate-content/thinking)
- [Generate Content API and usage metadata](https://ai.google.dev/api/generate-content)

### 3.5 NVIDIA

- [NVIDIA NIM LLM API reference](https://docs.api.nvidia.com/nim/reference/llm-apis)
- [Hosted Nemotron 3 Super](https://build.nvidia.com/nvidia/nemotron-3-super-120b-a12b?nim=hosted)
- [Hosted Nemotron 3 Ultra](https://build.nvidia.com/nvidia/nemotron-3-ultra-550b-a55b?nim=hosted)
- [Nemotron 3 Super request and reasoning controls](https://docs.nvidia.com/nim/large-language-models/latest/turbo/get-started-nemotron-3-super-120b-a12b.html)
- [NVIDIA thinking-budget controls](https://docs.nvidia.com/nim/large-language-models/1.15.0/thinking-budget-control.html)
- [NVIDIA NIM deployment options](https://docs.api.nvidia.com/nim/docs/run-anywhere)

The NVIDIA hosted endpoints are prototype/trial services. The catalog and UI must identify them as `Hosted prototype`; they are not Aster production deployments, self-hosted NIMs, or evidence of NVIDIA AI Enterprise entitlement.

## 4. Common minimum request policy

Rust owns every request. The selected provider/model comes from the conversation record and the registry, never from an arbitrary renderer string. Each adapter sends only:

- relevant visible `user` and `assistant` content for the pinned conversation;
- streaming enabled;
- the exact model selection required by that provider;
- one verified response-profile mapping from section 5;
- the applicable application-owned output cap; and
- the smallest documented usage-reporting option required to obtain final token metadata.

Tools, tool messages, attachments, files, device data, unrelated conversations, provider URL overrides, caller-selected headers, and preserved hidden reasoning are excluded. `reasoning_content`, thought parts, or equivalent hidden traces may be parsed only far enough to discard them safely; they are never displayed, emitted to React, replayed, logged, persisted, or exported.

## 5. Exact response-profile mappings

Output caps are Aster policy, not provider-advertised quality levels.

### 5.1 Z.AI

| Models                       | Profile  | Exact thinking field               | `max_tokens` |
| ---------------------------- | -------- | ---------------------------------- | -----------: |
| All four Z.AI catalog models | Fast     | `thinking: { "type": "disabled" }` |        4,096 |
| All four Z.AI catalog models | Standard | `thinking: { "type": "enabled" }`  |        8,192 |
| All four Z.AI catalog models | Deep     | `thinking: { "type": "enabled" }`  |       16,384 |

Standard and Deep use the same provider thinking switch. They differ only in Aster's output cap and guidance. Although `glm-5.2` documents additional controls, MVP v2 does not send them.

### 5.2 DeepSeek

| Models                       | Profile  | Exact fields                                                  | `max_tokens` |
| ---------------------------- | -------- | ------------------------------------------------------------- | -----------: |
| Both DeepSeek catalog models | Fast     | `thinking: { "type": "disabled" }`; omit `reasoning_effort`   |        4,096 |
| Both DeepSeek catalog models | Standard | `thinking: { "type": "enabled" }`, `reasoning_effort: "high"` |        8,192 |
| Both DeepSeek catalog models | Deep     | `thinking: { "type": "enabled" }`, `reasoning_effort: "max"`  |       16,384 |

Thinking-mode sampling fields documented as ignored are omitted rather than presented as controls.

### 5.3 Alibaba Cloud US

| Models                         | Profile  | Exact field              | `max_completion_tokens` |
| ------------------------------ | -------- | ------------------------ | ----------------------: |
| All six Alibaba catalog models | Fast     | `enable_thinking: false` |                   4,096 |
| All six Alibaba catalog models | Standard | `enable_thinking: true`  |                   8,192 |
| All six Alibaba catalog models | Deep     | `enable_thinking: true`  |                  16,384 |

Standard and Deep differ only in Aster's cap. The request uses `stream_options: { "include_usage": true }` to request final usage metadata. No workspace or region is caller-selectable; the fixed provider ID maps only to the US endpoint.

### 5.4 Google Gemini native API

| Model                                        | Profile  | `generationConfig.thinkingConfig.thinkingBudget` | `generationConfig.maxOutputTokens` |
| -------------------------------------------- | -------- | -----------------------------------------------: | ---------------------------------: |
| `gemini-2.5-flash` / `gemini-2.5-flash-lite` | Fast     |                                                0 |                              4,096 |
| `gemini-2.5-flash` / `gemini-2.5-flash-lite` | Standard |                                            1,024 |                              8,192 |
| `gemini-2.5-flash` / `gemini-2.5-flash-lite` | Deep     |                                            8,192 |                             16,384 |
| `gemini-2.5-pro`                             | Fast     |                                              128 |                              4,096 |
| `gemini-2.5-pro`                             | Standard |                                            4,096 |                              8,192 |
| `gemini-2.5-pro`                             | Deep     |                                           16,384 |                             16,384 |

Gemini 2.5 Pro cannot disable thinking. Its Fast profile is the minimum documented thinking budget and the UI must say so. Google uses native `contents` and `generationConfig` request structures; Aster does not translate this call through the OpenAI-compatibility surface.

### 5.5 NVIDIA hosted prototype

| Models                            | Profile  | Exact thinking fields                                                          | `max_tokens` |
| --------------------------------- | -------- | ------------------------------------------------------------------------------ | -----------: |
| Both NVIDIA hosted catalog models | Fast     | `chat_template_kwargs: { "enable_thinking": false }`; omit `reasoning_budget`  |        4,096 |
| Both NVIDIA hosted catalog models | Standard | `chat_template_kwargs: { "enable_thinking": true }`; `reasoning_budget: 4096`  |        8,192 |
| Both NVIDIA hosted catalog models | Deep     | `chat_template_kwargs: { "enable_thinking": true }`; `reasoning_budget: 16384` |       16,384 |

The hosted model pages publish `chat_template_kwargs.enable_thinking`, a top-level `reasoning_budget`, `temperature: 1`, and `top_p: 0.95`; they do not publish `reasoning_effort` for these two models. Aster fixes the documented sampling values rather than exposing them as universal response-profile controls. These mappings apply only to the fixed NVIDIA-hosted prototype endpoint. A future self-hosted or partner endpoint is a different provider contract.

## 6. Exact request, stream, finish, and usage contracts

Every adapter enforces the byte, event, accumulated-content, idle, overall, cancellation, and one-attempt bounds in [architecture.md](architecture.md). An unknown, malformed, oversized, duplicate terminal, or cross-request event fails closed. Aster never executes a tool event and never emits or persists hidden reasoning.

Normalized token categories are disjoint: `inputTokens` is non-cached input, `cachedInputTokens` is cached input, `outputTokens` includes provider-documented thought/reasoning tokens where applicable, and a complete `totalTokens` is their checked sum. Missing data stays null and makes the operation partial.

### 6.1 Z.AI

- Request body has exactly `model`, `messages`, `stream: true`, `thinking: { type }`, and `max_tokens`. No `stream_options`, reasoning-effort, sampling, tool, or extra field is sent.
- SSE must terminate with `[DONE]`. A clean EOF before `[DONE]` is partial/malformed even if content arrived.
- Exact `finish_reason` mapping: `stop` → success; `length` → output limit; `sensitive` → content rejection; `model_context_window_exceeded` → context limit; `network_error` → provider failure; `tool_calls` → unsupported capability. An unknown value fails closed.
- Usage mapping: `prompt_tokens` includes cached input; `prompt_tokens_details.cached_tokens` is cached input; `completion_tokens` is output; `total_tokens` is total. When prompt and cached are present, `inputTokens = prompt_tokens - cached_tokens`. A complete observation requires `prompt_tokens >= cached_tokens` and `total_tokens = prompt_tokens + completion_tokens`.

### 6.2 DeepSeek

- Request body has exactly `model`, `messages`, `stream: true`, `stream_options: { include_usage: true }`, `thinking: { type }`, and `max_tokens`; Standard and Deep also include `reasoning_effort`, while Fast omits it. No sampling, tool, or other field is sent.
- SSE must terminate with `[DONE]`.
- Exact `finish_reason` mapping: `stop` → success; `length` → output limit; `content_filter` → content rejection; `insufficient_system_resource` → provider failure; `tool_calls` → unsupported capability. An unknown value fails closed.
- Usage mapping: `prompt_cache_miss_tokens` is non-cached input; `prompt_cache_hit_tokens` is cached input; `completion_tokens` is output and already includes any `completion_tokens_details.reasoning_tokens` subset; `total_tokens` is total. A complete observation requires `prompt_tokens = prompt_cache_miss_tokens + prompt_cache_hit_tokens` and `total_tokens = prompt_tokens + completion_tokens`. Reasoning tokens are never added twice.

### 6.3 Alibaba Cloud US

- Request body has exactly `model`, `messages`, `stream: true`, `stream_options: { include_usage: true }`, `enable_thinking`, and `max_completion_tokens`. The deprecated `max_tokens`, tools, sampling, and extra fields are not sent. `max_completion_tokens` caps reasoning plus answer.
- The raw stream must contain a terminal choice and then `[DONE]`. With `include_usage: true`, the provider should emit a choices-empty usage chunk before `[DONE]`; missing or malformed usage leaves the coverage observation partial but does not invalidate an otherwise valid answer. Clean EOF before `[DONE]` is malformed/partial.
- Exact `finish_reason` mapping: `stop` → success; `length` → output limit; `tool_calls` → unsupported capability. An unknown value fails closed.
- Usage mapping: `prompt_tokens` includes cached input; `prompt_tokens_details.cached_tokens` is cached input; `completion_tokens` includes reasoning output while any reasoning detail is a subset; `total_tokens` is total. `inputTokens = prompt_tokens - cached_tokens`; a complete observation requires `prompt_tokens >= cached_tokens` and `total_tokens = prompt_tokens + completion_tokens`.

### 6.4 Google Gemini native API

- URL query is exactly `alt=sse`; authentication uses `x-goog-api-key`.
- Request body has exactly `contents` and `generationConfig`. `contents` contains only ordered `user`/`model` text parts derived from visible user/assistant messages. `generationConfig` contains exactly `maxOutputTokens` and `thinkingConfig: { thinkingBudget }`. No tools, system instruction, safety override, cached-content reference, or other field is sent.
- The native SSE stream terminates by clean HTTP EOF after a terminal `GenerateContentResponse`; it never requires `[DONE]`. EOF without a valid terminal response is partial/malformed.
- Exact candidate `finishReason` mapping: `STOP` → success; `MAX_TOKENS` → output limit; `SAFETY`, `RECITATION`, `LANGUAGE`, `BLOCKLIST`, `PROHIBITED_CONTENT`, `SPII`, `IMAGE_SAFETY`, `IMAGE_PROHIBITED_CONTENT`, `IMAGE_OTHER`, `NO_IMAGE`, and `IMAGE_RECITATION` → content rejection; `MALFORMED_FUNCTION_CALL`, `UNEXPECTED_TOOL_CALL`, `TOO_MANY_TOOL_CALLS`, and `MISSING_THOUGHT_SIGNATURE` → unsupported/malformed capability; `OTHER` and `MALFORMED_RESPONSE` → malformed provider response. `FINISH_REASON_UNSPECIFIED` cannot terminalize; any unknown value fails closed.
- Exact prompt `blockReason` mapping: `SAFETY`, `OTHER`, `BLOCKLIST`, `PROHIBITED_CONTENT`, and `IMAGE_SAFETY` → content rejection. `BLOCK_REASON_UNSPECIFIED` cannot terminalize and is invalid when used as a block; any unknown value fails closed.
- Thought parts and thought signatures are discarded.
- Usage mapping: `inputTokens = promptTokenCount - cachedContentTokenCount`; `cachedInputTokens = cachedContentTokenCount`; `outputTokens = candidatesTokenCount + thoughtsTokenCount`; `totalTokens = totalTokenCount`. Missing cache or thought count remains unknown rather than zero. A complete observation requires both subtractions/additions to be valid and `totalTokenCount = promptTokenCount + candidatesTokenCount + thoughtsTokenCount`.

### 6.5 NVIDIA hosted prototype

- Request body has exactly `model`, `messages`, `stream: true`, `temperature: 1`, `top_p: 0.95`, `max_tokens`, and `chat_template_kwargs: { enable_thinking }`. Standard and Deep also include the top-level `reasoning_budget`; Fast omits it. The hosted model pages do not authorize `reasoning_effort` or `stream_options` for these models, so neither is sent. Tools and all other fields are excluded.
- SSE must terminate with `[DONE]`.
- Exact `finish_reason` mapping: `stop` → success; `length` → output limit; `content_filter` → content rejection; `tool_calls` and `function_call` → unsupported capability. An unknown value fails closed.
- If the hosted service returns OpenAI-compatible `prompt_tokens`, `completion_tokens`, and `total_tokens`, they are normalized. `prompt_tokens_details.cached_tokens` is optional; when present, it is subtracted from prompt to produce non-cached input. A complete observation requires non-negative checked subtraction and `total_tokens = prompt_tokens + completion_tokens`. Absence stays partial; Aster never invents zero or requests undocumented usage fields.

### 6.6 Coverage and arithmetic rules

Every validated send that reaches provider networking creates exactly one partial coverage marker before the attempt. Valid final usage fills that marker; cancellation, failure, legal terminal without usage, missing fields, malformed usage, or early EOF leaves it partial. Pre-network validation or credential failure creates no marker. There is no automatic retry, and duplicate terminal/usage frames cannot create or replace another observation.

All provider counts must be non-negative integers and every intermediate/IPC result must pass checked arithmetic and JavaScript-safe-integer bounds. A malformed usage object does not corrupt an otherwise valid completed answer. Usage metadata is not a price, invoice, quota, or authoritative billing ledger; Aster does not estimate monetary cost.

### 6.7 Typed completion outcome

The exact per-provider finish mappings above produce one of two authoritative successful terminal outcomes: normal completion is `stop`, while the documented output-cap value is `outputLimit`. Rust preserves this typed outcome on the completed assistant message, in the terminal IPC event, SQLite, and version 2 export/import. `outputLimit` retains already validated visible content and usage and drives an explicit incomplete-response notice; it is not converted to an error or silently presented as a normal stop. A completed legacy or imported message that lacks authoritative terminal evidence is `unknown`, never an inferred `stop`. User, cancelled, and error messages carry no finish reason. Unknown, early, duplicate, or out-of-order terminal claims fail closed under the adapter contract.

## 7. DeepSeek exact balance

DeepSeek is the only MVP v2 provider with a verified exact read-only balance endpoint:

`GET https://api.deepseek.com/user/balance`

The request uses the DeepSeek Bearer credential and no body. The official response schema defines required `is_available`, `balance_infos`, currencies `CNY` or `USD`, and the `total_balance`, `granted_balance`, and `topped_up_balance` semantics. Aster adds the following security/validation policy: decode/shape validation permits at most 16 array entries; every required key must appear exactly once; each entry currency must then be `CNY` or `USD`; and duplicate required keys or duplicate currencies are invalid. Therefore a semantically accepted result has at most two entries. The body is at most 64 KiB. Each amount must be a non-negative decimal string matching `(0|[1-9][0-9]{0,17})(\.[0-9]{1,18})?`: no sign, exponent, whitespace, leading zero, comma, `NaN`, or infinity. Exact checked decimal arithmetic must satisfy `total_balance = granted_balance + topped_up_balance`. Unknown fields are ignored only after the byte, nesting, 16-entry shape cap, duplicate-key, required-field, currency, uniqueness, and decimal checks. Values cross IPC as canonical decimal strings, never binary floating point.

Balance refresh occurs only after explicit user action and is never automatically retried. The last successful result is held in Rust memory for the current session with its UTC success time and is never written to SQLite, logs, exports, or frontend browser storage. After a later refresh failure it may remain visible only as `Stale` with that last-success time and the current error. Authentication failure, rate limiting, timeout, malformed data, and offline state return safe typed results. No other provider receives a balance request.

## 8. Rust-owned provider account actions

React sends only an allowlisted provider ID and action enum. Rust selects one of these exact destinations and delegates it to the operating system's default browser:

| Provider         | Action       | Fixed official URL                                                                                    |
| ---------------- | ------------ | ----------------------------------------------------------------------------------------------------- |
| Z.AI             | `billing`    | `https://z.ai/manage-apikey/billing`                                                                  |
| Z.AI             | `addCredits` | `https://z.ai/manage-apikey/billing`                                                                  |
| DeepSeek         | `usage`      | `https://platform.deepseek.com/usage`                                                                 |
| DeepSeek         | `addCredits` | `https://platform.deepseek.com/top_up`                                                                |
| Alibaba Cloud US | `usage`      | `https://modelstudio.console.alibabacloud.com/?tab=costing-balance`                                   |
| Alibaba Cloud US | `billing`    | `https://usercenter2-intl.console.alibabacloud.com/finance/expense-report/expense-detail-by-instance` |
| Alibaba Cloud US | `addCredits` | `https://billing-cost-intl.aliyun.com/fortune/billing-account`                                        |
| Google           | `usage`      | `https://aistudio.google.com/usage`                                                                   |
| Google           | `billing`    | `https://aistudio.google.com/billing`                                                                 |
| Google           | `spend`      | `https://aistudio.google.com/spend`                                                                   |
| NVIDIA           | `deployment` | `https://build.nvidia.com/`                                                                           |

The only action values accepted over JSON IPC are `usage`, `billing`, `addCredits`, `spend`, and `deployment`. Labels are non-authoritative catalog metadata. The account command never accepts a model ID, URL, path, query, host, or model-generated label from React. Unsupported provider/action combinations fail. Aster does not log in, submit a form, buy credits, alter a plan, or embed these pages.

## 9. Re-verification triggers

This baseline and all mapped requirements, threats, ADRs, and fixtures must be reviewed before changing:

- a provider, model identifier, origin, path, authentication method, region, redirect policy, or account URL;
- a request field, response profile, output cap, usage field, finish reason, or SSE/stream shape;
- balance retrieval, billing behavior, or a provider's hosted/prototype status;
- token semantics, pricing claims, hidden-reasoning handling, tools, attachments, or system messages; or
- a provider deprecation, alias change, contract conflict, or official-source disappearance.

Documentation review does not replace exact request snapshots, controlled fake-server tests, packaged network-policy tests, or an explicitly authorized live compatibility job.
