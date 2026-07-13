import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type {
  AppStatus,
  AssistantAdapter,
  ChatMessage,
  ChatStreamEvent,
  Conversation,
  ConversationSummary,
  CredentialPromptResult,
  CredentialStatus,
  ExportResult,
  MessageStatus,
  ReasoningMode,
  SendMessageInput,
  SendMessageResult,
  StreamUnsubscribe,
} from "../types/chat";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

const isoNow = () => new Date().toISOString();

const MAX_IPC_BODY_BYTES = 320 * 1_024;

type NoArguments = Readonly<Record<string, never>>;

type DesktopCommandArguments = {
  app_status: NoArguments;
  credential_status: NoArguments;
  prompt_store_api_key: NoArguments;
  delete_api_key: NoArguments;
  list_conversations: NoArguments;
  get_conversation: Readonly<{ conversationId: string }>;
  create_conversation: Readonly<{ title?: string }>;
  rename_conversation: Readonly<{ conversationId: string; title: string }>;
  delete_conversation: Readonly<{ conversationId: string }>;
  send_message: Readonly<{
    conversationId: string;
    content: string;
    reasoningMode: ReasoningMode;
    regenerateFromMessageId?: string;
  }>;
  cancel_generation: Readonly<{ requestId: string }>;
  acknowledge_external_processing: NoArguments;
  open_external_url: Readonly<{ url: string }>;
  export_conversation: Readonly<{ conversationId: string }>;
  import_conversations: NoArguments;
};

type DesktopCommand = keyof DesktopCommandArguments;

async function invokeDesktop<Command extends DesktopCommand>(
  command: Command,
  argumentsObject: DesktopCommandArguments[Command],
): Promise<unknown> {
  const payload = new TextEncoder().encode(JSON.stringify(argumentsObject));
  if (payload.byteLength > MAX_IPC_BODY_BYTES) {
    throw new Error("Command arguments exceed the 320 KiB limit.");
  }
  return invoke<unknown>(command, payload);
}

const id = (prefix: string) => {
  const suffix = globalThis.crypto.randomUUID();
  return `${prefix}-${suffix}`;
};

const asRecord = (value: unknown): Record<string, unknown> =>
  value && typeof value === "object" ? (value as Record<string, unknown>) : {};

const text = (value: unknown, fallback = "") => (typeof value === "string" ? value : fallback);

function requiredText(value: unknown, label: string, maximum = 128) {
  if (typeof value !== "string" || !value.trim() || value.length > maximum) {
    throw new Error(`Received an invalid ${label}.`);
  }
  return value;
}

function requiredDate(value: unknown) {
  const candidate = requiredText(value, "timestamp", 64);
  if (Number.isNaN(Date.parse(candidate))) throw new Error("Received an invalid timestamp.");
  return candidate;
}

const reasoningMode = (value: unknown): ReasoningMode =>
  value === "fast" || value === "deep" || value === "standard" ? value : "standard";

function persistedMessageStatus(value: unknown): MessageStatus {
  if (value !== "complete" && value !== "cancelled" && value !== "error") {
    throw new Error("Received an invalid message status.");
  }
  return value;
}

function normalizeMessage(value: unknown, conversationId: string): ChatMessage {
  const raw = asRecord(value);
  const messageConversationId = requiredText(raw.conversationId, "message conversation ID");
  if (messageConversationId !== conversationId) {
    throw new Error("Received a message bound to the wrong conversation.");
  }
  if (raw.role !== "user" && raw.role !== "assistant") {
    throw new Error("Received an invalid message role.");
  }
  if (
    typeof raw.content !== "string" ||
    new TextEncoder().encode(raw.content).byteLength > 2 * 1_024 * 1_024
  ) {
    throw new Error("Received invalid message content.");
  }
  return {
    id: requiredText(raw.id, "message ID"),
    conversationId: messageConversationId,
    role: raw.role,
    content: raw.content,
    createdAt: requiredDate(raw.createdAt),
    status: persistedMessageStatus(raw.status),
  };
}

