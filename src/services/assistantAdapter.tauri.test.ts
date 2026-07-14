import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const tauriMocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriMocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: tauriMocks.listen }));

import { TauriAssistantAdapter } from "./assistantAdapter";
import { DEMO_MODEL_CATALOG, DEMO_PROVIDER_STATUSES } from "./providerCatalog";
import type { ChatStreamEvent } from "../types/chat";

const rawConversation = {
  id: "conversation-1",
  title: "Contract chat",
  providerId: "zai",
  modelId: "glm-5.1",
  responseProfile: "deep",
  createdAt: "2026-07-11T10:00:00.000Z",
  updatedAt: "2026-07-11T10:01:00.000Z",
  messageCount: 1,
  messages: [
    {
      id: "message-1",
      conversationId: "conversation-1",
      role: "user",
      content: "Hello",
      createdAt: "2026-07-11T10:00:00.000Z",
      status: "complete",
    },
  ],
};

const rawUsageSummary = {
  providerId: "zai",
  modelId: null,
  windowStart: "2026-07-06T12:00:00.000Z",
  windowEnd: "2026-07-13T12:00:00.000Z",
  observedAt: "2026-07-13T12:00:00.000Z",
  usage: {
    inputTokens: 10,
    cachedInputTokens: 2,
    outputTokens: 8,
    totalTokens: 20,
  },
  completeObservations: 1,
  partialObservations: 0,
  coverage: "complete",
  budget: null,
};

interface RawByteView {
  readonly length: number;
  readonly [index: number]: number;
}

function isRawByteView(value: unknown): value is RawByteView {
  return Object.prototype.toString.call(value) === "[object Uint8Array]";
}

function decodeRawArguments(value: unknown): unknown {
  if (!isRawByteView(value)) throw new Error("Expected a raw UTF-8 IPC body.");
  return JSON.parse(new TextDecoder().decode(Uint8Array.from(value))) as unknown;
}

function expectRawInvocation(command: string, expectedArguments: Record<string, unknown>) {
  const call = tauriMocks.invoke.mock.calls.find((candidate) => candidate[0] === command);
  expect(call).toBeDefined();
  expect(call).toHaveLength(2);
  expect(isRawByteView(call?.[1])).toBe(true);
  expect(decodeRawArguments(call?.[1])).toEqual(expectedArguments);
}

