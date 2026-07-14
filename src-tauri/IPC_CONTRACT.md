# Aster desktop IPC contract

This document defines the renderer-to-Rust contract for Aster MVP v2. Tauri
command argument keys, returned objects, and event payloads use `camelCase`.
Provider and model identifiers are case-sensitive.

The renderer UTF-8 JSON-encodes every application-command argument object into
a raw `Uint8Array`. An argument-free command sends `{}`. Rust rejects Tauri's
pre-deserialized JSON form, applies a 320-KiB byte ceiling before parsing, and
then validates JSON depth and the exact allowed keys before command-specific
deserialization. Every argument structure rejects unknown fields.

## Enumerations and shared records

`ProviderId` is one of `zai`, `deepseek`, `alibaba-us`, `google`, or `nvidia`.
`ResponseProfile` is `fast`, `standard`, or `deep`. `MessageStatus` is
`complete`, `cancelled`, or `error`. `FinishReason` is `stop`, `outputLimit`,
or `unknown`.

```ts
type TokenUsage = {
  inputTokens: number | null;
  cachedInputTokens: number | null;
  outputTokens: number | null;
  totalTokens: number | null;
};

type ConversationSummary = {
  id: string;
  title: string;
  providerId: ProviderId;
  modelId: string;
  responseProfile: ResponseProfile;
  createdAt: string;
  updatedAt: string;
  messageCount: number;
};

type Message = {
  id: string;
  conversationId: string;
  role: "user" | "assistant";
  content: string;
  createdAt: string;
  status: MessageStatus;
  finishReason?: FinishReason;
  usage?: TokenUsage;
};

type Conversation = ConversationSummary & { messages: Message[] };
```

Every token value is a non-negative JavaScript-safe integer. When all four
usage fields are known, `totalTokens` equals the disjoint sum of input, cached
input, and output tokens. Unknown or inconsistent provider usage remains
partial; it is never estimated or presented as exact.

A newly completed assistant message carries the adapter-authenticated `stop` or
`outputLimit` finish reason. A completed migrated/imported assistant message
without authoritative terminal evidence carries `unknown`. User, cancelled,
and error messages omit `finishReason`. `outputLimit` preserves partial content
and valid usage and drives an adjacent incomplete-response notice in React.

## Commands

| Command                           | Arguments                                                                | Result                                          |
| --------------------------------- | ------------------------------------------------------------------------ | ----------------------------------------------- |
| `app_status`                      | `{}`                                                                     | `{ mode, version, online, databaseReady }`      |
| `model_catalog`                   | `{}`                                                                     | `ModelCatalog`                                  |
| `provider_statuses`               | `{}`                                                                     | `ProviderStatus[]`                              |
| `acknowledge_external_processing` | `{ providerId }`                                                         | `null`                                          |
| `prompt_store_api_key`            | `{ providerId }`                                                         | `{ providerId, configured, source, cancelled }` |
| `delete_api_key`                  | `{ providerId }`                                                         | `{ providerId, configured, source }`            |
| `list_conversations`              | `{}`                                                                     | `ConversationSummary[]`, recent first           |
| `get_conversation`                | `{ conversationId }`                                                     | `Conversation`                                  |
| `create_conversation`             | `{ title?, providerId?, modelId? }`                                      | `Conversation`                                  |
| `update_conversation_selection`   | `{ conversationId, providerId, modelId }`                                | `Conversation`                                  |
| `rename_conversation`             | `{ conversationId, title }`                                              | `ConversationSummary`                           |
| `delete_conversation`             | `{ conversationId }`                                                     | `null`                                          |
| `send_message`                    | `{ conversationId, content, responseProfile, regenerateFromMessageId? }` | `{ requestId }`                                 |
| `cancel_generation`               | `{ requestId }`                                                          | `null`                                          |
| `usage_summary`                   | `{ providerId, modelId? }`                                               | `UsageSummary`                                  |
| `set_usage_budget`                | `{ providerId, tokenBudget }`                                            | provider-level `UsageSummary`                   |
| `deepseek_balance_status`         | `{}`                                                                     | `DeepSeekBalanceStatus`                         |
| `refresh_deepseek_balance`        | `{}`                                                                     | `DeepSeekBalanceStatus`                         |
| `open_provider_account`           | `{ providerId, action }`                                                 | `null`                                          |
| `export_conversation`             | `{ conversationId }`                                                     | `{ cancelled, fileName? }`                      |
| `import_conversations`            | `{}`                                                                     | `ConversationSummary[]`; empty when cancelled   |
| `open_external_url`               | `{ url }`                                                                | `null`                                          |

`app_status.online` remains `false` because Aster does not perform a global
connectivity probe. Use the per-provider `reachability` value described below;
it changes only after an actual provider request.