function normalizeSummary(value: unknown): ConversationSummary {
  const raw = asRecord(value);
  const mode = raw.reasoningMode;
  if (mode !== "fast" && mode !== "standard" && mode !== "deep") {
    throw new Error("Received an invalid reasoning mode.");
  }
  const messageCount = raw.messageCount;
  if (!Number.isSafeInteger(messageCount) || Number(messageCount) < 0) {
    throw new Error("Received an invalid message count.");
  }
  if (raw.model !== "glm-5.1") throw new Error("Received an unsupported model.");
  return {
    id: requiredText(raw.id, "conversation ID"),
    title: requiredText(raw.title, "conversation title", 80),
    model: raw.model,
    createdAt: requiredDate(raw.createdAt),
    updatedAt: requiredDate(raw.updatedAt),
    messageCount: Number(messageCount),
    reasoningMode: mode,
  };
}

function normalizeConversation(value: unknown): Conversation {
  const raw = asRecord(value);
  const summary = normalizeSummary(raw);
  if (!Array.isArray(raw.messages)) throw new Error("Received an invalid message collection.");
  const messages = raw.messages.map((message) => normalizeMessage(message, summary.id));
  return { ...summary, messages };
}

function normalizeStreamEvent(value: unknown): ChatStreamEvent {
  const raw = asRecord(value);
  const requestId = requiredText(raw.requestId, "stream request ID");
  const conversationId = requiredText(raw.conversationId, "stream conversation ID");
  if (!Number.isSafeInteger(raw.sequence) || Number(raw.sequence) < 0) {
    throw new Error("Received an invalid stream sequence.");
  }
  const sequence = Number(raw.sequence);
  const kind = raw.kind;
  if (
    kind !== "started" &&
    kind !== "delta" &&
    kind !== "completed" &&
    kind !== "cancelled" &&
    kind !== "error"
  ) {
    throw new Error("Received an invalid stream event.");
  }
  if (kind === "started" && sequence !== 0) {
    throw new Error("Received an invalid stream start sequence.");
  }
  const baseKeys = ["requestId", "conversationId", "sequence", "kind"];
  const allowedKeys = new Set(
    kind === "delta"
      ? [...baseKeys, "delta"]
      : kind === "completed"
        ? [...baseKeys, "message"]
        : kind === "cancelled"
          ? [...baseKeys, "message"]
          : kind === "error"
            ? [...baseKeys, "message", "error", "errorCode", "retryable"]
            : baseKeys,
  );
  if (Object.keys(raw).some((key) => !allowedKeys.has(key))) {
    throw new Error("Received incompatible stream event fields.");
  }
  const message =
    raw.message === undefined ? undefined : normalizeMessage(raw.message, conversationId);
  if (
    kind === "delta" &&
    (typeof raw.delta !== "string" ||
      !raw.delta ||
      new TextEncoder().encode(raw.delta).byteLength > 64 * 1_024)
  ) {
    throw new Error("Received an invalid or oversized stream delta.");
  }
  if (kind === "completed" && (message?.role !== "assistant" || message.status !== "complete")) {
    throw new Error("Received an invalid completed stream message.");
  }
  if (
    kind === "cancelled" &&
    message &&
    (message.role !== "assistant" || message.status !== "cancelled")
  ) {
    throw new Error("Received an invalid cancelled stream message.");
  }
  if (kind === "error" && message && (message.role !== "assistant" || message.status !== "error")) {
    throw new Error("Received an invalid failed stream message.");
  }
  if (
    kind === "error" &&
    (typeof raw.error !== "string" ||
      !raw.error.trim() ||
      new TextEncoder().encode(raw.error).byteLength > 512 ||
      typeof raw.errorCode !== "string" ||
      !/^[a-z0-9_]{1,64}$/.test(raw.errorCode) ||
      typeof raw.retryable !== "boolean")
  ) {
    throw new Error("Received invalid stream error details.");
  }
  return {
    requestId,
    conversationId,
    sequence,
    kind,
    delta: typeof raw.delta === "string" ? raw.delta : undefined,
    message,
    error: typeof raw.error === "string" ? raw.error : undefined,
    errorCode: typeof raw.errorCode === "string" ? raw.errorCode : undefined,
    retryable: typeof raw.retryable === "boolean" ? raw.retryable : undefined,
  };
}

export class TauriAssistantAdapter implements AssistantAdapter {
  readonly runtime = "tauri" as const;

