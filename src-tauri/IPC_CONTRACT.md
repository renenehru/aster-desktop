# Aster desktop IPC contract

This file is the normative renderer-to-Rust contract for MVP v1. Tauri command
argument keys use `camelCase`. Returned objects and event payloads also use
`camelCase`. The renderer UTF-8 JSON-encodes every application-command argument
object into a raw `Uint8Array`; even an argument-free application command sends
`{}`. Every application command receives the raw Tauri request first; Rust
rejects Tauri's pre-deserialized JSON form,
checks the 320-KiB byte ceiling before parsing, and then validates depth and
allowed keys before deserializing a command-specific `deny_unknown_fields`
structure. Missing, mistyped, unknown, malformed, oversized, or excessively
nested arguments return a stable validation error.

## Commands

| Command                           | Arguments                                                              | Result                                                                                           |
| --------------------------------- | ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| `app_status`                      | none                                                                   | `{ mode, version, online, providerReachability, databaseReady, externalProcessingAcknowledged }` |
| `credential_status`               | none                                                                   | `{ configured, source }`                                                                         |
| `acknowledge_external_processing` | none                                                                   | `null`; acknowledges the visible notice for this process session only                            |
| `prompt_store_api_key`            | none                                                                   | `{ configured, source, cancelled }` after a Rust-owned native Windows credential prompt          |
| `delete_api_key`                  | none                                                                   | credential status                                                                                |
| `list_conversations`              | none                                                                   | `ConversationSummary[]` (recent first)                                                           |
| `get_conversation`                | `{ conversationId: string }`                                           | `Conversation`                                                                                   |
| `create_conversation`             | `{ title?: string }`                                                   | `Conversation`                                                                                   |
| `rename_conversation`             | `{ conversationId, title }`                                            | `ConversationSummary`                                                                            |
| `delete_conversation`             | `{ conversationId }`                                                   | `null`                                                                                           |
| `send_message`                    | `{ conversationId, content, reasoningMode, regenerateFromMessageId? }` | `{ requestId }`                                                                                  |
| `cancel_generation`               | `{ requestId }`                                                        | `null`                                                                                           |
| `export_conversation`             | `{ conversationId }`                                                   | `{ cancelled, fileName? }` after a native save dialog                                            |
| `import_conversations`            | none                                                                   | imported `ConversationSummary[]` after a native open dialog; `[]` when cancelled                 |
| `open_external_url`               | `{ url: string }`                                                      | `null` after opening the validated URL in the OS default browser                                 |

`reasoningMode` is `fast`, `standard`, or `deep`. A regeneration ID that
targets a user message edits that message and atomically removes its
descendants. A regeneration ID that targets the most recent assistant message
removes that answer and reuses the preceding persisted user prompt.

The backend uses the currently documented Z.AI `glm-5.1` model. Fast maps to
`thinking: { type: "disabled" }`; Standard and Deep map to
`thinking: { type: "enabled" }`. Deep allows a larger output budget. The
backend never emits or stores `reasoning_content`.

`providerReachability` is `unknown` until a provider attempt completes,
`reachable` after any valid provider HTTP/stream interaction, or `unreachable`
after a network/timeout failure. `online` is retained as a compatibility boolean
and is true only for `reachable`; startup never claims provider connectivity.
`send_message` returns `external_processing_notice_required` without reserving a
request, mutating SQLite, or contacting Z.AI until the session acknowledgement
command succeeds. The acknowledgement is deliberately not persisted across
application launches.

## Native conversation document

Import and export use this minimal version 1 JSON schema:

```json
{
  "format": "aster-conversation",
  "version": 1,
  "exportedAt": "RFC 3339 timestamp",
  "conversations": [
    {
      "title": "Conversation title",
      "model": "glm-5.1",
      "reasoningMode": "fast | standard | deep",
      "createdAt": "RFC 3339 timestamp",
      "updatedAt": "RFC 3339 timestamp",
      "messages": [
        {
          "role": "user | assistant",
          "content": "message text",
          "createdAt": "RFC 3339 timestamp",
          "status": "complete | cancelled | error",
          "tokenUsage": 0
        }
      ]
    }
  ]
}
```

`tokenUsage` is optional. User messages must have `complete` status. The
document omits all local conversation/message IDs and derived message counts;
Rust generates new IDs during the atomic import. Unknown fields, source IDs,
unsupported enum values, and timestamps where `updatedAt` precedes `createdAt`
are rejected before mutation.

Native document input and pre-write serialized output are each limited to 32
MiB. Import additionally permits 1–100 conversations, at most 10,000 messages,
and at most eight JSON container levels. Titles are trimmed and limited to 80
characters. Message content is limited to 2 MiB. IPC command bodies are limited
to 320 KiB, which comfortably accommodates the 32,000-unit composer limit
without exposing an unbounded renderer-to-Rust parse. The separate 256-KiB
semantic user-message ceiling still applies; highly escaped raw JSON can reach
the body ceiling first.

## Streaming

Rust emits `chat-stream` only to the `main` window:

```json
{
  "requestId": "UUID",
  "conversationId": "UUID",
  "sequence": 0,
  "kind": "started | delta | completed | cancelled | error",
  "delta": "optional incremental text",
  "message": "optional persisted assistant Message",
  "error": "optional safe English message",
  "errorCode": "optional stable machine code",
  "retryable": false
}
```

The renderer must subscribe before sending. Sequence starts at zero for
`started` and increases by exactly one for every delta and terminal event; the
renderer rejects duplicates and gaps for that request. At most one generation can be
active per conversation. `cancel_generation` is idempotent and a cancelled
request cannot later emit a completion.

Command errors serialize as `{ code, message, retryable }`. Messages are
bounded, stable English text and never include provider bodies, SQL, file
paths, conversation content, authorization headers, or credential values.

External navigation accepts absolute HTTPS URLs of at most 2,048 bytes. It
rejects credentials in the authority, IP literals, single-label/local/private
host suffixes, whitespace, controls, and every non-HTTPS scheme. The validated
URL opens through the Rust-owned Tauri opener in the OS default browser, never
inside the webview. The renderer is not granted the opener plugin's commands.

## Security boundary

The renderer has no filesystem, shell, process, HTTP, clipboard, dialog,
credential, updater, or window-creation capability. Import and export use native
dialogs owned by Rust. File paths never cross IPC, all reads and serialized
writes are bounded, and Rust never accepts a renderer-supplied path. The API key
is captured only by the Rust-owned Windows credential prompt. It never enters
the webview or an IPC argument, result, or event. Rust accepts 8 to 255 printable
ASCII bytes, rejects a 256-character result as potentially truncated, moves a
confirmed value directly through zeroizing memory, and stores it through Windows
Credential Manager. Prompt cancellation preserves the existing credential and
returns `cancelled: true`. The key is never returned, logged, placed in SQLite,
or included in an event.
