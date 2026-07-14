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
  DesktopAssistantAdapter,
  ExportResult,
  FinishReason,
  MessageStatus,
  ResponseProfile,
  SendMessageInput,
  SendMessageResult,
  StreamUnsubscribe,
} from "../types/chat";
import type {
  AdvisoryBudget,
  DeepSeekBalance,
  ModelId,
  ModelSelection,
  ProviderAccountAction,
  ProviderId,
  ProviderStatus,
  TokenUsage,
  UsageSummary,
} from "../types/providers";
import {
  DEMO_MODEL_CATALOG,
  DEMO_PROVIDER_STATUSES,
  asSelection,
  isActionForProvider,
  isModelForProvider,
  isProviderId,
  isResponseProfile,
  normalizeDeepSeekBalance,
  normalizeModelCatalog,
  normalizeProviderStatuses,
  normalizeTokenUsage,
  normalizeUsageSummary,
} from "./providerCatalog";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

const MAX_IPC_BODY_BYTES = 320 * 1_024;
const MAX_STREAM_DELTA_BYTES = 64 * 1_024;
const MAX_SAFE_TOKEN_COUNT = Number.MAX_SAFE_INTEGER;
const DEMO_NOW = "2026-07-13T12:00:00.000Z";
const textEncoder = new TextEncoder();

type NoArguments = Readonly<Record<string, never>>;