  async appStatus(): Promise<AppStatus> {
    const raw = asRecord(await invokeDesktop("app_status", {}));
    return {
      mode: "desktop",
      version: text(raw.version) || undefined,
      online: raw.online !== false,
      providerReachability:
        raw.providerReachability === "reachable" || raw.providerReachability === "unreachable"
          ? raw.providerReachability
          : "unknown",
      externalProcessingAcknowledged: raw.externalProcessingAcknowledged === true,
      databaseReady: (raw.databaseReady ?? raw.database_ready) !== false,
    };
  }

  async credentialStatus(): Promise<CredentialStatus> {
    const raw = asRecord(await invokeDesktop("credential_status", {}));
    return {
      configured: Boolean(raw.configured),
      source: raw.configured ? "credential-vault" : "none",
    };
  }

  async promptStoreApiKey(): Promise<CredentialPromptResult> {
    const raw = await invokeDesktop("prompt_store_api_key", {});
    const status = asRecord(raw);
    if (typeof status.configured !== "boolean" || typeof status.cancelled !== "boolean") {
      throw new Error("The native credential prompt returned an invalid status.");
    }
    return {
      configured: status.configured,
      source: status.configured ? "credential-vault" : "none",
      cancelled: status.cancelled,
    };
  }

  async deleteApiKey(): Promise<CredentialStatus> {
    await invokeDesktop("delete_api_key", {});
    return { configured: false, source: "none" };
  }

  async listConversations(): Promise<ConversationSummary[]> {
    const values = await invokeDesktop("list_conversations", {});
    if (!Array.isArray(values)) throw new Error("Received an invalid conversation list.");
    return values.map(normalizeSummary);
  }

  async getConversation(conversationId: string): Promise<Conversation> {
    return normalizeConversation(await invokeDesktop("get_conversation", { conversationId }));
  }

  async createConversation(title?: string): Promise<Conversation> {
    return normalizeConversation(await invokeDesktop("create_conversation", { title }));
  }

  async renameConversation(conversationId: string, title: string): Promise<ConversationSummary> {
    return normalizeSummary(await invokeDesktop("rename_conversation", { conversationId, title }));
  }

  async deleteConversation(conversationId: string): Promise<void> {
    await invokeDesktop("delete_conversation", { conversationId });
  }

  async sendMessage(input: SendMessageInput): Promise<SendMessageResult> {
    const raw = asRecord(
      await invokeDesktop("send_message", {
        conversationId: input.conversationId,
        content: input.content,
        reasoningMode: input.reasoningMode,
        regenerateFromMessageId: input.regenerateFromMessageId,
      }),
    );
    const requestId = text(raw.requestId ?? raw.request_id);
    if (!requestId) throw new Error("The backend did not return a generation request ID.");
    return { requestId };
  }

  async cancelGeneration(requestId: string): Promise<void> {
    await invokeDesktop("cancel_generation", { requestId });
  }

  async onChatStream(
    listener: (event: ChatStreamEvent) => void,
    onUnscopedProtocolError?: () => void,
  ): Promise<StreamUnsubscribe> {
    return listen<unknown>("chat-stream", ({ payload }) => {
      try {
        listener(normalizeStreamEvent(payload));
      } catch {
        const raw = asRecord(payload);
        if (
          typeof raw.requestId === "string" &&
          raw.requestId.length > 0 &&
          raw.requestId.length <= 128 &&
          typeof raw.conversationId === "string" &&
          raw.conversationId.length > 0 &&
          raw.conversationId.length <= 128 &&
          Number.isSafeInteger(raw.sequence) &&
          Number(raw.sequence) >= 0
        ) {
          listener({
            requestId: raw.requestId,
            conversationId: raw.conversationId,
            sequence: Number(raw.sequence),
            kind: "error",
            error: "Aster rejected an invalid response stream event.",
            errorCode: "malformed_stream_event",
            retryable: true,
          });
        } else {
          onUnscopedProtocolError?.();
        }
      }
    });
  }

  async acknowledgeExternalProcessing(): Promise<void> {
    await invokeDesktop("acknowledge_external_processing", {});
  }

  async openExternalUrl(url: string): Promise<void> {
    await invokeDesktop("open_external_url", { url });
  }