describe("TauriAssistantAdapter IPC mapping", () => {
  beforeEach(() => {
    tauriMocks.invoke.mockReset();
    tauriMocks.listen.mockReset();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("maps status, credentials, and conversation CRUD to exact command arguments", async () => {
    const adapter = new TauriAssistantAdapter();
    tauriMocks.invoke
      .mockResolvedValueOnce({
        mode: "desktop",
        version: "0.1.0",
        online: false,
        databaseReady: true,
      })
      .mockResolvedValueOnce(DEMO_MODEL_CATALOG)
      .mockResolvedValueOnce(
        DEMO_PROVIDER_STATUSES.map((status) =>
          status.providerId === "zai" ? { ...status, configured: true } : status,
        ),
      )
      .mockResolvedValueOnce({ providerId: "zai", configured: true, cancelled: false })
      .mockResolvedValueOnce({ providerId: "zai", configured: false })
      .mockResolvedValueOnce([rawConversation])
      .mockResolvedValueOnce(rawConversation)
      .mockResolvedValueOnce(rawConversation)
      .mockResolvedValueOnce({
        ...rawConversation,
        providerId: "deepseek",
        modelId: "deepseek-v4-pro",
      })
      .mockResolvedValueOnce({ ...rawConversation, title: "Renamed" })
      .mockResolvedValueOnce(null);

    await expect(adapter.appStatus()).resolves.toEqual({
      mode: "desktop",
      version: "0.1.0",
      online: false,
      databaseReady: true,
    });
    await expect(adapter.modelCatalog()).resolves.toEqual(DEMO_MODEL_CATALOG);
    await expect(adapter.providerStatuses()).resolves.toHaveLength(5);
    await expect(adapter.promptStoreApiKey("zai")).resolves.toMatchObject({
      providerId: "zai",
      configured: true,
      cancelled: false,
    });
    await expect(adapter.deleteApiKey("zai")).resolves.toEqual({
      providerId: "zai",
      configured: false,
      source: "none",
    });
    await expect(adapter.listConversations()).resolves.toHaveLength(1);
    await expect(adapter.getConversation("conversation-1")).resolves.toMatchObject({
      id: "conversation-1",
      messages: [{ role: "user" }],
    });
    await adapter.createConversation("New title");
    await adapter.updateConversationSelection("conversation-1", "deepseek", "deepseek-v4-pro");
    await adapter.renameConversation("conversation-1", "Renamed");
    await adapter.deleteConversation("conversation-1");

    expectRawInvocation("app_status", {});
    expectRawInvocation("model_catalog", {});
    expectRawInvocation("provider_statuses", {});
    expectRawInvocation("prompt_store_api_key", { providerId: "zai" });
    expectRawInvocation("delete_api_key", { providerId: "zai" });
    expectRawInvocation("create_conversation", { title: "New title" });
    expectRawInvocation("update_conversation_selection", {
      conversationId: "conversation-1",
      providerId: "deepseek",
      modelId: "deepseek-v4-pro",
    });
    expectRawInvocation("rename_conversation", {
      conversationId: "conversation-1",
      title: "Renamed",
    });
    expectRawInvocation("delete_conversation", {
      conversationId: "conversation-1",
    });
    expect(
      tauriMocks.invoke.mock.calls.every((call) => call.length === 2 && isRawByteView(call[1])),
    ).toBe(true);
  });

  it("maps send, cancel, import, and both export result shapes", async () => {
    const adapter = new TauriAssistantAdapter();
    tauriMocks.invoke
      .mockResolvedValueOnce({ requestId: "request-1" })
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce('{"format":"aster-conversation"}')
      .mockResolvedValueOnce({ cancelled: false, fileName: "chat.aster.json" })
      .mockResolvedValueOnce({ unexpected: true })
      .mockResolvedValueOnce([rawConversation]);

    await expect(
      adapter.sendMessage({
        conversationId: "conversation-1",
        content: "Hello",
        responseProfile: "standard",
        regenerateFromMessageId: "message-1",
      }),
    ).resolves.toEqual({ requestId: "request-1" });
    await adapter.cancelGeneration("request-1");
    await adapter.openExternalUrl("https://example.com/guide");
    await adapter.acknowledgeExternalProcessing("zai");
    await expect(adapter.exportConversation("conversation-1")).resolves.toBe(
      '{"format":"aster-conversation"}',
    );
    await expect(adapter.exportConversation("conversation-1")).resolves.toEqual({
      cancelled: false,
      fileName: "chat.aster.json",
    });
    await expect(adapter.exportConversation("conversation-1")).resolves.toBeUndefined();
    await expect(adapter.importConversations("renderer-content-is-ignored")).resolves.toHaveLength(
      1,
    );

    expectRawInvocation("send_message", {
      conversationId: "conversation-1",
      content: "Hello",
      responseProfile: "standard",
      regenerateFromMessageId: "message-1",
    });
    expectRawInvocation("cancel_generation", { requestId: "request-1" });
    expectRawInvocation("open_external_url", {
      url: "https://example.com/guide",
    });
    expectRawInvocation("acknowledge_external_processing", { providerId: "zai" });
    expectRawInvocation("import_conversations", {});
    expect(
      tauriMocks.invoke.mock.calls.every((call) => call.length === 2 && isRawByteView(call[1])),
    ).toBe(true);
  });

  it("rejects an oversized UTF-8 body before invoking Rust", async () => {
    const adapter = new TauriAssistantAdapter();

    await expect(
      adapter.sendMessage({
        conversationId: "conversation-1",
        content: "\u20ac".repeat(110 * 1_024),
        responseProfile: "standard",
      }),
    ).rejects.toThrow("320 KiB");
    expect(tauriMocks.invoke).not.toHaveBeenCalled();
  });

  it("rejects a send result without a request ID", async () => {
    const adapter = new TauriAssistantAdapter();
    tauriMocks.invoke.mockResolvedValueOnce({});
    await expect(
      adapter.sendMessage({
        conversationId: "conversation-1",
        content: "Hello",
        responseProfile: "fast",
      }),
    ).rejects.toThrow("invalid generation request ID");
  });

  it("rejects usage and budget responses that are crossed to another scope", async () => {
    const adapter = new TauriAssistantAdapter();
    tauriMocks.invoke
      .mockResolvedValueOnce({ ...rawUsageSummary, providerId: "deepseek" })
      .mockResolvedValueOnce({ ...rawUsageSummary, modelId: "glm-5.1" });

    await expect(adapter.usageSummary("zai")).rejects.toThrow(/requested usage scope/i);
    await expect(adapter.setUsageBudget("zai", 100_000)).rejects.toThrow(
      /provider-wide budget scope/i,
    );
  });

  it.each(["stop", "outputLimit", "unknown"] as const)(
    "preserves the typed %s finish reason on completed desktop messages",
    async (finishReason) => {
      const adapter = new TauriAssistantAdapter();
      tauriMocks.invoke.mockResolvedValueOnce({
        ...rawConversation,
        messages: [
          {
            ...rawConversation.messages[0],
            role: "assistant",
            finishReason,
          },
        ],
      });

      await expect(adapter.getConversation("conversation-1")).resolves.toMatchObject({
        messages: [{ role: "assistant", status: "complete", finishReason }],
      });
    },
  );

  it.each(["cancelled", "error"] as const)(
    "accepts an assistant %s message only when its finish reason is omitted",
    async (status) => {
      const adapter = new TauriAssistantAdapter();
      tauriMocks.invoke.mockResolvedValueOnce({
        ...rawConversation,
        messages: [{ ...rawConversation.messages[0], role: "assistant", status }],
      });

      const conversation = await adapter.getConversation("conversation-1");
      expect(conversation.messages[0]).toMatchObject({ role: "assistant", status });
      expect(conversation.messages[0]).not.toHaveProperty("finishReason");
    },
  );

  it.each([
    [
      "a completed assistant message without a finish reason",
      { ...rawConversation.messages[0], role: "assistant" },
      /finish reason/i,
    ],
    [
      "an unknown finish reason",
      { ...rawConversation.messages[0], role: "assistant", finishReason: "length" },
      /finish reason/i,
    ],
    [
      "a user message with a finish reason",
      { ...rawConversation.messages[0], finishReason: "stop" },
      /finish reason/i,
    ],
    [
      "a cancelled user message",
      { ...rawConversation.messages[0], status: "cancelled" },
      /user message status/i,
    ],
    [
      "a cancelled assistant message with a finish reason",
      {
        ...rawConversation.messages[0],
        role: "assistant",
        status: "cancelled",
        finishReason: "stop",
      },
      /finish reason/i,
    ],
    [
      "an error assistant message with a finish reason",
      {
        ...rawConversation.messages[0],
        role: "assistant",
        status: "error",
        finishReason: "unknown",
      },
      /finish reason/i,
    ],
  ])("rejects %s", async (_label, message, expectedError) => {
    const adapter = new TauriAssistantAdapter();
    tauriMocks.invoke.mockResolvedValueOnce({ ...rawConversation, messages: [message] });

    await expect(adapter.getConversation("conversation-1")).rejects.toThrow(expectedError);
  });

  it("normalizes valid stream events and ignores malformed events", async () => {
    const adapter = new TauriAssistantAdapter();
    const events: ChatStreamEvent[] = [];
    const unsubscribe = vi.fn();
    let deliver: ((event: { payload: unknown }) => void) | undefined;
    tauriMocks.listen.mockImplementation(
      (_name: string, callback: (event: { payload: unknown }) => void) => {
        deliver = callback;
        return Promise.resolve(unsubscribe);
      },
    );
    const stopListening = await adapter.onChatStream((event) => events.push(event));
    deliver?.({
      payload: {
        requestId: "request-1",
        conversationId: "conversation-1",
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 1,
        kind: "completed",
        message: {
          ...rawConversation.messages[0],
          role: "assistant",
          status: "complete",
          finishReason: "outputLimit",
        },
      },
    });
    deliver?.({
      payload: {
        requestId: "request-2",
        conversationId: "conversation-1",
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 0,
        kind: "unknown",
      },
    });

    expect(events).toHaveLength(2);
    expect(events[0]).toMatchObject({
      kind: "completed",
      requestId: "request-1",
      message: { finishReason: "outputLimit" },
    });
    expect(events[1]).toMatchObject({
      kind: "error",
      errorCode: "malformed_stream_event",
      retryable: true,
    });
    stopListening();
    expect(unsubscribe).toHaveBeenCalledOnce();
  });

  it("rejects terminal stream messages with invalid state or finish-reason combinations", async () => {
    const adapter = new TauriAssistantAdapter();
    const events: ChatStreamEvent[] = [];
    let deliver: ((event: { payload: unknown }) => void) | undefined;
    tauriMocks.listen.mockImplementation(
      (_name: string, callback: (event: { payload: unknown }) => void) => {
        deliver = callback;
        return Promise.resolve(() => undefined);
      },
    );
    await adapter.onChatStream((event) => events.push(event));

    const base = {
      requestId: "request-terminal",
      conversationId: "conversation-1",
      providerId: "zai",
      modelId: "glm-5.1",
      sequence: 1,
    };
    const assistant = { ...rawConversation.messages[0], role: "assistant" };
    for (const payload of [
      { ...base, kind: "completed", message: assistant },
      {
        ...base,
        kind: "completed",
        message: { ...assistant, finishReason: "unknown" },
      },
      {
        ...base,
        kind: "cancelled",
        message: { ...assistant, finishReason: "stop" },
      },
      {
        ...base,
        kind: "error",
        message: { ...assistant, status: "error", finishReason: "stop" },
        error: "Provider failed",
        errorCode: "provider_error",
        retryable: false,
      },
    ]) {
      deliver?.({ payload });
    }

    expect(events).toHaveLength(4);
    expect(events.every((event) => event.kind === "error")).toBe(true);
    expect(events.every((event) => event.errorCode === "malformed_stream_event")).toBe(true);
  });

  it("terminalizes oversized and kind-incompatible stream payloads safely", async () => {
    const adapter = new TauriAssistantAdapter();
    const events: ChatStreamEvent[] = [];
    let deliver: ((event: { payload: unknown }) => void) | undefined;
    tauriMocks.listen.mockImplementation(
      (_name: string, callback: (event: { payload: unknown }) => void) => {
        deliver = callback;
        return Promise.resolve(() => undefined);
      },
    );
    const unscopedError = vi.fn();
    await adapter.onChatStream((event) => events.push(event), unscopedError);

    deliver?.({
      payload: {
        requestId: "oversized",
        conversationId: "conversation-1",
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 1,
        kind: "delta",
        delta: "x".repeat(64 * 1_024 + 1),
      },
    });
    deliver?.({
      payload: {
        requestId: "oversized-utf8",
        conversationId: "conversation-1",
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 1,
        kind: "delta",
        delta: "é".repeat(32 * 1_024 + 1),
      },
    });
    deliver?.({
      payload: {
        requestId: "wrong-fields",
        conversationId: "conversation-1",
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 0,
        kind: "started",
        delta: "not allowed",
      },
    });
    deliver?.({
      payload: {
        requestId: "bad-error",
        conversationId: "conversation-1",
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 2,
        kind: "error",
        error: "Provider failed",
        errorCode: "INVALID CODE",
        retryable: "yes",
      },
    });
    deliver?.({
      payload: {
        conversationId: "conversation-1",
        sequence: "invalid",
        kind: "delta",
        delta: "cannot scope",
      },
    });

    expect(events).toHaveLength(4);
    expect(events.every((event) => event.errorCode === "malformed_stream_event")).toBe(true);
    expect(events.every((event) => event.retryable === true)).toBe(true);
    expect(unscopedError).toHaveBeenCalledOnce();
  });

  it("fails closed on malformed conversation identifiers and statuses", async () => {
    const adapter = new TauriAssistantAdapter();
    tauriMocks.invoke
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce([{ providerId: "not-real" }])
      .mockResolvedValueOnce([
        null,
        { title: 42, created_at: "bad-date", reasoning_mode: "unknown" },
      ]);

    await expect(adapter.appStatus()).resolves.toMatchObject({
      mode: "desktop",
      online: false,
      databaseReady: true,
    });
    await expect(adapter.providerStatuses()).rejects.toThrow("provider status");
    await expect(adapter.listConversations()).rejects.toThrow("provider");
  });
});
