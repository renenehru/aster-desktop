import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { BrowserDemoAdapter } from "./assistantAdapter";
import type { ChatStreamEvent } from "../types/chat";

const defaultDemoResponse = [
  "I can help turn that into a clear, testable plan.",
  "",
  "- Define the smallest user-visible outcome.",
  "- Capture acceptance criteria before implementation.",
  "- Keep trust boundaries explicit.",
  "- Verify failure states as carefully as the happy path.",
  "",
  "This browser preview uses an **in-memory demo adapter**. The desktop build routes requests through secure Tauri IPC.",
].join("\n");

function validConversation(title = "Imported conversation") {
  return {
    title,
    model: "glm-5.1",
    reasoningMode: "deep",
    createdAt: "2026-07-11T12:00:00.000Z",
    updatedAt: "2026-07-11T12:00:00.000Z",
    messages: [
      {
        role: "assistant",
        content: "Imported **safely**.",
        createdAt: "2026-07-11T12:00:00.000Z",
        status: "complete",
        tokenUsage: 12,
      },
    ],
  };
}

describe("BrowserDemoAdapter", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("is visibly demo-only and never accepts an API key", async () => {
    const adapter = new BrowserDemoAdapter([]);
    await expect(adapter.appStatus()).resolves.toMatchObject({
      mode: "demo",
      version: "0.1.0-preview",
      databaseReady: true,
    });
    await expect(adapter.credentialStatus()).resolves.toEqual({
      configured: false,
      source: "none",
    });
    await expect(adapter.promptStoreApiKey()).rejects.toThrow("available only in the desktop app");
    await expect(adapter.deleteApiKey()).resolves.toEqual({ configured: false, source: "none" });
  });

  it("opens only public credential-free HTTPS destinations", async () => {
    const adapter = new BrowserDemoAdapter([]);
    const open = vi.spyOn(window, "open").mockImplementation(() => null);
    await expect(adapter.openExternalUrl("https://example.com/guide")).resolves.toBeUndefined();
    expect(open).toHaveBeenCalledWith("https://example.com/guide", "_blank", "noopener,noreferrer");
    for (const rejected of [
      "javascript:alert(1)",
      "data:text/html,unsafe",
      "https://localhost/admin",
      "https://service.local/path",
      "https://127.0.0.1/",
      "https://192.168.1.2/",
      "https://[::1]/",
      "https://user:pass@example.com/",
      "not a URL",
    ]) {
      await expect(adapter.openExternalUrl(rejected)).rejects.toThrow();
    }
    expect(open).toHaveBeenCalledOnce();
  });

  it("supports isolated conversation CRUD with bounded titles", async () => {
    const adapter = new BrowserDemoAdapter([]);
    const first = await adapter.createConversation("  First conversation  ");
    const second = await adapter.createConversation();
    expect((await adapter.listConversations()).map((item) => item.id)).toEqual([
      second.id,
      first.id,
    ]);
    expect(await adapter.getConversation(first.id)).toMatchObject({ title: "First conversation" });

    const renamed = await adapter.renameConversation(first.id, "Renamed locally");
    expect(renamed.title).toBe("Renamed locally");
    await expect(async () => adapter.renameConversation(first.id, "   ")).rejects.toThrow(
      "title cannot be empty",
    );
    await expect(async () => adapter.getConversation("missing")).rejects.toThrow("not found");
    await expect(async () => adapter.renameConversation("missing", "No item")).rejects.toThrow(
      "not found",
    );

    await adapter.deleteConversation(first.id);
    await adapter.deleteConversation("already-missing");
    expect((await adapter.listConversations()).map((item) => item.id)).toEqual([second.id]);
  });

  it("streams every Unicode character losslessly and persists the exact completed response", async () => {
    const adapter = new BrowserDemoAdapter([]);
    const conversation = await adapter.createConversation();
    const events: ChatStreamEvent[] = [];
    const unsubscribe = await adapter.onChatStream((event) => events.push(event));

    await adapter.sendMessage({
      conversationId: conversation.id,
      content: "Explain a clear plan 🚀",
      reasoningMode: "standard",
    });
    await vi.runAllTimersAsync();

    const deltas = events
      .filter((event) => event.kind === "delta")
      .map((event) => event.delta)
      .join("");
    const completed = events.find((event) => event.kind === "completed");
    expect(events[0]?.kind).toBe("started");
    expect(deltas).toBe(defaultDemoResponse);
    expect(completed?.message?.content).toBe(defaultDemoResponse);
    expect((await adapter.getConversation(conversation.id)).messages.at(-1)?.content).toBe(
      defaultDemoResponse,
    );

    unsubscribe();
  });

  it("cancels an active stream idempotently and retains only the partial text", async () => {
    const adapter = new BrowserDemoAdapter([]);
    const conversation = await adapter.createConversation();
    const events: ChatStreamEvent[] = [];
    await adapter.onChatStream((event) => events.push(event));
    const { requestId } = await adapter.sendMessage({
      conversationId: conversation.id,
      content: "Review a security incident",
      reasoningMode: "deep",
    });

    await vi.advanceTimersByTimeAsync(170);
    await adapter.cancelGeneration(requestId);
    await adapter.cancelGeneration(requestId);
    await vi.runAllTimersAsync();

    expect(events.filter((event) => event.kind === "cancelled")).toHaveLength(1);
    expect(events.some((event) => event.kind === "completed")).toBe(false);
    const persisted = await adapter.getConversation(conversation.id);
    expect(persisted.messages.at(-1)?.status).toBe("cancelled");
    expect(persisted.messages.at(-1)?.content.length).toBeGreaterThan(0);
  });

  it("implements edit and regenerate without duplicating the user prompt", async () => {
    const adapter = new BrowserDemoAdapter([]);
    const conversation = await adapter.createConversation();
    await adapter.sendMessage({
      conversationId: conversation.id,
      content: "Write code",
      reasoningMode: "fast",
    });
    await vi.runAllTimersAsync();
    const initial = await adapter.getConversation(conversation.id);
    const user = initial.messages[0];
    const assistant = initial.messages[1];
    expect(user?.role).toBe("user");
    expect(assistant?.role).toBe("assistant");

    await adapter.sendMessage({
      conversationId: conversation.id,
      content: "Review a threat instead",
      reasoningMode: "deep",
      regenerateFromMessageId: user?.id,
    });
    await vi.runAllTimersAsync();
    const edited = await adapter.getConversation(conversation.id);
    expect(edited.messages).toHaveLength(2);
    expect(edited.messages[0]?.content).toBe("Review a threat instead");

    await adapter.sendMessage({
      conversationId: conversation.id,
      content: "Review a threat instead",
      reasoningMode: "standard",
      regenerateFromMessageId: edited.messages[1]?.id,
    });
    await vi.runAllTimersAsync();
    const regenerated = await adapter.getConversation(conversation.id);
    expect(regenerated.messages.filter((message) => message.role === "user")).toHaveLength(1);
    expect(regenerated.messages).toHaveLength(2);
  });

  it("rejects invalid regeneration targets and non-latest assistant targets", async () => {
    const adapter = new BrowserDemoAdapter([]);
    const conversation = await adapter.createConversation();
    await expect(async () =>
      adapter.sendMessage({
        conversationId: "missing",
        content: "Hello",
        reasoningMode: "standard",
      }),
    ).rejects.toThrow("not found");
    await expect(async () =>
      adapter.sendMessage({
        conversationId: conversation.id,
        content: "Hello",
        reasoningMode: "standard",
        regenerateFromMessageId: "missing-message",
      }),
    ).rejects.toThrow("not found");

    await adapter.sendMessage({
      conversationId: conversation.id,
      content: "First prompt",
      reasoningMode: "standard",
    });
    await vi.runAllTimersAsync();
    const firstAnswer = (await adapter.getConversation(conversation.id)).messages[1];
    await adapter.sendMessage({
      conversationId: conversation.id,
      content: "Second prompt",
      reasoningMode: "standard",
    });
    await vi.runAllTimersAsync();
    await expect(async () =>
      adapter.sendMessage({
        conversationId: conversation.id,
        content: "First prompt",
        reasoningMode: "standard",
        regenerateFromMessageId: firstAnswer?.id,
      }),
    ).rejects.toThrow("latest response");
  });

  it("round-trips valid exports with remapped IDs", async () => {
    const source = new BrowserDemoAdapter([]);
    const conversation = await source.createConversation("Export me");
    await source.sendMessage({
      conversationId: conversation.id,
      content: "Create an export",
      reasoningMode: "standard",
    });
    await vi.runAllTimersAsync();
    const serialized = await source.exportConversation(conversation.id);
    expect(serialized).toContain('"format": "aster-conversation"');

    const target = new BrowserDemoAdapter([]);
    const imported = await target.importConversations(serialized);
    expect(imported).toHaveLength(1);
    expect(imported[0]?.id).toMatch(/^conversation-/);
    const restored = await target.getConversation(imported[0]?.id ?? "");
    expect(restored.messages[1]).toMatchObject({
      conversationId: restored.id,
      role: "assistant",
      status: "complete",
    });
  });

  it.each([
    [undefined, "Choose an Aster JSON export"],
    ["not json", "not valid JSON"],
    [
      JSON.stringify({ format: "wrong", version: 1, exportedAt: "x", conversations: [] }),
      "not a supported",
    ],
    [
      JSON.stringify({
        format: "aster-conversation",
        version: 1,
        exportedAt: "x",
        conversations: [],
        secret: "x",
      }),
      "unsupported fields",
    ],
  ])("rejects malformed import input without mutation", async (serialized, message) => {
    const adapter = new BrowserDemoAdapter([]);
    await expect(async () => adapter.importConversations(serialized)).rejects.toThrow(message);
    await expect(adapter.listConversations()).resolves.toEqual([]);
  });

  it("validates every imported conversation before mutation", async () => {
    const adapter = new BrowserDemoAdapter([]);
    const invalidBundle = JSON.stringify({
      format: "aster-conversation",
      version: 1,
      exportedAt: "2026-07-11T12:00:00.000Z",
      conversations: [
        validConversation(),
        {
          title: "Invalid second item",
          reasoningMode: "standard",
          createdAt: "2026-07-11T12:00:00.000Z",
          updatedAt: "2026-07-11T12:00:00.000Z",
          messages: [
            {
              role: "system",
              content: "Unsupported role",
              createdAt: "2026-07-11T12:00:00.000Z",
              status: "complete",
            },
          ],
        },
      ],
    });

    await expect(async () => adapter.importConversations(invalidBundle)).rejects.toThrow(
      "unsupported message role",
    );
    await expect(adapter.listConversations()).resolves.toEqual([]);
  });
});