  async exportConversation(conversationId: string): Promise<string | ExportResult | undefined> {
    const result = await invokeDesktop("export_conversation", { conversationId });
    if (typeof result === "string") return result;
    const raw = asRecord(result);
    if (typeof raw.cancelled === "boolean") {
      return {
        cancelled: raw.cancelled,
        fileName: typeof raw.fileName === "string" ? raw.fileName : undefined,
      };
    }
    return undefined;
  }

  async importConversations(_serialized?: string): Promise<ConversationSummary[]> {
    const result = await invokeDesktop("import_conversations", {});
    if (!Array.isArray(result)) throw new Error("Received an invalid import result.");
    return result.map(normalizeSummary);
  }
}

type DemoListener = (event: ChatStreamEvent) => void;

const seedTime = Date.now();
const demoSeedConversations: Conversation[] = [
  {
    id: "demo-threat-model",
    title: "Threat model review",
    createdAt: new Date(seedTime - 25 * 60_000).toISOString(),
    updatedAt: new Date(seedTime - 25 * 60_000).toISOString(),
    reasoningMode: "deep",
    messages: [
      {
        id: "demo-message-1",
        conversationId: "demo-threat-model",
        role: "user",
        content: "Create a concise threat-model checklist for a desktop AI client.",
        createdAt: new Date(seedTime - 27 * 60_000).toISOString(),
        status: "complete",
      },
      {
        id: "demo-message-2",
        conversationId: "demo-threat-model",
        role: "assistant",
        content:
          "## MVP security checklist\n\n- Keep provider credentials in the operating-system vault.\n- Treat model output as untrusted text.\n- Validate every IPC payload in Rust.\n- Use an outbound-domain allowlist and TLS certificate validation.\n- Record security events without prompts or secrets.",
        createdAt: new Date(seedTime - 25 * 60_000).toISOString(),
        status: "complete",
      },
    ],
  },
  {
    id: "demo-streaming-notes",
    title: "Rust streaming notes",
    createdAt: new Date(seedTime - 86_400_000).toISOString(),
    updatedAt: new Date(seedTime - 86_400_000).toISOString(),
    reasoningMode: "standard",
    messages: [],
  },
];

function demoResponse(prompt: string, mode: ReasoningMode) {
  const lower = prompt.toLowerCase();
  if (lower.includes("code") || lower.includes("component") || lower.includes("app")) {
    return `## A secure starting point\n\nI would begin with a narrow interface boundary, explicit input validation, and a testable adapter. The **${mode}** reasoning profile is active.\n\n\`\`\`typescript\ntype SafeRequest = {\n  conversationId: string;\n  content: string;\n};\n\nexport function validateRequest(value: SafeRequest) {\n  if (!value.content.trim()) throw new Error("Message is required");\n  if (value.content.length > 32_000) throw new Error("Message is too long");\n}\n\`\`\`\n\nThis keeps secrets and network access outside the renderer while preserving a fast UI feedback loop.`;
  }
  if (lower.includes("incident") || lower.includes("security") || lower.includes("threat")) {
    return "## Initial incident triage\n\n1. Preserve evidence and record the timeline.\n2. Contain the affected boundary without destroying volatile data.\n3. Rotate exposed credentials through the approved vault.\n4. Validate recovery from a known-good state.\n\n> Avoid pasting credentials, tokens, or personal data into the conversation.";
  }
  return `I can help turn that into a clear, testable plan.\n\n- Define the smallest user-visible outcome.\n- Capture acceptance criteria before implementation.\n- Keep trust boundaries explicit.\n- Verify failure states as carefully as the happy path.\n\nThis browser preview uses an **in-memory demo adapter**. The desktop build routes requests through secure Tauri IPC.`;
}

const conversationImportKeys = new Set([
  "title",
  "model",
  "reasoningMode",
  "createdAt",
  "updatedAt",
  "messages",
]);

const messageImportKeys = new Set(["role", "content", "createdAt", "status", "tokenUsage"]);

function ensureKnownKeys(raw: Record<string, unknown>, allowed: Set<string>) {
  if (Object.keys(raw).some((key) => !allowed.has(key))) {
    throw new Error("The import contains unsupported fields.");
  }
}

function importText(value: unknown, maximum: number, label: string) {
  if (typeof value !== "string" || !value.trim() || value.length > maximum) {
    throw new Error(`The import has an invalid ${label}.`);
  }
  return value;
}