`create_conversation` defaults to `zai` and `glm-5.1`. Supplying one of
`providerId` or `modelId` without the other is invalid. The pair may be changed
only while the conversation has no messages. The Rust state coordinator and a
SQLite trigger both enforce this lock, including races with generation.

`send_message` derives provider and model from the persisted conversation; it
does not accept them from the renderer. A regeneration ID targeting a user
message edits that message and atomically removes descendants. An ID targeting
the latest assistant message removes that answer and reuses the preceding user
prompt. Aster performs one provider network attempt and never automatically
retries chat requests.

## Catalog and provider state

`ModelCatalog` has this shape:

```ts
type ModelCatalog = {
  version: 2;
  defaultSelection: { providerId: "zai"; modelId: "glm-5.1" };
  providers: Array<{
    id: ProviderId;
    displayName: string;
    regionLabel: string | null;
    noticeVersion: number;
    processingNotice: string;
    accountActions: Array<{
      action: "usage" | "billing" | "addCredits" | "spend" | "deployment";
      label: string;
      description: string;
    }>;
    models: Array<{
      id: string;
      displayName: string;
      delivery: "official-api" | "hosted-prototype";
      profiles: Array<{
        id: ResponseProfile;
        label: string;
        description: string;
        enabled: boolean;
        disabledReason: string | null;
      }>;
    }>;
  }>;
};
```

The registry is closed and contains exactly these 17 provider/model pairs:

- Z.AI: `glm-4.7`, `glm-5`, `glm-5.1`, `glm-5.2`.
- DeepSeek: `deepseek-v4-flash`, `deepseek-v4-pro`.
- Alibaba Cloud US: `qwen3.5-plus`, `qwen3.5-flash`, `qwen3.6-plus`,
  `qwen3.6-flash`, `qwen3.7-plus`, `qwen3.7-max`.
- Google Gemini: `gemini-2.5-flash`, `gemini-2.5-flash-lite`,
  `gemini-2.5-pro`.
- NVIDIA: `nvidia/nemotron-3-super-120b-a12b`,
  `nvidia/nemotron-3-ultra-550b-a55b`.

NVIDIA entries are explicitly labelled `hosted-prototype`; the other entries
are labelled `official-api`. The catalog is the only source from which the UI
may present model and account-action choices.

```ts
type ProviderStatus = {
  providerId: ProviderId;
  configured: boolean;
  reachability: "unknown" | "reachable" | "unreachable";
  noticeVersion: number;
  noticeAcknowledged: boolean;
};
```

The external-processing acknowledgement is provider- and notice-version
scoped. It is required before chat data is sent to that provider. Reachability
begins as `unknown` and changes only from an actual provider request outcome.

## Usage and advisory budgets

One usage observation is created immediately before each provider chat network
attempt. A completed provider usage record fills that observation exactly once;
cancelled, failed, missing, malformed, and overflowed observations remain
partial. Deleting a conversation does not delete its usage observations.

```ts
type UsageSummary = {
  providerId: ProviderId;
  modelId: string | null;
  windowStart: string;
  windowEnd: string;
  observedAt: string;
  usage: TokenUsage;
  completeObservations: number;
  partialObservations: number;
  coverage: "empty" | "complete" | "partial";
  budget: null | {
    tokenBudget: number;
    knownUsedTokens: number | null;
    remainingTokens: number;
    remainingPercentage: number;
    state: "normal" | "low" | "exhausted";
  };
};
```

The summary window is the immediately preceding seven days. `modelId` filters
usage but the budget remains provider-scoped. `tokenBudget` accepts either a
positive JavaScript-safe integer or `null` to remove the budget. Budgets are
local advisory values only: they do not represent provider credit, do not
change provider plans, and never block a chat attempt. `low` means at most ten
percent remains by exact integer comparison; `exhausted` means zero known
tokens remain. If the aggregate exceeds the JavaScript-safe range, the budget
remains exhausted and `knownUsedTokens` is `null` rather than an invented exact
number.

DeepSeek balance is separate from local token usage and is fetched only by the
explicit `refresh_deepseek_balance` command:

```ts
type DeepSeekBalanceStatus = {
  status: "notChecked" | "current" | "stale" | "error";
  observedAt: string | null;
  isAvailable: boolean | null;
  balanceInfos: Array<{
    currency: "CNY" | "USD";
    totalBalance: string;
    grantedBalance: string;
    toppedUpBalance: string;
  }>;
  error: null | { code: string; message: string; retryable: boolean };
};
```

Balance amounts remain exact decimal strings. The latest result is memory-only;
it is not persisted, refreshed automatically, or polled in the background.
Every explicit refresh receives a Rust-owned credential generation and a
monotonically latest operation authority. Credential replacement or deletion
uses the same balance synchronization to clear the cached result, invalidate
every in-flight authority, and reset DeepSeek reachability. A current
completion commits its balance and reachability outcome together while holding
that synchronization; a stale completion updates neither. A credential-load
failure clears any prior balance before returning its safe error.