type DesktopCommandArguments = {
  app_status: NoArguments;
  model_catalog: NoArguments;
  provider_statuses: NoArguments;
  prompt_store_api_key: Readonly<{ providerId: ProviderId }>;
  delete_api_key: Readonly<{ providerId: ProviderId }>;
  list_conversations: NoArguments;
  get_conversation: Readonly<{ conversationId: string }>;
  create_conversation: Readonly<{
    title?: string;
    providerId?: ProviderId;
    modelId?: ModelId;
  }>;
  update_conversation_selection: Readonly<{
    conversationId: string;
    providerId: ProviderId;
    modelId: ModelId;
  }>;
  rename_conversation: Readonly<{ conversationId: string; title: string }>;
  delete_conversation: Readonly<{ conversationId: string }>;
  send_message: Readonly<{
    conversationId: string;
    content: string;
    responseProfile: ResponseProfile;
    regenerateFromMessageId?: string;
  }>;
  cancel_generation: Readonly<{ requestId: string }>;
  acknowledge_external_processing: Readonly<{ providerId: ProviderId }>;
  usage_summary: Readonly<{ providerId: ProviderId; modelId?: ModelId }>;
  set_usage_budget: Readonly<{ providerId: ProviderId; tokenBudget: number | null }>;
  deepseek_balance_status: NoArguments;
  refresh_deepseek_balance: NoArguments;
  open_provider_account: Readonly<{
    providerId: ProviderId;
    action: ProviderAccountAction;
  }>;
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

const id = (prefix: string) => `${prefix}-${globalThis.crypto.randomUUID()}`;
const isoNow = () => new Date().toISOString();

function asRecord(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

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

type PersistedMessageStatus = Exclude<MessageStatus, "streaming">;

function persistedMessageStatus(value: unknown): PersistedMessageStatus {
  if (value !== "complete" && value !== "cancelled" && value !== "error") {
    throw new Error("Received an invalid message status.");
  }
  return value;
}

function isFinishReason(value: unknown): value is FinishReason {
  return value === "stop" || value === "outputLimit" || value === "unknown";
}

function normalizePersistedFinishReason(
  role: ChatMessage["role"],
  status: PersistedMessageStatus,
  value: unknown,
  legacyUnknown = false,
): FinishReason | undefined {
  if (role === "user") {
    if (status !== "complete") throw new Error("Received an invalid user message status.");
    if (value !== undefined) throw new Error("Received a finish reason on a user message.");
    return undefined;
  }
  if (status === "complete") {
    const candidate = value ?? (legacyUnknown ? "unknown" : undefined);
    if (!isFinishReason(candidate)) {
      throw new Error("Received an invalid or missing assistant finish reason.");
    }
    return candidate;
  }
  if (value !== undefined) {
    throw new Error("Received a finish reason on a cancelled or error message.");
  }
  return undefined;
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
  const role = raw.role;
  if (
    typeof raw.content !== "string" ||
    textEncoder.encode(raw.content).byteLength > 2 * 1_024 * 1_024
  ) {
    throw new Error("Received invalid message content.");
  }
  const status = persistedMessageStatus(raw.status);
  const finishReason = normalizePersistedFinishReason(role, status, raw.finishReason);
  const message: ChatMessage = {
    id: requiredText(raw.id, "message ID"),
    conversationId: messageConversationId,
    role,
    content: raw.content,
    createdAt: requiredDate(raw.createdAt),
    status,
    ...(finishReason ? { finishReason } : {}),
  };
  if (raw.usage !== undefined) message.usage = normalizeTokenUsage(raw.usage);
  return message;
}

function normalizeSummary(value: unknown): ConversationSummary {
  const raw = asRecord(value);
  const selection = asSelection(raw.providerId, raw.modelId);
  if (!isResponseProfile(raw.responseProfile)) {
    throw new Error("Received an invalid response profile.");
  }
  if (!Number.isSafeInteger(raw.messageCount) || Number(raw.messageCount) < 0) {
    throw new Error("Received an invalid message count.");
  }
  return {
    id: requiredText(raw.id, "conversation ID"),
    title: requiredText(raw.title, "conversation title", 80),
    ...selection,
    responseProfile: raw.responseProfile,
    createdAt: requiredDate(raw.createdAt),
    updatedAt: requiredDate(raw.updatedAt),
    messageCount: Number(raw.messageCount),
  };
}

function normalizeConversation(value: unknown): Conversation {
  const raw = asRecord(value);
  const summary = normalizeSummary(raw);
  if (!Array.isArray(raw.messages)) throw new Error("Received an invalid message collection.");
  return {
    ...summary,
    messages: raw.messages.map((message) => normalizeMessage(message, summary.id)),
  };
}

function normalizeStreamEvent(value: unknown): ChatStreamEvent {
  const raw = asRecord(value);
  const requestId = requiredText(raw.requestId, "stream request ID");
  const conversationId = requiredText(raw.conversationId, "stream conversation ID");
  const selection = asSelection(raw.providerId, raw.modelId);
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
    throw new Error("Received an invalid stream kind.");
  }
  if (kind === "started" && sequence !== 0) throw new Error("Invalid stream start sequence.");
  if (
    kind === "delta" &&
    (typeof raw.delta !== "string" ||
      textEncoder.encode(raw.delta).byteLength > MAX_STREAM_DELTA_BYTES)
  ) {
    throw new Error("Received an invalid stream delta.");
  }
  if (
    (kind === "started" &&
      (raw.delta !== undefined ||
        raw.message !== undefined ||
        raw.error !== undefined ||
        raw.errorCode !== undefined ||
        raw.retryable !== undefined)) ||
    (kind === "delta" &&
      (raw.message !== undefined ||
        raw.error !== undefined ||
        raw.errorCode !== undefined ||
        raw.retryable !== undefined)) ||
    ((kind === "completed" || kind === "cancelled") &&
      (raw.delta !== undefined ||
        raw.error !== undefined ||
        raw.errorCode !== undefined ||
        raw.retryable !== undefined)) ||
    (kind === "error" && raw.delta !== undefined)
  ) {
    throw new Error("Received fields that are incompatible with the stream event kind.");
  }
  const event: ChatStreamEvent = {
    requestId,
    conversationId,
    ...selection,
    sequence,
    kind,
  };
  if (kind === "delta") event.delta = raw.delta as string;
  if (kind === "completed" && raw.message === undefined) {
    throw new Error("Received a completed stream event without its message.");
  }
  if (
    (kind === "completed" || kind === "cancelled" || kind === "error") &&
    raw.message !== undefined
  ) {
    const message = normalizeMessage(raw.message, conversationId);
    const expectedStatus =
      kind === "completed" ? "complete" : kind === "cancelled" ? "cancelled" : "error";
    if (message.role !== "assistant" || message.status !== expectedStatus) {
      throw new Error("Received a terminal stream message with an incompatible role or status.");
    }
    if (kind === "completed" && message.finishReason === "unknown") {
      throw new Error("Received an unknown finish reason for a newly completed stream.");
    }
    event.message = message;
  }
  if (kind === "error") {
    if (
      typeof raw.error !== "string" ||
      !raw.error ||
      raw.error.length > 320 ||
      typeof raw.errorCode !== "string" ||
      !/^[a-z0-9_]{1,64}$/.test(raw.errorCode) ||
      typeof raw.retryable !== "boolean"
    ) {
      throw new Error("Received an invalid stream error.");
    }
    event.error = raw.error;
    event.errorCode = raw.errorCode;
    event.retryable = raw.retryable;
  }
  return event;
}

function normalizeAppStatus(value: unknown): AppStatus {
  const raw = asRecord(value);
  return {
    mode: "desktop",
    version: typeof raw.version === "string" ? raw.version : undefined,
    online: raw.online === true,
    databaseReady: raw.databaseReady !== false,
  };
}

function normalizeCredentialStatus(
  value: unknown,
  expectedProvider: ProviderId,
  includeCancelled: boolean,
): CredentialStatus | CredentialPromptResult {
  const raw = asRecord(value);
  if (raw.providerId !== expectedProvider || typeof raw.configured !== "boolean") {
    throw new Error("Received an invalid provider credential status.");
  }
  const base: CredentialStatus = {
    providerId: expectedProvider,
    configured: raw.configured,
    source: raw.configured ? "credential-vault" : "none",
  };
  if (!includeCancelled) return base;
  if (typeof raw.cancelled !== "boolean") throw new Error("Received an invalid prompt result.");
  return { ...base, cancelled: raw.cancelled };
}

export class TauriAssistantAdapter implements DesktopAssistantAdapter {
  readonly runtime = "tauri" as const;

  async appStatus() {
    return normalizeAppStatus(await invokeDesktop("app_status", {}));
  }

  async modelCatalog() {
    return normalizeModelCatalog(await invokeDesktop("model_catalog", {}));
  }

  async providerStatuses() {
    return normalizeProviderStatuses(await invokeDesktop("provider_statuses", {}));
  }

  async promptStoreApiKey(providerId: ProviderId): Promise<CredentialPromptResult> {
    return normalizeCredentialStatus(
      await invokeDesktop("prompt_store_api_key", { providerId }),
      providerId,
      true,
    ) as CredentialPromptResult;
  }

  async deleteApiKey(providerId: ProviderId) {
    return normalizeCredentialStatus(
      await invokeDesktop("delete_api_key", { providerId }),
      providerId,
      false,
    );
  }

  async listConversations() {
    const result = await invokeDesktop("list_conversations", {});
    if (!Array.isArray(result)) throw new Error("Received an invalid conversation list.");
    return result.map(normalizeSummary);
  }

  async getConversation(conversationId: string) {
    return normalizeConversation(await invokeDesktop("get_conversation", { conversationId }));
  }

  async createConversation(title?: string, selection?: ModelSelection) {
    return normalizeConversation(
      await invokeDesktop("create_conversation", {
        ...(title === undefined ? {} : { title }),
        ...(selection ?? {}),
      }),
    );
  }

  async updateConversationSelection(
    conversationId: string,
    providerId: ProviderId,
    modelId: ModelId,
  ) {
    asSelection(providerId, modelId);
    return normalizeConversation(
      await invokeDesktop("update_conversation_selection", {
        conversationId,
        providerId,
        modelId,
      }),
    );
  }

  async renameConversation(conversationId: string, title: string) {
    return normalizeSummary(await invokeDesktop("rename_conversation", { conversationId, title }));
  }

  async deleteConversation(conversationId: string) {
    await invokeDesktop("delete_conversation", { conversationId });
  }

  async sendMessage(input: SendMessageInput): Promise<SendMessageResult> {
    const raw = asRecord(
      await invokeDesktop("send_message", {
        conversationId: input.conversationId,
        content: input.content,
        responseProfile: input.responseProfile,
        ...(input.regenerateFromMessageId
          ? { regenerateFromMessageId: input.regenerateFromMessageId }
          : {}),
      }),
    );
    const requestId = requiredText(raw.requestId, "generation request ID");
    return { requestId };
  }

  async cancelGeneration(requestId: string) {
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
          raw.requestId.length <= 128 &&
          typeof raw.conversationId === "string" &&
          raw.conversationId.length <= 128 &&
          isProviderId(raw.providerId) &&
          isModelForProvider(raw.providerId, raw.modelId) &&
          Number.isSafeInteger(raw.sequence) &&
          Number(raw.sequence) >= 0
        ) {
          listener({
            requestId: raw.requestId,
            conversationId: raw.conversationId,
            providerId: raw.providerId,
            modelId: raw.modelId,
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

  async acknowledgeExternalProcessing(providerId: ProviderId) {
    await invokeDesktop("acknowledge_external_processing", { providerId });
  }

  async usageSummary(providerId: ProviderId, modelId?: ModelId) {
    if (modelId !== undefined) asSelection(providerId, modelId);
    const normalized = normalizeUsageSummary(
      await invokeDesktop("usage_summary", {
        providerId,
        ...(modelId === undefined ? {} : { modelId }),
      }),
    );
    if (normalized.providerId !== providerId || normalized.modelId !== (modelId ?? null)) {
      throw new Error("Received usage outside the requested usage scope.");
    }
    return normalized;
  }

  async setUsageBudget(providerId: ProviderId, tokenBudget: number | null) {
    if (
      tokenBudget !== null &&
      (!Number.isSafeInteger(tokenBudget) || tokenBudget < 1 || tokenBudget > MAX_SAFE_TOKEN_COUNT)
    ) {
      throw new Error("Token budget must be a positive safe integer.");
    }
    const normalized = normalizeUsageSummary(
      await invokeDesktop("set_usage_budget", { providerId, tokenBudget }),
    );
    if (normalized.providerId !== providerId || normalized.modelId !== null) {
      throw new Error("Received usage outside the requested provider-wide budget scope.");
    }
    return normalized;
  }

  async deepSeekBalanceStatus() {
    return normalizeDeepSeekBalance(await invokeDesktop("deepseek_balance_status", {}));
  }

  async refreshDeepSeekBalance() {
    return normalizeDeepSeekBalance(await invokeDesktop("refresh_deepseek_balance", {}));
  }

  async openProviderAccount(providerId: ProviderId, action: ProviderAccountAction) {
    if (!isActionForProvider(providerId, action)) {
      throw new Error("This provider account action is not supported.");
    }
    await invokeDesktop("open_provider_account", { providerId, action });
  }

  async openExternalUrl(url: string) {
    await invokeDesktop("open_external_url", { url });
  }

  async exportConversation(conversationId: string): Promise<string | ExportResult | undefined> {
    const result = await invokeDesktop("export_conversation", { conversationId });
    if (typeof result === "string") return result;
    const raw = asRecord(result);
    if (typeof raw.cancelled !== "boolean") return undefined;
    return {
      cancelled: raw.cancelled,
      fileName: typeof raw.fileName === "string" ? raw.fileName : undefined,
    };
  }

  async importConversations(_serialized?: string) {
    const result = await invokeDesktop("import_conversations", {});
    if (!Array.isArray(result)) throw new Error("Received an invalid import result.");
    return result.map(normalizeSummary);
  }
}

type DemoListener = (event: ChatStreamEvent) => void;

const seedConversations: Conversation[] = [
  {
    id: "demo-threat-model",
    title: "Threat model review",
    providerId: "zai",
    modelId: "glm-5.1",
    responseProfile: "deep",
    createdAt: "2026-07-13T11:30:00.000Z",
    updatedAt: "2026-07-13T11:35:00.000Z",
    messageCount: 2,
    messages: [
      {
        id: "demo-message-1",
        conversationId: "demo-threat-model",
        role: "user",
        content: "Create a concise threat-model checklist for a desktop AI client.",
        createdAt: "2026-07-13T11:30:00.000Z",
        status: "complete",
      },
      {
        id: "demo-message-2",
        conversationId: "demo-threat-model",
        role: "assistant",
        content:
          "## MVP security checklist\n\n- Keep each provider credential in its own operating-system vault target.\n- Treat model output as untrusted text.\n- Validate every IPC payload in Rust.\n- Pin every provider origin and model identifier.\n- Record usage without prompts or secrets.",
        createdAt: "2026-07-13T11:35:00.000Z",
        status: "complete",
        finishReason: "stop",
        usage: {
          inputTokens: 46,
          cachedInputTokens: 8,
          outputTokens: 92,
          totalTokens: 146,
        },
      },
    ],
  },
  {
    id: "demo-streaming-notes",
    title: "Rust streaming notes",
    providerId: "deepseek",
    modelId: "deepseek-v4-pro",
    responseProfile: "standard",
    createdAt: "2026-07-12T12:00:00.000Z",
    updatedAt: "2026-07-12T12:00:00.000Z",
    messageCount: 0,
    messages: [],
  },
];

const demoUsageBase: Record<ProviderId, Omit<UsageSummary, "budget" | "modelId">> = {
  zai: {
    providerId: "zai",
    windowStart: "2026-07-06T12:00:00.000Z",
    windowEnd: DEMO_NOW,
    observedAt: DEMO_NOW,
    usage: {
      inputTokens: 24_000,
      cachedInputTokens: 5_000,
      outputTokens: 22_000,
      totalTokens: 51_000,
    },
    completeObservations: 8,
    partialObservations: 0,
    coverage: "complete",
  },
  deepseek: {
    providerId: "deepseek",
    windowStart: "2026-07-06T12:00:00.000Z",
    windowEnd: DEMO_NOW,
    observedAt: DEMO_NOW,
    usage: {
      inputTokens: 42_000,
      cachedInputTokens: null,
      outputTokens: 18_000,
      totalTokens: null,
    },
    completeObservations: 2,
    partialObservations: 1,
    coverage: "partial",
  },
  "alibaba-us": {
    providerId: "alibaba-us",
    windowStart: "2026-07-06T12:00:00.000Z",
    windowEnd: DEMO_NOW,
    observedAt: DEMO_NOW,
    usage: {
      inputTokens: 8_400,
      cachedInputTokens: 600,
      outputTokens: 4_200,
      totalTokens: 13_200,
    },
    completeObservations: 3,
    partialObservations: 1,
    coverage: "partial",
  },
  google: {
    providerId: "google",
    windowStart: "2026-07-06T12:00:00.000Z",
    windowEnd: DEMO_NOW,
    observedAt: DEMO_NOW,
    usage: {
      inputTokens: null,
      cachedInputTokens: null,
      outputTokens: null,
      totalTokens: null,
    },
    completeObservations: 0,
    partialObservations: 0,
    coverage: "empty",
  },
  nvidia: {
    providerId: "nvidia",
    windowStart: "2026-07-06T12:00:00.000Z",
    windowEnd: DEMO_NOW,
    observedAt: "2026-07-12T08:00:00.000Z",
    usage: {
      inputTokens: 12_000,
      cachedInputTokens: 0,
      outputTokens: 7_500,
      totalTokens: 19_500,
    },
    completeObservations: 4,
    partialObservations: 0,
    coverage: "complete",
  },
};

export function deriveDemoAdvisoryBudget(
  tokenBudget: number,
  knownUsedTokens: number,
): AdvisoryBudget {
  const remainingTokens = Math.max(tokenBudget - knownUsedTokens, 0);
  return {
    tokenBudget,
    knownUsedTokens,
    remainingTokens,
    remainingPercentage: (remainingTokens / tokenBudget) * 100,
    state:
      remainingTokens === 0
        ? "exhausted"
        : remainingTokens <= Math.floor(tokenBudget / 10)
          ? "low"
          : "normal",
  };
}

function demoResponse(prompt: string, profile: ResponseProfile) {
  const lower = prompt.toLowerCase();
  if (lower.includes("security") || lower.includes("incident") || lower.includes("threat")) {
    return "## Initial incident triage\n\n1. Preserve evidence and record the timeline.\n2. Contain the affected boundary without destroying volatile data.\n3. Rotate exposed credentials through the approved vault.\n4. Validate recovery from a known-good state.\n\n> Avoid pasting credentials, tokens, or personal data into the conversation.";
  }
  return `I can help turn that into a clear, testable plan.\n\n- Define the smallest user-visible outcome.\n- Capture acceptance criteria before implementation.\n- Keep trust boundaries explicit.\n- Verify failure states as carefully as the happy path.\n\nThe **${profile}** response profile is active. This browser preview is synthetic and in memory only.`;
}

function losslessChunks(value: string, size = 5) {
  const characters = Array.from(value);
  const chunks: string[] = [];
  for (let index = 0; index < characters.length; index += size) {
    chunks.push(characters.slice(index, index + size).join(""));
  }
  return chunks;
}

export class BrowserDemoAdapter implements AssistantAdapter {
  readonly runtime = "browser-demo" as const;
  private readonly conversations: Conversation[];
  private readonly listeners = new Set<DemoListener>();
  private readonly timers = new Map<string, number>();
  private readonly partial = new Map<
    string,
    { conversationId: string; content: string; sequence: number; selection: ModelSelection }
  >();
  private readonly budgets = new Map<ProviderId, number>([["zai", 100_000]]);
  private deepSeekBalance: DeepSeekBalance = {
    status: "notChecked",
    observedAt: null,
    isAvailable: null,
    balanceInfos: [],
    error: null,
  };

  constructor(seed: Conversation[] = seedConversations) {
    this.conversations = structuredClone(seed);
  }

  appStatus(): Promise<AppStatus> {
    return Promise.resolve({
      mode: "demo",
      version: "0.2.0-preview",
      online: false,
      databaseReady: true,
    });
  }

  modelCatalog() {
    return Promise.resolve(structuredClone(DEMO_MODEL_CATALOG));
  }

  providerStatuses(): Promise<ProviderStatus[]> {
    return Promise.resolve(structuredClone(DEMO_PROVIDER_STATUSES));
  }

  listConversations(): Promise<ConversationSummary[]> {
    return Promise.resolve(
      [...this.conversations]
        .sort((left, right) => Date.parse(right.updatedAt) - Date.parse(left.updatedAt))
        .map(({ messages, ...conversation }) => ({
          ...conversation,
          messageCount: messages.length,
        })),
    );
  }

  getConversation(conversationId: string) {
    const conversation = this.conversations.find((candidate) => candidate.id === conversationId);
    if (!conversation) throw new Error("Conversation not found.");
    return Promise.resolve(structuredClone(conversation));
  }

  createConversation(title = "New conversation", selection?: ModelSelection) {
    const selected = selection ?? DEMO_MODEL_CATALOG.defaultSelection;
    asSelection(selected.providerId, selected.modelId);
    const now = isoNow();
    const conversation: Conversation = {
      id: id("conversation"),
      title: title.trim().slice(0, 80) || "New conversation",
      ...selected,
      responseProfile: "standard",
      createdAt: now,
      updatedAt: now,
      messageCount: 0,
      messages: [],
    };
    this.conversations.unshift(conversation);
    return Promise.resolve(structuredClone(conversation));
  }

  updateConversationSelection(conversationId: string, providerId: ProviderId, modelId: ModelId) {
    const selection = asSelection(providerId, modelId);
    const conversation = this.conversations.find((candidate) => candidate.id === conversationId);
    if (!conversation) throw new Error("Conversation not found.");
    if (conversation.messages.length > 0) {
      throw Object.assign(new Error("Start a new chat to use a different provider or model."), {
        code: "conversation_model_locked",
        retryable: false,
      });
    }
    Object.assign(conversation, selection, { updatedAt: isoNow() });
    return Promise.resolve(structuredClone(conversation));
  }

  renameConversation(conversationId: string, title: string): Promise<ConversationSummary> {
    const conversation = this.conversations.find((candidate) => candidate.id === conversationId);
    if (!conversation) throw new Error("Conversation not found.");
    const nextTitle = title.trim().slice(0, 80);
    if (!nextTitle) throw new Error("Conversation title cannot be empty.");
    conversation.title = nextTitle;
    conversation.updatedAt = isoNow();
    const { messages, ...summary } = conversation;
    return Promise.resolve({ ...summary, messageCount: messages.length });
  }

  deleteConversation(conversationId: string) {
    const index = this.conversations.findIndex((candidate) => candidate.id === conversationId);
    if (index >= 0) this.conversations.splice(index, 1);
    return Promise.resolve();
  }

  sendMessage(input: SendMessageInput): Promise<SendMessageResult> {
    const conversation = this.conversations.find(
      (candidate) => candidate.id === input.conversationId,
    );
    if (!conversation) throw new Error("Conversation not found.");
    const profile = DEMO_MODEL_CATALOG.providers
      .find((provider) => provider.id === conversation.providerId)
      ?.models.find((model) => model.id === conversation.modelId)
      ?.profiles.find((candidate) => candidate.id === input.responseProfile);
    if (!profile?.enabled) throw new Error("This response profile is not supported.");
    const requestId = id("request");
    const now = isoNow();
    const regenerationIndex = input.regenerateFromMessageId
      ? conversation.messages.findIndex((message) => message.id === input.regenerateFromMessageId)
      : -1;
    const target = regenerationIndex >= 0 ? conversation.messages[regenerationIndex] : undefined;
    if (input.regenerateFromMessageId && !target) {
      throw new Error("The message to revise was not found.");
    }
    if (target?.role === "user") {
      conversation.messages.splice(regenerationIndex);
      conversation.messages.push({ ...target, content: input.content, createdAt: now });
    } else if (target?.role === "assistant") {
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
    conversation.responseProfile = input.responseProfile;
    conversation.messageCount = conversation.messages.length;
    conversation.updatedAt = now;
    if (conversation.title === "New conversation") {
      conversation.title = input.content.trim().replace(/\s+/g, " ").slice(0, 42);
    }

    const response = demoResponse(input.content, input.responseProfile);
    const chunks = losslessChunks(response);
    const selection = {
      providerId: conversation.providerId,
      modelId: conversation.modelId,
    } as ModelSelection;
    this.partial.set(requestId, {
      conversationId: conversation.id,
      content: "",
      sequence: 0,
      selection,
    });
    queueMicrotask(() => {
      this.emit({
        requestId,
        conversationId: conversation.id,
        ...selection,
        sequence: 0,
        kind: "started",
      });
    });
    let chunkIndex = 0;
    const step = () => {
      const progress = this.partial.get(requestId);
      if (!progress) return;
      const delta = chunks[chunkIndex] ?? "";
      progress.content += delta;
      progress.sequence += 1;
      this.emit({
        requestId,
        conversationId: conversation.id,
        ...selection,
        sequence: progress.sequence,
        kind: "delta",
        delta,
      });
      chunkIndex += 1;
      if (chunkIndex < chunks.length) {
        this.timers.set(requestId, window.setTimeout(step, 18));
        return;
      }
      const usage: TokenUsage = {
        inputTokens: 18,
        cachedInputTokens: 0,
        outputTokens: 64,
        totalTokens: 82,
      };
      const message: ChatMessage = {
        id: id("message"),
        conversationId: conversation.id,
        role: "assistant",
        content: progress.content,
        createdAt: isoNow(),
        status: "complete",
        finishReason: "stop",
        usage,
      };
      conversation.messages.push(message);
      conversation.messageCount = conversation.messages.length;
      conversation.updatedAt = message.createdAt;
      this.partial.delete(requestId);
      this.timers.delete(requestId);
      this.emit({
        requestId,
        conversationId: conversation.id,
        ...selection,
        sequence: progress.sequence + 1,
        kind: "completed",
        message,
      });
    };
    this.timers.set(requestId, window.setTimeout(step, 150));
    return Promise.resolve({ requestId });
  }

  cancelGeneration(requestId: string) {
    const timer = this.timers.get(requestId);
    if (timer !== undefined) window.clearTimeout(timer);
    const progress = this.partial.get(requestId);
    this.timers.delete(requestId);
    this.partial.delete(requestId);
    if (!progress) return Promise.resolve();
    const conversation = this.conversations.find(
      (candidate) => candidate.id === progress.conversationId,
    );
    let message: ChatMessage | undefined;
    if (conversation) {
      message = {
        id: id("message"),
        conversationId: progress.conversationId,
        role: "assistant",
        content: progress.content,
        createdAt: isoNow(),
        status: "cancelled",
      };
      conversation.messages.push(message);
      conversation.messageCount = conversation.messages.length;
    }
    this.emit({
      requestId,
      conversationId: progress.conversationId,
      ...progress.selection,
      sequence: progress.sequence + 1,
      kind: "cancelled",
      message,
    });
    return Promise.resolve();
  }

  onChatStream(listener: DemoListener): Promise<StreamUnsubscribe> {
    this.listeners.add(listener);
    return Promise.resolve(() => this.listeners.delete(listener));
  }

  acknowledgeExternalProcessing(_providerId: ProviderId) {
    return Promise.resolve();
  }

  usageSummary(providerId: ProviderId, modelId?: ModelId) {
    if (modelId !== undefined) asSelection(providerId, modelId);
    const base = structuredClone(demoUsageBase[providerId]);
    const tokenBudget = this.budgets.get(providerId);
    const knownUsedTokens = base.usage.totalTokens ?? 0;
    const budget =
      tokenBudget === undefined ? null : deriveDemoAdvisoryBudget(tokenBudget, knownUsedTokens);
    return Promise.resolve({ ...base, modelId: modelId ?? null, budget });
  }

  async setUsageBudget(providerId: ProviderId, tokenBudget: number | null) {
    if (
      tokenBudget !== null &&
      (!Number.isSafeInteger(tokenBudget) || tokenBudget < 1 || tokenBudget > MAX_SAFE_TOKEN_COUNT)
    ) {
      throw new Error("Token budget must be a positive safe integer.");
    }
    if (tokenBudget === null) this.budgets.delete(providerId);
    else this.budgets.set(providerId, tokenBudget);
    return this.usageSummary(providerId);
  }

  deepSeekBalanceStatus() {
    return Promise.resolve(structuredClone(this.deepSeekBalance));
  }

  refreshDeepSeekBalance() {
    this.deepSeekBalance = {
      status: "current",
      observedAt: DEMO_NOW,
      isAvailable: true,
      balanceInfos: [
        {
          currency: "USD",
          totalBalance: "19.25",
          grantedBalance: "4.00",
          toppedUpBalance: "15.25",
        },
      ],
      error: null,
    };
    return Promise.resolve(structuredClone(this.deepSeekBalance));
  }

  openExternalUrl(_url: string): Promise<void> {
    return Promise.reject(new Error("External links are unavailable in browser demo."));
  }

  async exportConversation(conversationId: string) {
    const conversation = await this.getConversation(conversationId);
    return JSON.stringify(
      {
        format: "aster-conversation",
        version: 2,
        exportedAt: isoNow(),
        conversations: [
          {
            title: conversation.title,
            provider: conversation.providerId,
            model: conversation.modelId,
            responseProfile: conversation.responseProfile,
            createdAt: conversation.createdAt,
            updatedAt: conversation.updatedAt,
            messages: conversation.messages.map((message) => ({
              role: message.role,
              content: message.content,
              createdAt: message.createdAt,
              status: message.status === "streaming" ? "error" : message.status,
              ...(message.finishReason ? { finishReason: message.finishReason } : {}),
              ...(message.usage ? { usage: message.usage } : {}),
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
    const rootKeys = new Set(["format", "version", "exportedAt", "conversations"]);
    if (Object.keys(raw).some((key) => !rootKeys.has(key))) {
      throw new Error("Import contains unsupported fields.");
    }
    if (
      raw.format !== "aster-conversation" ||
      (raw.version !== 1 && raw.version !== 2) ||
      typeof raw.exportedAt !== "string" ||
      Number.isNaN(Date.parse(raw.exportedAt)) ||
      !Array.isArray(raw.conversations) ||
      raw.conversations.length === 0 ||
      raw.conversations.length > 100
    ) {
      throw new Error("This is not a supported Aster conversation export.");
    }
    const imported = raw.conversations.map((candidate) => {
      const item = asRecord(candidate);
      const itemKeys = new Set(
        raw.version === 1
          ? ["title", "model", "reasoningMode", "createdAt", "updatedAt", "messages"]
          : ["title", "provider", "model", "responseProfile", "createdAt", "updatedAt", "messages"],
      );
      if (Object.keys(item).some((key) => !itemKeys.has(key))) {
        throw new Error("Imported conversation contains unsupported fields.");
      }
      const selection =
        raw.version === 1
          ? ({ providerId: "zai", modelId: "glm-5.1" } as const)
          : asSelection(item.provider, item.model);
      const profileValue = raw.version === 1 ? item.reasoningMode : item.responseProfile;
      if (!isResponseProfile(profileValue)) throw new Error("Unsupported response profile.");
      if (!Array.isArray(item.messages)) throw new Error("Invalid message collection.");
      const conversationId = id("conversation");
      const messages = item.messages.map((messageCandidate) => {
        const message = asRecord(messageCandidate);
        const messageKeys = new Set(
          raw.version === 1
            ? ["role", "content", "createdAt", "status", "tokenUsage"]
            : ["role", "content", "createdAt", "status", "finishReason", "usage"],
        );
        if (Object.keys(message).some((key) => !messageKeys.has(key))) {
          throw new Error("Imported message contains unsupported fields.");
        }
        if (message.role !== "user" && message.role !== "assistant") {
          throw new Error("Unsupported message role.");
        }
        const role = message.role;
        const status = persistedMessageStatus(message.status);
        const finishReason = normalizePersistedFinishReason(
          role,
          status,
          message.finishReason,
          true,
        );
        let usage: TokenUsage | undefined;
        if (raw.version === 1) {
          if (message.tokenUsage !== undefined && message.tokenUsage !== null) {
            usage = normalizeTokenUsage({
              inputTokens: null,
              cachedInputTokens: null,
              outputTokens: null,
              totalTokens: message.tokenUsage,
            });
          }
        } else if (message.usage !== undefined) {
          usage = normalizeTokenUsage(message.usage);
        }
        return {
          id: id("message"),
          conversationId,
          role,
          content: requiredText(message.content, "message content", 100_000),
          createdAt: requiredDate(message.createdAt),
          status,
          ...(finishReason ? { finishReason } : {}),
          ...(usage ? { usage } : {}),
        } satisfies ChatMessage;
      });
      const now = isoNow();
      return {
        id: conversationId,
        title: requiredText(item.title, "conversation title", 80),
        ...selection,
        responseProfile: profileValue,
        createdAt: now,
        updatedAt: now,
        messageCount: messages.length,
        messages,
      } satisfies Conversation;
    });
    this.conversations.unshift(...imported);
    return Promise.resolve(
      imported.map(({ messages, ...summary }) => ({ ...summary, messageCount: messages.length })),
    );
  }

  private emit(event: ChatStreamEvent) {
    this.listeners.forEach((listener) => {
      listener(event);
    });
  }
}

export const assistantAdapter: TauriAssistantAdapter | BrowserDemoAdapter =
  typeof window !== "undefined" && window.__TAURI_INTERNALS__
    ? new TauriAssistantAdapter()
    : new BrowserDemoAdapter();