function importDate(value: unknown) {
  const candidate = importText(value, 64, "timestamp");
  if (Number.isNaN(Date.parse(candidate))) throw new Error("The import has an invalid timestamp.");
  return candidate;
}

function prepareDemoConversation(value: unknown): Conversation {
  const raw = asRecord(value);
  ensureKnownKeys(raw, conversationImportKeys);
  if (!Array.isArray(raw.messages) || raw.messages.length > 2_000) {
    throw new Error("A conversation in the import has too many messages.");
  }
  const nextId = id("conversation");
  const importedMessages = raw.messages.map((candidate) => {
    const message = asRecord(candidate);
    ensureKnownKeys(message, messageImportKeys);
    if (message.role !== "user" && message.role !== "assistant") {
      throw new Error("The import contains an unsupported message role.");
    }
    if (
      message.status !== "complete" &&
      message.status !== "cancelled" &&
      message.status !== "error"
    ) {
      throw new Error("The import contains an unsupported message status.");
    }
    return {
      id: id("message"),
      conversationId: nextId,
      role: message.role,
      content: importText(message.content, 100_000, "message content"),
      createdAt: importDate(message.createdAt),
      status: message.status,
    } satisfies ChatMessage;
  });
  const importedMode = reasoningMode(raw.reasoningMode);
  if (raw.reasoningMode !== undefined && importedMode !== raw.reasoningMode) {
    throw new Error("The import contains an unsupported reasoning mode.");
  }
  const now = isoNow();
  return {
    id: nextId,
    title: importText(raw.title, 80, "conversation title"),
    model: "glm-5.1",
    reasoningMode: importedMode,
    createdAt: now,
    updatedAt: now,
    messageCount: importedMessages.length,
    messages: importedMessages,
  };
}

function losslessChunks(value: string, size = 5) {
  const characters = Array.from(value);
  const chunks: string[] = [];
  for (let index = 0; index < characters.length; index += size) {
    chunks.push(characters.slice(index, index + size).join(""));
  }
  return chunks;
}

function isLocalOrPrivateHost(hostname: string) {
  const host = hostname.toLowerCase().replace(/^\[|\]$/g, "");
  if (
    host === "localhost" ||
    host.endsWith(".localhost") ||
    host.endsWith(".local") ||
    host === "::1" ||
    host.startsWith("fc") ||
    host.startsWith("fd") ||
    host.startsWith("fe80:")
  ) {
    return true;
  }
  const parts = host.split(".").map(Number);
  if (
    parts.length !== 4 ||
    parts.some((part) => !Number.isInteger(part) || part < 0 || part > 255)
  ) {
    return false;
  }
  const first = parts[0] ?? -1;
  const second = parts[1] ?? -1;
  return (
    first === 0 ||
    first === 10 ||
    first === 127 ||
    (first === 100 && second >= 64 && second <= 127) ||
    (first === 169 && second === 254) ||
    (first === 172 && second >= 16 && second <= 31) ||
    (first === 192 && second === 168)
  );
}

export class BrowserDemoAdapter implements AssistantAdapter {
  readonly runtime = "browser-demo" as const;
  private readonly conversations: Conversation[];
  private readonly listeners = new Set<DemoListener>();
  private readonly timers = new Map<string, number>();
  private readonly partial = new Map<
    string,
    { conversationId: string; content: string; sequence: number }
  >();

  constructor(seed: Conversation[] = demoSeedConversations) {
    this.conversations = structuredClone(seed);
  }

  appStatus(): Promise<AppStatus> {
    return Promise.resolve({
      mode: "demo",
      version: "0.1.0-preview",
      online: true,
      providerReachability: "unknown",
      externalProcessingAcknowledged: true,
      databaseReady: true,
    });
  }

  credentialStatus(): Promise<CredentialStatus> {
    return Promise.resolve({ configured: false, source: "none" });
  }

  promptStoreApiKey(): Promise<CredentialPromptResult> {
    return Promise.reject(new Error("API key setup is available only in the desktop app."));
  }

  deleteApiKey(): Promise<CredentialStatus> {
    return Promise.resolve({ configured: false, source: "none" });
  }

  listConversations(): Promise<ConversationSummary[]> {
    return Promise.resolve(
      [...this.conversations]
        .sort((a, b) => Date.parse(b.updatedAt) - Date.parse(a.updatedAt))
        .map(({ messages, ...conversation }) => ({
          ...conversation,
          messageCount: messages.length,
        })),
    );
  }