## Provider account navigation

`open_provider_account` accepts only actions advertised for that provider in
the catalog. Rust maps the pair to a compiled, fixed HTTPS URL and opens it in
the operating system's default browser. The renderer cannot supply or modify
that URL. Aster does not top up credit, change a plan, submit payment data, or
embed a provider account page.

`open_external_url` is a separate safe-link command for rendered message links.
It accepts an absolute HTTPS URL of at most 2,048 bytes and rejects credentials,
IP literals, local or single-label hosts, whitespace, control characters, and
every non-HTTPS scheme.

## Native conversation document

Export always writes version 2. Import accepts the strict version 1 format from
MVP v1 and the version 2 format below. Both input and pre-write output are
limited to 32 MiB and eight JSON container levels.

```json
{
  "format": "aster-conversation",
  "version": 2,
  "exportedAt": "RFC 3339 timestamp",
  "conversations": [
    {
      "title": "Conversation title",
      "provider": "google",
      "model": "gemini-2.5-pro",
      "responseProfile": "standard",
      "createdAt": "RFC 3339 timestamp",
      "updatedAt": "RFC 3339 timestamp",
      "messages": [
        {
          "role": "assistant",
          "content": "Response text",
          "createdAt": "RFC 3339 timestamp",
          "status": "complete",
          "finishReason": "stop",
          "usage": {
            "inputTokens": 100,
            "cachedInputTokens": 0,
            "outputTokens": 50,
            "totalTokens": 150
          }
        }
      ]
    }
  ]
}
```

Version 1 conversations are mapped to `zai` and `glm-5.1`; legacy
`reasoningMode` becomes `responseProfile`. A legacy `tokenUsage` value is kept
only as a message's `totalTokens`; it is not copied into the v2 usage ledger or
used to seed a budget.

Documents contain no local conversation or message IDs. Rust generates new IDs
during one atomic import transaction. Unknown fields, unsupported provider/model
pairs, invalid enums or finish-reason/status/role combinations, duplicate or
impossible usage totals, and timestamps where `updatedAt` precedes `createdAt`
are rejected before mutation. Version 1 complete assistant messages map to
`finishReason: "unknown"`; absence never infers a normal stop. Import permits
1-100 conversations and at most 10,000 messages in total. Titles are trimmed
and limited to 80 characters. Message content is limited to 2 MiB.

## Streaming

Rust emits `chat-stream` only to the `main` window:

```ts
type StreamEvent = {
  requestId: string;
  conversationId: string;
  providerId: ProviderId;
  modelId: string;
  sequence: number;
  kind: "started" | "delta" | "completed" | "cancelled" | "error";
  delta?: string;
  message?: Message;
  error?: string;
  errorCode?: string;
  retryable?: boolean;
};
```

The renderer subscribes before sending. Sequence begins at zero for `started`
and increases by exactly one for each delta and the terminal event. The
renderer rejects duplicates, gaps, stale events, and events whose persisted
provider/model pair does not match the active request. At most one generation
may be active per conversation. Cancellation propagates to the Rust HTTP
request. A terminal claim is atomic, so a cancelled request cannot later emit
completion. Hidden provider reasoning and thought-signature content are never
emitted or persisted. A completed event's message carries `stop` or
`outputLimit`; the latter retains partial visible content and validated usage.
Rust reserves capacity for the terminal event before accepting a delta. A
response may contain at most 65,536 ordered events in total, including
`started`, every delta, and its one terminal event. Pre-command buffering and
post-command processing apply the same ceiling. Event 65,537 fails closed
before its delta is appended or a successful completion is persisted, and
reconciliation cannot revive that rejected completion. Unknown, early,
duplicate, or out-of-order terminal or usage claims fail closed.

## Security boundary

The React webview has no filesystem, shell, process, HTTP, credential, dialog,
updater, or window-creation capability. It receives only the explicitly allowed
commands in `capabilities/main.json`. Provider networking, SSE parsing,
cancellation, policy validation, SQLite, import/export, account URL mapping,
and secret handling remain Rust-owned.

Each provider API key is captured by a provider-labelled native Windows prompt,
validated as 8-255 printable ASCII bytes without whitespace, moved through
zeroizing memory, and stored under a separate Windows Credential Manager target.
The existing Z.AI target remains `zai-api-key`. No credential may enter an IPC
argument, result, event, SQLite, log, exported document, or frontend state.

Provider HTTP endpoints are compiled constants, redirects are disabled, normal
platform certificate verification is used, timeouts and response limits are
explicit, and chat requests are not automatically retried. Provider bodies and
authorization data never appear in public errors. Command errors serialize as
`{ code, message, retryable }` with bounded English messages.