  getConversation(conversationId: string): Promise<Conversation> {
    const found = this.conversations.find((conversation) => conversation.id === conversationId);
    if (!found) throw new Error("Conversation not found.");
    return Promise.resolve(structuredClone(found));
  }

  createConversation(title = "New conversation"): Promise<Conversation> {
    const now = isoNow();
    const conversation: Conversation = {
      id: id("conversation"),
      title: title.trim().slice(0, 80) || "New conversation",
      createdAt: now,
      updatedAt: now,
      reasoningMode: "standard",
      messages: [],
    };
    this.conversations.unshift(conversation);
    return Promise.resolve(structuredClone(conversation));
  }

  renameConversation(conversationId: string, title: string): Promise<ConversationSummary> {
    const conversation = this.conversations.find((item) => item.id === conversationId);
    if (!conversation) throw new Error("Conversation not found.");
    const nextTitle = title.trim().slice(0, 80);
    if (!nextTitle) throw new Error("Conversation title cannot be empty.");
    conversation.title = nextTitle;
    conversation.updatedAt = isoNow();
    const { messages, ...summary } = conversation;
    return Promise.resolve({ ...summary, messageCount: messages.length });
  }

  deleteConversation(conversationId: string): Promise<void> {
    const index = this.conversations.findIndex((item) => item.id === conversationId);
    if (index >= 0) this.conversations.splice(index, 1);
    return Promise.resolve();
  }

  sendMessage(input: SendMessageInput): Promise<SendMessageResult> {
    const conversation = this.conversations.find((item) => item.id === input.conversationId);
    if (!conversation) throw new Error("Conversation not found.");
    const requestId = id("request");
    const now = isoNow();
    const regenerationIndex = input.regenerateFromMessageId
      ? conversation.messages.findIndex((message) => message.id === input.regenerateFromMessageId)
      : -1;
    const regenerationTarget =
      regenerationIndex >= 0 ? conversation.messages[regenerationIndex] : undefined;

    if (input.regenerateFromMessageId && !regenerationTarget) {
      throw new Error("The message to revise was not found.");
    }

    if (regenerationTarget?.role === "user") {
      conversation.messages.splice(regenerationIndex);
      conversation.messages.push({
        ...regenerationTarget,
        content: input.content,
        createdAt: now,
        status: "complete",
      });
    } else if (regenerationTarget?.role === "assistant") {
      if (regenerationIndex !== conversation.messages.length - 1) {
        throw new Error("Only the latest response can be regenerated.");
      }
      conversation.messages.splice(regenerationIndex, 1);
    } else {
      conversation.messages.push({
        id: id("message"),
        conversationId: conversation.id,
        role: "user",
        content: input.content,
        createdAt: now,
        status: "complete",
      });
    }
    conversation.updatedAt = now;
    conversation.reasoningMode = input.reasoningMode;
    if (conversation.title === "New conversation") {
      conversation.title =
        input.content.trim().replace(/\s+/g, " ").slice(0, 42) || conversation.title;
    }

    const response = demoResponse(input.content, input.reasoningMode);
    const chunks = losslessChunks(response);
    this.partial.set(requestId, { conversationId: conversation.id, content: "", sequence: 0 });
    queueMicrotask(() => {
      this.emit({ requestId, conversationId: conversation.id, sequence: 0, kind: "started" });
    });

    let index = 0;
    const step = () => {
      const progress = this.partial.get(requestId);
      if (!progress) return;
      const delta = chunks[index] ?? "";
      progress.content += delta;
      progress.sequence += 1;
      this.emit({
        requestId,
        conversationId: conversation.id,
        sequence: progress.sequence,
        kind: "delta",
        delta,
      });
      index += 1;
      if (index < chunks.length) {
        this.timers.set(requestId, window.setTimeout(step, 18));
        return;
      }
      const message: ChatMessage = {
        id: id("message"),
        conversationId: conversation.id,
        role: "assistant",
        content: progress.content,
        createdAt: isoNow(),
        status: "complete",
      };
      conversation.messages.push(message);
      conversation.updatedAt = message.createdAt;
      this.partial.delete(requestId);
      this.timers.delete(requestId);
      this.emit({
        requestId,
        conversationId: conversation.id,
        sequence: progress.sequence + 1,
        kind: "completed",
        message,
      });
    };
    this.timers.set(requestId, window.setTimeout(step, 150));
    return Promise.resolve({ requestId });
  }

  cancelGeneration(requestId: string): Promise<void> {
    const timer = this.timers.get(requestId);
    if (timer) window.clearTimeout(timer);
    const progress = this.partial.get(requestId);
    this.timers.delete(requestId);
    this.partial.delete(requestId);
    if (!progress) return Promise.resolve();
    const conversation = this.conversations.find((item) => item.id === progress.conversationId);
    if (conversation) {
      conversation.messages.push({
        id: id("message"),
        conversationId: progress.conversationId,
        role: "assistant",
        content: progress.content,
        createdAt: isoNow(),
        status: "cancelled",
      });
    }
    this.emit({
      requestId,
      conversationId: progress.conversationId,
      sequence: progress.sequence + 1,
      kind: "cancelled",
    });
    return Promise.resolve();
  }

  onChatStream(
    listener: DemoListener,
    _onUnscopedProtocolError?: () => void,
  ): Promise<StreamUnsubscribe> {
    this.listeners.add(listener);
    return Promise.resolve(() => {
      this.listeners.delete(listener);
    });
  }

  acknowledgeExternalProcessing(): Promise<void> {
    return Promise.resolve();
  }

  openExternalUrl(url: string): Promise<void> {
    let parsed: URL;
    try {
      parsed = new URL(url);
    } catch {
      return Promise.reject(new Error("Only valid HTTPS links can be opened."));
    }
    if (parsed.protocol !== "https:") {
      return Promise.reject(new Error("Only HTTPS links can be opened."));
    }
    if (parsed.username || parsed.password || isLocalOrPrivateHost(parsed.hostname)) {
      return Promise.reject(new Error("This link destination is not allowed."));
    }
    window.open(parsed.toString(), "_blank", "noopener,noreferrer");
    return Promise.resolve();
  }

  async exportConversation(conversationId: string): Promise<string> {
    const conversation = await this.getConversation(conversationId);
    return JSON.stringify(
      {
        format: "aster-conversation",
        version: 1,
        exportedAt: isoNow(),
        conversations: [
          {
            title: conversation.title,
            model: conversation.model ?? "glm-5.1",
            reasoningMode: conversation.reasoningMode ?? "standard",
            createdAt: conversation.createdAt,
            updatedAt: conversation.updatedAt,
            messages: conversation.messages.map((message) => ({
              role: message.role,
              content: message.content,
              createdAt: message.createdAt,
              status: message.status === "streaming" ? "error" : message.status,
            })),
          },
        ],
      },
      null,
      2,
    );
  }

  importConversations(serialized?: string): Promise<ConversationSummary[]> {
    if (!serialized) throw new Error("Choose an Aster JSON export to import.");
    if (serialized.length > 5_000_000) throw new Error("Import is larger than the 5 MB limit.");
    let parsed: unknown;
    try {
      parsed = JSON.parse(serialized);
    } catch {
      throw new Error("The selected file is not valid JSON.");
    }
    const raw = asRecord(parsed);
    ensureKnownKeys(raw, new Set(["format", "version", "exportedAt", "conversations"]));
    if (
      raw.format !== "aster-conversation" ||
      raw.version !== 1 ||
      typeof raw.exportedAt !== "string" ||
      !Array.isArray(raw.conversations)
    ) {
      throw new Error("This is not a supported Aster conversation export.");
    }
    if (raw.conversations.length === 0 || raw.conversations.length > 100) {
      throw new Error("The import must contain between 1 and 100 conversations.");
    }
    const prepared = raw.conversations.map(prepareDemoConversation);
    this.conversations.unshift(...prepared);
    return Promise.resolve(
      prepared.map(({ messages, ...summary }) => ({
        ...summary,
        messageCount: messages.length,
      })),
    );
  }

  private emit(event: ChatStreamEvent) {
    this.listeners.forEach((listener) => {
      listener(event);
    });
  }
}

export const assistantAdapter: AssistantAdapter = window.__TAURI_INTERNALS__
  ? new TauriAssistantAdapter()
  : new BrowserDemoAdapter();
