import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatStreamEvent, Conversation } from "./types/chat";

const desktopMocks = vi.hoisted(() => {
  const state: { listener?: (event: unknown) => void } = {};
  return {
    state,
    adapter: {
      runtime: "tauri" as const,
      appStatus: vi.fn(),
      modelCatalog: vi.fn(),
      providerStatuses: vi.fn(),
      promptStoreApiKey: vi.fn(),
      deleteApiKey: vi.fn(),
      listConversations: vi.fn(),
      getConversation: vi.fn(),
      createConversation: vi.fn(),
      updateConversationSelection: vi.fn(),
      renameConversation: vi.fn(),
      deleteConversation: vi.fn(),
      sendMessage: vi.fn(),
      cancelGeneration: vi.fn(),
      onChatStream: vi.fn(),
      acknowledgeExternalProcessing: vi.fn(),
      usageSummary: vi.fn(),
      setUsageBudget: vi.fn(),
      deepSeekBalanceStatus: vi.fn(),
      refreshDeepSeekBalance: vi.fn(),
      openProviderAccount: vi.fn(),
      openExternalUrl: vi.fn(),
      exportConversation: vi.fn(),
      importConversations: vi.fn(),
    },
  };
});

vi.mock("./services/assistantAdapter", () => ({ assistantAdapter: desktopMocks.adapter }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn(),
  }),
}));

import App from "./App";

const createdConversation: Conversation = {
  id: "conversation-authoritative",
  title: "New conversation",
  providerId: "zai",
  modelId: "glm-5.1",
  responseProfile: "standard",
  createdAt: "2026-07-11T12:00:00.000Z",
  updatedAt: "2026-07-11T12:00:00.000Z",
  messageCount: 0,
  messages: [],
};

function deferred<T>() {
  let resolvePromise: (value: T) => void = () => undefined;
  const promise = new Promise<T>((resolve) => {
    resolvePromise = resolve;
  });
  return { promise, resolve: resolvePromise };
}

describe("Aster desktop reconciliation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    desktopMocks.state.listener = undefined;
    desktopMocks.adapter.appStatus.mockResolvedValue({
      mode: "desktop",
      version: "0.1.0",
      online: false,
      providerReachability: "unknown",
      externalProcessingAcknowledged: false,
      databaseReady: true,
    });
    desktopMocks.adapter.modelCatalog.mockResolvedValue({
      version: 2,
      defaultSelection: { providerId: "zai", modelId: "glm-5.1" },
      providers: [
        {
          id: "zai",
          displayName: "Z.AI",
          regionLabel: null,
          noticeVersion: 1,
          processingNotice: "Messages leave this device for processing by Z.AI.",
          accountActions: [],
          models: [
            {
              id: "glm-5.1",
              displayName: "GLM-5.1",
              delivery: "official-api",
              profiles: [
                {
                  id: "fast",
                  label: "Fast",
                  description: "Fast response",
                  enabled: true,
                  disabledReason: null,
                },
                {
                  id: "standard",
                  label: "Standard",
                  description: "Balanced response",
                  enabled: true,
                  disabledReason: null,
                },
                {
                  id: "deep",
                  label: "Deep",
                  description: "Deeper response",
                  enabled: true,
                  disabledReason: null,
                },
              ],
            },
            {
              id: "glm-5.2",
              displayName: "GLM-5.2",
              delivery: "official-api",
              profiles: [
                {
                  id: "fast",
                  label: "Fast",
                  description: "Fast response",
                  enabled: true,
                  disabledReason: null,
                },
                {
                  id: "standard",
                  label: "Standard",
                  description: "Balanced response",
                  enabled: true,
                  disabledReason: null,
                },
                {
                  id: "deep",
                  label: "Deep",
                  description: "Deeper response",
                  enabled: true,
                  disabledReason: null,
                },
              ],
            },
          ],
        },
      ],
    });
    desktopMocks.adapter.providerStatuses.mockResolvedValue([
      {
        providerId: "zai",
        configured: true,
        reachability: "unknown",
        noticeVersion: 0,
        noticeAcknowledged: false,
      },
    ]);
    desktopMocks.adapter.listConversations.mockResolvedValue([]);
    desktopMocks.adapter.createConversation.mockResolvedValue(createdConversation);
    desktopMocks.adapter.onChatStream.mockImplementation(
      (listener: (event: ChatStreamEvent) => void) => {
        desktopMocks.state.listener = listener as (event: unknown) => void;
        return Promise.resolve(() => undefined);
      },
    );
    desktopMocks.adapter.acknowledgeExternalProcessing.mockImplementation(() => {
      desktopMocks.adapter.providerStatuses.mockResolvedValue([
        {
          providerId: "zai",
          configured: true,
          reachability: "unknown",
          noticeVersion: 1,
          noticeAcknowledged: true,
        },
      ]);
      return Promise.resolve(undefined);
    });
    desktopMocks.adapter.promptStoreApiKey.mockResolvedValue({
      configured: true,
      source: "credential-vault",
      cancelled: false,
    });
  });

  afterEach(cleanup);

  it("keeps revision disabled until terminal reconciliation replaces optimistic IDs", async () => {
    const authoritative = deferred<Conversation>();
    desktopMocks.adapter.getConversation.mockReturnValue(authoritative.promise);
    desktopMocks.adapter.sendMessage.mockResolvedValueOnce({ requestId: "request-1" });
    desktopMocks.adapter.sendMessage.mockResolvedValueOnce({ requestId: "request-2" });

    const emitTerminal = () => {
      const listener = desktopMocks.state.listener as
        ((event: ChatStreamEvent) => void) | undefined;
      listener?.({
        requestId: "request-1",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 0,
        kind: "started",
      });
      listener?.({
        requestId: "request-1",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 1,
        kind: "completed",
        message: {
          id: "assistant-authoritative",
          conversationId: createdConversation.id,
          role: "assistant",
          content: "Authoritative answer",
          createdAt: "2026-07-11T12:01:00.000Z",
          status: "complete",
          finishReason: "stop",
        },
      });
    };

    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getByRole("button", { name: /New chat/i }));
    await user.type(
      screen.getByRole("textbox", { name: "Message Aster" }),
      "Original prompt{enter}",
    );
    await user.click(await screen.findByRole("button", { name: "Continue and send" }));
    await waitFor(() => {
      expect(desktopMocks.adapter.sendMessage).toHaveBeenCalledOnce();
    });
    await act(() => {
      emitTerminal();
      return Promise.resolve();
    });

    expect(
      (await screen.findByRole<HTMLButtonElement>("button", { name: /Edit and resend/i })).disabled,
    ).toBe(true);
    await act(async () => {
      authoritative.resolve({
        ...createdConversation,
        title: "Original prompt",
        updatedAt: "2026-07-11T12:01:00.000Z",
        messageCount: 2,
        messages: [
          {
            id: "user-authoritative",
            conversationId: createdConversation.id,
            role: "user",
            content: "Original prompt",
            createdAt: "2026-07-11T12:00:30.000Z",
            status: "complete",
          },
          {
            id: "assistant-authoritative",
            conversationId: createdConversation.id,
            role: "assistant",
            content: "Authoritative answer",
            createdAt: "2026-07-11T12:01:00.000Z",
            status: "complete",
            finishReason: "stop",
          },
        ],
      });
      await authoritative.promise;
    });

    const edit = await waitFor(() => {
      const button = screen.getByRole<HTMLButtonElement>("button", { name: /Edit and resend/i });
      expect(button.disabled).toBe(false);
      return button;
    });
    await user.click(edit);
    const editor = screen.getByRole("textbox", { name: "Edit message" });
    await user.clear(editor);
    await user.type(editor, "Revised prompt");
    fireEvent.keyDown(editor, { key: "Enter" });

    await waitFor(() => {
      expect(desktopMocks.adapter.sendMessage).toHaveBeenCalledTimes(2);
    });
    expect(desktopMocks.adapter.sendMessage.mock.calls[1]?.[0]).toMatchObject({
      regenerateFromMessageId: "user-authoritative",
      content: "Revised prompt",
    });
  });

  it("keeps an output-limit notice adjacent to the completed response", async () => {
    const outputLimitedConversation: Conversation = {
      ...createdConversation,
      title: "Bounded answer",
      updatedAt: "2026-07-11T12:01:00.000Z",
      messageCount: 2,
      messages: [
        {
          id: "user-output-limit",
          conversationId: createdConversation.id,
          role: "user",
          content: "Give me a bounded answer",
          createdAt: "2026-07-11T12:00:30.000Z",
          status: "complete",
        },
        {
          id: "assistant-output-limit",
          conversationId: createdConversation.id,
          role: "assistant",
          content: "Partial answer from the provider.",
          createdAt: "2026-07-11T12:01:00.000Z",
          status: "complete",
          finishReason: "outputLimit",
        },
      ],
    };
    desktopMocks.adapter.getConversation.mockResolvedValue(outputLimitedConversation);
    desktopMocks.adapter.sendMessage.mockImplementationOnce(() => {
      const listener = desktopMocks.state.listener as
        ((event: ChatStreamEvent) => void) | undefined;
      listener?.({
        requestId: "request-output-limit",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 0,
        kind: "started",
      });
      listener?.({
        requestId: "request-output-limit",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 1,
        kind: "completed",
        message: outputLimitedConversation.messages[1],
      });
      return Promise.resolve({ requestId: "request-output-limit" });
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getByRole("button", { name: /New chat/i }));
    await user.type(
      screen.getByRole("textbox", { name: "Message Aster" }),
      "Give me a bounded answer{enter}",
    );
    await user.click(await screen.findByRole("button", { name: "Continue and send" }));

    const notice = await screen.findByRole("note", { name: "Incomplete response notice" });
    expect(notice.textContent).toContain("The provider reached Aster's output limit.");
    expect(notice.textContent).toContain("This response may be incomplete.");
    expect(notice.closest("article")?.textContent).toContain("Partial answer from the provider.");
    expect(screen.queryByText("Response failed")).toBeNull();
  });

  it("retries a failed answer by authoritative regeneration without duplicating the prompt", async () => {
    const failedConversation: Conversation = {
      ...createdConversation,
      title: "Original prompt",
      messageCount: 2,
      messages: [
        {
          id: "user-authoritative",
          conversationId: createdConversation.id,
          role: "user",
          content: "Original prompt",
          createdAt: "2026-07-11T12:00:30.000Z",
          status: "complete",
        },
        {
          id: "error-authoritative",
          conversationId: createdConversation.id,
          role: "assistant",
          content: "",
          createdAt: "2026-07-11T12:01:00.000Z",
          status: "error",
        },
      ],
    };
    desktopMocks.adapter.getConversation.mockResolvedValue(failedConversation);
    desktopMocks.adapter.sendMessage.mockImplementationOnce(() => {
      const listener = desktopMocks.state.listener as
        ((event: ChatStreamEvent) => void) | undefined;
      listener?.({
        requestId: "request-error",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 0,
        kind: "started",
      });
      listener?.({
        requestId: "request-error",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 1,
        kind: "error",
        message: failedConversation.messages[1],
        error: "The provider is temporarily unavailable.",
        errorCode: "provider_unavailable",
        retryable: true,
      });
      return Promise.resolve({ requestId: "request-error" });
    });
    desktopMocks.adapter.sendMessage.mockResolvedValueOnce({ requestId: "request-retry" });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getByRole("button", { name: /New chat/i }));
    await user.type(
      screen.getByRole("textbox", { name: "Message Aster" }),
      "Original prompt{enter}",
    );
    await user.click(await screen.findByRole("button", { name: "Continue and send" }));
    await user.click(await screen.findByRole("button", { name: "Retry response" }));

    await waitFor(() => {
      expect(desktopMocks.adapter.sendMessage).toHaveBeenCalledTimes(2);
    });
    expect(desktopMocks.adapter.sendMessage.mock.calls[1]?.[0]).toMatchObject({
      content: "Original prompt",
      regenerateFromMessageId: "error-authoritative",
    });
  });

  it("retries a terminal database error from the authoritative user message", async () => {
    const userOnly: Conversation = {
      ...createdConversation,
      title: "Persisted prompt",
      messageCount: 1,
      messages: [
        {
          id: "user-db-authoritative",
          conversationId: createdConversation.id,
          role: "user",
          content: "Persisted prompt",
          createdAt: "2026-07-11T12:00:30.000Z",
          status: "complete",
        },
      ],
    };
    desktopMocks.adapter.getConversation.mockResolvedValue(userOnly);
    desktopMocks.adapter.sendMessage.mockImplementationOnce(() => {
      const listener = desktopMocks.state.listener as
        ((event: ChatStreamEvent) => void) | undefined;
      listener?.({
        requestId: "request-db-error",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 0,
        kind: "started",
      });
      listener?.({
        requestId: "request-db-error",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 1,
        kind: "error",
        error: "Local conversation storage is unavailable.",
        errorCode: "database_error",
        retryable: true,
      });
      return Promise.resolve({ requestId: "request-db-error" });
    });
    desktopMocks.adapter.sendMessage.mockResolvedValueOnce({ requestId: "request-db-retry" });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getByRole("button", { name: /New chat/i }));
    await user.type(
      screen.getByRole("textbox", { name: "Message Aster" }),
      "Persisted prompt{enter}",
    );
    await user.click(await screen.findByRole("button", { name: "Continue and send" }));
    await user.click(await screen.findByRole("button", { name: "Retry response" }));

    await waitFor(() => {
      expect(desktopMocks.adapter.sendMessage).toHaveBeenCalledTimes(2);
    });
    expect(desktopMocks.adapter.sendMessage.mock.calls[1]?.[0]).toMatchObject({
      content: "Persisted prompt",
      regenerateFromMessageId: "user-db-authoritative",
    });
  });

  it("requires deliberate confirmation before deleting the saved credential", async () => {
    desktopMocks.adapter.deleteApiKey.mockResolvedValue({ configured: false, source: "none" });
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getAllByRole("button", { name: /settings/i })[0] as HTMLElement);
    expect(document.querySelectorAll("#provider-settings input")).toHaveLength(0);
    await user.click(await screen.findByRole("button", { name: "Replace Z.AI API key" }));
    await waitFor(() => {
      expect(desktopMocks.adapter.promptStoreApiKey).toHaveBeenCalledWith("zai");
    });
    await user.click(await screen.findByRole("button", { name: "Remove Z.AI API key" }));
    expect(await screen.findByRole("dialog", { name: "Remove saved API key?" })).toBeTruthy();
    expect(desktopMocks.adapter.deleteApiKey).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Keep saved key" }));
    expect(await screen.findByRole("dialog", { name: "Settings" })).toBeTruthy();
    expect(desktopMocks.adapter.deleteApiKey).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Remove Z.AI API key" }));
    await user.click(await screen.findByRole("button", { name: "Remove key" }));
    await waitFor(() => {
      expect(desktopMocks.adapter.deleteApiKey).toHaveBeenCalledWith("zai");
    });
  });

  it("keeps API keys out of desktop DOM and opens the provider-scoped native prompt", async () => {
    desktopMocks.adapter.providerStatuses.mockResolvedValue([
      {
        providerId: "zai",
        configured: false,
        reachability: "unknown",
        noticeVersion: 0,
        noticeAcknowledged: false,
      },
    ]);
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getAllByRole("button", { name: /settings/i })[0] as HTMLElement);

    const settings = await screen.findByRole("dialog", { name: "Settings" });
    expect(settings.querySelectorAll("input")).toHaveLength(0);
    await user.click(screen.getByRole("button", { name: "Add Z.AI API key" }));

    await waitFor(() => {
      expect(desktopMocks.adapter.promptStoreApiKey).toHaveBeenCalledOnce();
    });
    expect(desktopMocks.adapter.promptStoreApiKey.mock.calls[0]).toEqual(["zai"]);
  });

  it("treats provider unreachability as advisory and permits a deliberate new probe", async () => {
    desktopMocks.adapter.appStatus.mockResolvedValue({
      mode: "desktop",
      version: "0.1.0",
      online: false,
      providerReachability: "unreachable",
      externalProcessingAcknowledged: true,
      databaseReady: true,
    });
    desktopMocks.adapter.providerStatuses.mockResolvedValue([
      {
        providerId: "zai",
        configured: true,
        reachability: "unreachable",
        noticeVersion: 1,
        noticeAcknowledged: true,
      },
    ]);
    desktopMocks.adapter.sendMessage.mockResolvedValue({ requestId: "request-probe" });
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getByRole("button", { name: /New chat/i }));

    const composer = screen.getByRole<HTMLTextAreaElement>("textbox", { name: "Message Aster" });
    expect(composer.placeholder).toBe("Z.AI was unreachable on the last request");
    await user.type(composer, "Probe Z.AI again");
    const send = screen.getByRole<HTMLButtonElement>("button", { name: "Send message" });
    expect(send.disabled).toBe(false);
    await user.click(send);

    await waitFor(() => {
      expect(desktopMocks.adapter.sendMessage).toHaveBeenCalledWith(
        expect.objectContaining({ content: "Probe Z.AI again" }),
      );
    });
  });

  it("preserves configured status when the native credential prompt is cancelled", async () => {
    desktopMocks.adapter.promptStoreApiKey.mockResolvedValue({
      configured: true,
      source: "credential-vault",
      cancelled: true,
    });
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getAllByRole("button", { name: /settings/i })[0] as HTMLElement);
    await user.click(await screen.findByRole("button", { name: "Replace Z.AI API key" }));

    expect(await screen.findByText("API key setup cancelled")).toBeTruthy();
    expect(screen.getByText("Key configured")).toBeTruthy();
    expect(screen.queryByText(/saved in Windows Credential Manager/i)).toBeNull();
    expect(desktopMocks.adapter.promptStoreApiKey).toHaveBeenCalledWith("zai");
  });

  it("terminalizes and reconciles a stream sequence gap instead of wedging", async () => {
    const recovered: Conversation = {
      ...createdConversation,
      title: "Sequence test",
      messageCount: 2,
      messages: [
        {
          id: "user-sequence",
          conversationId: createdConversation.id,
          role: "user",
          content: "Sequence test",
          createdAt: "2026-07-11T12:00:30.000Z",
          status: "complete",
        },
        {
          id: "error-sequence",
          conversationId: createdConversation.id,
          role: "assistant",
          content: "",
          createdAt: "2026-07-11T12:01:00.000Z",
          status: "error",
        },
      ],
    };
    desktopMocks.adapter.getConversation.mockResolvedValue(recovered);
    desktopMocks.adapter.cancelGeneration.mockImplementation(() => {
      const listener = desktopMocks.state.listener as
        ((event: ChatStreamEvent) => void) | undefined;
      queueMicrotask(() => {
        listener?.({
          requestId: "request-gap",
          conversationId: createdConversation.id,
          providerId: "zai",
          modelId: "glm-5.1",
          sequence: 3,
          kind: "cancelled",
        });
      });
      return Promise.resolve();
    });
    desktopMocks.adapter.sendMessage.mockImplementationOnce(() => {
      const listener = desktopMocks.state.listener as
        ((event: ChatStreamEvent) => void) | undefined;
      listener?.({
        requestId: "request-gap",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 0,
        kind: "started",
      });
      listener?.({
        requestId: "request-gap",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 2,
        kind: "delta",
        delta: "out of order",
      });
      return Promise.resolve({ requestId: "request-gap" });
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getByRole("button", { name: /New chat/i }));
    await user.type(screen.getByRole("textbox", { name: "Message Aster" }), "Sequence test{enter}");
    await user.click(await screen.findByRole("button", { name: "Continue and send" }));

    expect(await screen.findByText(/arrived out of order/i)).toBeTruthy();
    await waitFor(() => {
      expect(desktopMocks.adapter.cancelGeneration).toHaveBeenCalledWith("request-gap");
      expect(screen.getByRole("button", { name: "Retry response" })).toBeTruthy();
    });
    expect(screen.queryByText("out of order", { exact: true })).toBeNull();
  });

  it("buffers pre-return events and adopts only the authoritative returned request ID", async () => {
    const sendResult = deferred<{ requestId: string }>();
    const authoritative: Conversation = {
      ...createdConversation,
      title: "Authoritative request",
      messageCount: 2,
      messages: [
        {
          id: "user-authoritative-request",
          conversationId: createdConversation.id,
          role: "user",
          content: "Use only the authoritative request",
          createdAt: "2026-07-11T12:00:30.000Z",
          status: "complete",
        },
        {
          id: "assistant-authoritative-request",
          conversationId: createdConversation.id,
          role: "assistant",
          content: "Authoritative response",
          createdAt: "2026-07-11T12:01:00.000Z",
          status: "complete",
          finishReason: "stop",
        },
      ],
    };
    desktopMocks.adapter.getConversation.mockResolvedValue(authoritative);
    desktopMocks.adapter.cancelGeneration.mockResolvedValue(undefined);
    desktopMocks.adapter.sendMessage.mockImplementationOnce(() => {
      const listener = desktopMocks.state.listener as
        ((event: ChatStreamEvent) => void) | undefined;
      listener?.({
        requestId: "request-crossed",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 0,
        kind: "started",
      });
      listener?.({
        requestId: "request-crossed",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 1,
        kind: "delta",
        delta: "Crossed response",
      });
      return sendResult.promise;
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getByRole("button", { name: /New chat/i }));
    await user.type(
      screen.getByRole("textbox", { name: "Message Aster" }),
      "Use only the authoritative request{enter}",
    );
    await user.click(await screen.findByRole("button", { name: "Continue and send" }));
    await waitFor(() => {
      expect(desktopMocks.adapter.sendMessage).toHaveBeenCalledOnce();
    });

    await act(async () => {
      sendResult.resolve({ requestId: "request-authoritative" });
      await sendResult.promise;
    });
    act(() => {
      const listener = desktopMocks.state.listener as
        ((event: ChatStreamEvent) => void) | undefined;
      listener?.({
        requestId: "request-authoritative",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 0,
        kind: "started",
      });
      listener?.({
        requestId: "request-authoritative",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 1,
        kind: "completed",
        message: authoritative.messages[1],
      });
    });

    expect(await screen.findByText("Authoritative response")).toBeTruthy();
    expect(screen.queryByText("Crossed response")).toBeNull();
    expect(desktopMocks.adapter.cancelGeneration).not.toHaveBeenCalledWith("request-crossed");
  });

  it("replays authoritative events that all arrive before the command returns", async () => {
    const sendResult = deferred<{ requestId: string }>();
    const authoritative: Conversation = {
      ...createdConversation,
      title: "Buffered authoritative request",
      messageCount: 2,
      messages: [
        {
          id: "user-buffered-authoritative",
          conversationId: createdConversation.id,
          role: "user",
          content: "Replay the authoritative buffer",
          createdAt: "2026-07-11T12:00:30.000Z",
          status: "complete",
        },
        {
          id: "assistant-buffered-authoritative",
          conversationId: createdConversation.id,
          role: "assistant",
          content: "Buffered authoritative response",
          createdAt: "2026-07-11T12:01:00.000Z",
          status: "complete",
          finishReason: "stop",
        },
      ],
    };
    desktopMocks.adapter.getConversation.mockResolvedValue(authoritative);
    desktopMocks.adapter.sendMessage.mockImplementationOnce(() => {
      const listener = desktopMocks.state.listener as
        ((event: ChatStreamEvent) => void) | undefined;
      listener?.({
        requestId: "request-buffered-authoritative",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 0,
        kind: "started",
      });
      listener?.({
        requestId: "request-buffered-authoritative",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 1,
        kind: "delta",
        delta: "Buffered authoritative response",
      });
      listener?.({
        requestId: "request-buffered-authoritative",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 2,
        kind: "completed",
        message: authoritative.messages[1],
      });
      return sendResult.promise;
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getByRole("button", { name: /New chat/i }));
    await user.type(
      screen.getByRole("textbox", { name: "Message Aster" }),
      "Replay the authoritative buffer{enter}",
    );
    await user.click(await screen.findByRole("button", { name: "Continue and send" }));
    await waitFor(() => {
      expect(desktopMocks.adapter.sendMessage).toHaveBeenCalledOnce();
    });
    expect(screen.queryByText("Buffered authoritative response")).toBeNull();

    await act(async () => {
      sendResult.resolve({ requestId: "request-buffered-authoritative" });
      await sendResult.promise;
    });

    expect(await screen.findByText("Buffered authoritative response")).toBeTruthy();
    expect(desktopMocks.adapter.cancelGeneration).not.toHaveBeenCalled();
  });

  it("queues Stop until the authoritative request ID returns", async () => {
    const sendResult = deferred<{ requestId: string }>();
    desktopMocks.adapter.cancelGeneration.mockResolvedValue(undefined);
    desktopMocks.adapter.sendMessage.mockImplementationOnce(() => {
      const listener = desktopMocks.state.listener as
        ((event: ChatStreamEvent) => void) | undefined;
      listener?.({
        requestId: "request-crossed-before-stop",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 0,
        kind: "started",
      });
      return sendResult.promise;
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getByRole("button", { name: /New chat/i }));
    await user.type(
      screen.getByRole("textbox", { name: "Message Aster" }),
      "Stop only the authoritative request{enter}",
    );
    await user.click(await screen.findByRole("button", { name: "Continue and send" }));
    await waitFor(() => {
      expect(desktopMocks.adapter.sendMessage).toHaveBeenCalledOnce();
    });
    await user.click(await screen.findByRole("button", { name: "Stop generation" }));
    expect(desktopMocks.adapter.cancelGeneration).not.toHaveBeenCalled();

    await act(async () => {
      sendResult.resolve({ requestId: "request-authoritative-stop" });
      await sendResult.promise;
    });
    await waitFor(() => {
      expect(desktopMocks.adapter.cancelGeneration).toHaveBeenCalledWith(
        "request-authoritative-stop",
      );
    });
    expect(desktopMocks.adapter.cancelGeneration).not.toHaveBeenCalledWith(
      "request-crossed-before-stop",
    );
  });

  it("accepts more than 4,096 ordered tiny delta events within the byte ceiling", async () => {
    const deltaCount = 4_100;
    desktopMocks.adapter.sendMessage.mockImplementationOnce(() => {
      const listener = desktopMocks.state.listener as
        ((event: ChatStreamEvent) => void) | undefined;
      listener?.({
        requestId: "request-many-deltas",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 0,
        kind: "started",
      });
      for (let sequence = 1; sequence <= deltaCount; sequence += 1) {
        listener?.({
          requestId: "request-many-deltas",
          conversationId: createdConversation.id,
          providerId: "zai",
          modelId: "glm-5.1",
          sequence,
          kind: "delta",
          delta: "x",
        });
      }
      return Promise.resolve({ requestId: "request-many-deltas" });
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getByRole("button", { name: /New chat/i }));
    await user.type(
      screen.getByRole("textbox", { name: "Message Aster" }),
      "Accept many ordered deltas{enter}",
    );
    await user.click(await screen.findByRole("button", { name: "Continue and send" }));

    await waitFor(() => {
      expect(document.querySelector(".assistant-content")?.textContent).toContain(
        "x".repeat(deltaCount),
      );
    });
    expect(screen.queryByText(/exceeded Aster's safety limits/i)).toBeNull();
    expect(screen.getByRole("button", { name: "Stop generation" })).toBeTruthy();
  });

  it("accepts exactly 65,536 buffered events and fails closed on post-command event 65,537", async () => {
    desktopMocks.adapter.cancelGeneration.mockResolvedValue(undefined);
    desktopMocks.adapter.sendMessage.mockImplementationOnce(() => {
      const listener = desktopMocks.state.listener as
        ((event: ChatStreamEvent) => void) | undefined;
      listener?.({
        requestId: "request-event-limit",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 0,
        kind: "started",
      });
      for (let sequence = 1; sequence < 65_536; sequence += 1) {
        listener?.({
          requestId: "request-event-limit",
          conversationId: createdConversation.id,
          providerId: "zai",
          modelId: "glm-5.1",
          sequence,
          kind: "delta",
          delta: "",
        });
      }
      return Promise.resolve({ requestId: "request-event-limit" });
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getByRole("button", { name: /New chat/i }));
    await user.type(
      screen.getByRole("textbox", { name: "Message Aster" }),
      "Enforce the post-command event ceiling{enter}",
    );
    await user.click(await screen.findByRole("button", { name: "Continue and send" }));
    await waitFor(() => {
      expect(desktopMocks.adapter.sendMessage).toHaveBeenCalledOnce();
    });

    const listener = desktopMocks.state.listener as ((event: ChatStreamEvent) => void) | undefined;
    expect(listener).toBeDefined();
    expect(screen.queryByText(/exceeded Aster's safety limits/i)).toBeNull();
    expect(screen.getByRole("button", { name: "Stop generation" })).toBeTruthy();

    await act(async () => {
      listener?.({
        requestId: "request-event-limit",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 65_536,
        kind: "delta",
        delta: "",
      });
      await Promise.resolve();
    });

    expect(await screen.findByText(/exceeded Aster's safety limits/i)).toBeTruthy();
    expect(desktopMocks.adapter.cancelGeneration).toHaveBeenCalledWith("request-event-limit");
  }, 30_000);

  it.each([
    {
      name: "event-count",
      emit: (listener: (event: ChatStreamEvent) => void) => {
        for (let sequence = 0; sequence <= 65_536; sequence += 1) {
          listener({
            requestId: "request-buffer-limit",
            conversationId: createdConversation.id,
            providerId: "zai",
            modelId: "glm-5.1",
            sequence,
            kind: sequence === 0 ? "started" : "delta",
            ...(sequence === 0 ? {} : { delta: "" }),
          });
        }
      },
    },
    {
      name: "UTF-8 byte",
      emit: (listener: (event: ChatStreamEvent) => void) => {
        listener({
          requestId: "request-buffer-limit",
          conversationId: createdConversation.id,
          providerId: "zai",
          modelId: "glm-5.1",
          sequence: 0,
          kind: "delta",
          delta: "é".repeat(1_048_577),
        });
      },
    },
  ])("fails closed when the pre-return $name buffer exceeds its bound", async ({ emit }) => {
    desktopMocks.adapter.cancelGeneration.mockResolvedValue(undefined);
    desktopMocks.adapter.sendMessage.mockImplementationOnce(() => {
      const listener = desktopMocks.state.listener as
        ((event: ChatStreamEvent) => void) | undefined;
      if (listener) emit(listener);
      return Promise.resolve({ requestId: "request-buffer-limit" });
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getByRole("button", { name: /New chat/i }));
    await user.type(
      screen.getByRole("textbox", { name: "Message Aster" }),
      "Bound the buffered stream{enter}",
    );
    await user.click(await screen.findByRole("button", { name: "Continue and send" }));

    expect(
      await screen.findByText(/exceeded Aster's safety limits before it started/i),
    ).toBeTruthy();
    expect(desktopMocks.adapter.cancelGeneration).toHaveBeenCalledWith("request-buffer-limit");
  });

  it("ignores late events after a terminal event while reconciliation is pending", async () => {
    const authoritative = deferred<Conversation>();
    desktopMocks.adapter.getConversation.mockReturnValue(authoritative.promise);
    desktopMocks.adapter.sendMessage.mockImplementationOnce(() => {
      const listener = desktopMocks.state.listener as
        ((event: ChatStreamEvent) => void) | undefined;
      listener?.({
        requestId: "request-terminal",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 0,
        kind: "started",
      });
      listener?.({
        requestId: "request-terminal",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 1,
        kind: "completed",
        message: {
          id: "assistant-terminal",
          conversationId: createdConversation.id,
          role: "assistant",
          content: "Terminal response",
          createdAt: "2026-07-11T12:01:00.000Z",
          status: "complete",
          finishReason: "stop",
        },
      });
      return Promise.resolve({ requestId: "request-terminal" });
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getByRole("button", { name: /New chat/i }));
    await user.type(
      screen.getByRole("textbox", { name: "Message Aster" }),
      "Reject late stream resurrection{enter}",
    );
    await user.click(await screen.findByRole("button", { name: "Continue and send" }));
    expect(await screen.findByText("Terminal response")).toBeTruthy();

    act(() => {
      const listener = desktopMocks.state.listener as
        ((event: ChatStreamEvent) => void) | undefined;
      listener?.({
        requestId: "request-terminal",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 0,
        kind: "delta",
        delta: "Late response must be ignored",
      });
    });
    expect(screen.queryByText("Late response must be ignored")).toBeNull();

    await act(async () => {
      authoritative.resolve({
        ...createdConversation,
        title: "Reject late stream resurrection",
        messageCount: 2,
        messages: [
          {
            id: "user-terminal",
            conversationId: createdConversation.id,
            role: "user",
            content: "Reject late stream resurrection",
            createdAt: "2026-07-11T12:00:30.000Z",
            status: "complete",
          },
          {
            id: "assistant-terminal",
            conversationId: createdConversation.id,
            role: "assistant",
            content: "Terminal response",
            createdAt: "2026-07-11T12:01:00.000Z",
            status: "complete",
            finishReason: "stop",
          },
        ],
      });
      await authoritative.promise;
    });
  });

  it("rejects an authoritative terminal event that arrives before started sequence zero", async () => {
    const recovered: Conversation = {
      ...createdConversation,
      title: "Early terminal",
      messageCount: 2,
      messages: [
        {
          id: "user-early-terminal",
          conversationId: createdConversation.id,
          role: "user",
          content: "Reject an early terminal",
          createdAt: "2026-07-11T12:00:30.000Z",
          status: "complete",
        },
        {
          id: "assistant-early-terminal-error",
          conversationId: createdConversation.id,
          role: "assistant",
          content: "",
          createdAt: "2026-07-11T12:01:00.000Z",
          status: "error",
        },
      ],
    };
    desktopMocks.adapter.getConversation.mockResolvedValue(recovered);
    desktopMocks.adapter.cancelGeneration.mockResolvedValue(undefined);
    desktopMocks.adapter.sendMessage.mockImplementationOnce(() => {
      const listener = desktopMocks.state.listener as
        ((event: ChatStreamEvent) => void) | undefined;
      listener?.({
        requestId: "request-early-terminal",
        conversationId: createdConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 0,
        kind: "completed",
        message: {
          id: "assistant-invalid-early-terminal",
          conversationId: createdConversation.id,
          role: "assistant",
          content: "This terminal must not be accepted.",
          createdAt: "2026-07-11T12:01:00.000Z",
          status: "complete",
          finishReason: "stop",
        },
      });
      return Promise.resolve({ requestId: "request-early-terminal" });
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getByRole("button", { name: /New chat/i }));
    await user.type(
      screen.getByRole("textbox", { name: "Message Aster" }),
      "Reject an early terminal{enter}",
    );
    await user.click(await screen.findByRole("button", { name: "Continue and send" }));

    expect(await screen.findByText(/did not begin correctly/i)).toBeTruthy();
    expect(screen.queryByText("This terminal must not be accepted.")).toBeNull();
    expect(desktopMocks.adapter.cancelGeneration).toHaveBeenCalledWith("request-early-terminal");
  });

  it("clears the prior conversation while a newly selected conversation is loading", async () => {
    const firstConversation: Conversation = {
      ...createdConversation,
      id: "conversation-first",
      title: "First conversation",
      messageCount: 1,
      messages: [
        {
          id: "first-user",
          conversationId: "conversation-first",
          role: "user",
          content: "First conversation content",
          createdAt: "2026-07-11T12:00:30.000Z",
          status: "complete",
        },
      ],
    };
    const secondConversation: Conversation = {
      ...createdConversation,
      id: "conversation-second",
      title: "Second conversation",
      messageCount: 1,
      messages: [
        {
          id: "second-user",
          conversationId: "conversation-second",
          role: "user",
          content: "Second conversation content",
          createdAt: "2026-07-11T12:01:30.000Z",
          status: "complete",
        },
      ],
    };
    const secondLoad = deferred<Conversation>();
    desktopMocks.adapter.listConversations.mockResolvedValue([
      { ...firstConversation, messages: undefined },
      { ...secondConversation, messages: undefined },
    ]);
    desktopMocks.adapter.getConversation.mockImplementation((id: string) =>
      id === firstConversation.id ? Promise.resolve(firstConversation) : secondLoad.promise,
    );

    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: firstConversation.title }));
    expect(await screen.findByText("First conversation content")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: secondConversation.title }));
    expect(screen.getByLabelText("Loading conversation")).toBeTruthy();
    expect(screen.queryByText("First conversation content")).toBeNull();
    expect(screen.queryByRole("textbox", { name: "Message Aster" })).toBeNull();
    expect(screen.getByRole<HTMLButtonElement>("button", { name: /New chat/i }).disabled).toBe(
      true,
    );
    fireEvent.keyDown(window, { key: "n", ctrlKey: true });
    expect(desktopMocks.adapter.createConversation).not.toHaveBeenCalled();

    await act(async () => {
      secondLoad.resolve(secondConversation);
      await secondLoad.promise;
    });
    expect(await screen.findByText("Second conversation content")).toBeTruthy();
  });

  it("keeps the newer navigation authoritative when two conversation loads overlap", async () => {
    const olderConversation: Conversation = {
      ...createdConversation,
      id: "conversation-overlap-older",
      title: "Older pending load",
      messageCount: 1,
      messages: [
        {
          id: "older-pending-user",
          conversationId: "conversation-overlap-older",
          role: "user",
          content: "Older completion must stay hidden",
          createdAt: "2026-07-11T12:00:30.000Z",
          status: "complete",
        },
      ],
    };
    const newerConversation: Conversation = {
      ...createdConversation,
      id: "conversation-overlap-newer",
      title: "Newer selected load",
      messageCount: 1,
      messages: [
        {
          id: "newer-selected-user",
          conversationId: "conversation-overlap-newer",
          role: "user",
          content: "Newer navigation remains visible",
          createdAt: "2026-07-11T12:01:30.000Z",
          status: "complete",
        },
      ],
    };
    const olderLoad = deferred<Conversation>();
    const newerLoad = deferred<Conversation>();
    desktopMocks.adapter.listConversations.mockResolvedValue([
      { ...olderConversation, messages: undefined },
      { ...newerConversation, messages: undefined },
    ]);
    desktopMocks.adapter.getConversation.mockImplementation((id: string) =>
      id === olderConversation.id ? olderLoad.promise : newerLoad.promise,
    );

    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: olderConversation.title }));
    expect(screen.getByLabelText("Loading conversation")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: newerConversation.title }));
    expect(desktopMocks.adapter.getConversation).toHaveBeenNthCalledWith(2, newerConversation.id);

    await act(async () => {
      newerLoad.resolve(newerConversation);
      await newerLoad.promise;
    });
    expect(await screen.findByText("Newer navigation remains visible")).toBeTruthy();
    expect(screen.queryByLabelText("Loading conversation")).toBeNull();

    await act(async () => {
      olderLoad.resolve(olderConversation);
      await olderLoad.promise;
    });
    expect(screen.getByText("Newer navigation remains visible")).toBeTruthy();
    expect(screen.queryByText("Older completion must stay hidden")).toBeNull();
    expect(screen.getByRole("heading", { name: newerConversation.title })).toBeTruthy();
  });

  it("lets a selected load finish while a background conversation reconciles", async () => {
    const sourceConversation: Conversation = {
      ...createdConversation,
      id: "conversation-background-source",
      title: "Background source",
    };
    const destinationConversation: Conversation = {
      ...createdConversation,
      id: "conversation-visible-destination",
      title: "Visible destination",
      messageCount: 1,
      messages: [
        {
          id: "visible-destination-user",
          conversationId: "conversation-visible-destination",
          role: "user",
          content: "Visible destination content",
          createdAt: "2026-07-11T12:04:00.000Z",
          status: "complete",
        },
      ],
    };
    const sourceReconciled: Conversation = {
      ...sourceConversation,
      messageCount: 2,
      messages: [
        {
          id: "background-source-user",
          conversationId: sourceConversation.id,
          role: "user",
          content: "Background prompt",
          createdAt: "2026-07-11T12:03:00.000Z",
          status: "complete",
        },
        {
          id: "background-source-assistant",
          conversationId: sourceConversation.id,
          role: "assistant",
          content: "Background answer",
          createdAt: "2026-07-11T12:03:30.000Z",
          status: "complete",
          finishReason: "stop",
        },
      ],
    };
    const destinationLoad = deferred<Conversation>();
    let sourceReads = 0;
    desktopMocks.adapter.listConversations.mockResolvedValue([
      { ...sourceConversation, messages: undefined },
      { ...destinationConversation, messages: undefined },
    ]);
    desktopMocks.adapter.getConversation.mockImplementation((id: string) => {
      if (id === destinationConversation.id) return destinationLoad.promise;
      sourceReads += 1;
      return Promise.resolve(sourceReads === 1 ? sourceConversation : sourceReconciled);
    });
    desktopMocks.adapter.providerStatuses.mockResolvedValue([
      {
        providerId: "zai",
        configured: true,
        reachability: "unknown",
        noticeVersion: 1,
        noticeAcknowledged: true,
      },
    ]);
    desktopMocks.adapter.sendMessage.mockResolvedValue({ requestId: "request-background" });

    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: sourceConversation.title }));
    await user.type(
      await screen.findByRole("textbox", { name: "Message Aster" }),
      "Background prompt{enter}",
    );
    await waitFor(() => {
      expect(desktopMocks.adapter.sendMessage).toHaveBeenCalledOnce();
    });

    await user.click(screen.getByRole("button", { name: destinationConversation.title }));
    expect(screen.getByLabelText("Loading conversation")).toBeTruthy();
    act(() => {
      const listener = desktopMocks.state.listener as
        ((event: ChatStreamEvent) => void) | undefined;
      listener?.({
        requestId: "request-background",
        conversationId: sourceConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 0,
        kind: "started",
      });
      listener?.({
        requestId: "request-background",
        conversationId: sourceConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 1,
        kind: "completed",
        message: sourceReconciled.messages[1],
      });
    });
    await waitFor(() => {
      expect(sourceReads).toBe(2);
    });

    await act(async () => {
      destinationLoad.resolve(destinationConversation);
      await destinationLoad.promise;
    });
    expect(await screen.findByText("Visible destination content")).toBeTruthy();
    expect(screen.queryByLabelText("Loading conversation")).toBeNull();
    expect(screen.queryByText("Background answer")).toBeNull();
  });

  it("does not let an older summary refresh erase a locally created conversation", async () => {
    const sourceConversation: Conversation = {
      ...createdConversation,
      id: "conversation-summary-source",
      title: "Summary source",
    };
    const sourceSummary = { ...sourceConversation, messages: undefined };
    const staleSummaryRefresh = deferred<(typeof sourceSummary)[]>();
    desktopMocks.adapter.listConversations
      .mockResolvedValueOnce([sourceSummary])
      .mockReturnValueOnce(staleSummaryRefresh.promise);
    desktopMocks.adapter.getConversation.mockResolvedValue(sourceConversation);
    desktopMocks.adapter.providerStatuses.mockResolvedValue([
      {
        providerId: "zai",
        configured: true,
        reachability: "unknown",
        noticeVersion: 1,
        noticeAcknowledged: true,
      },
    ]);
    desktopMocks.adapter.sendMessage.mockResolvedValue({ requestId: "request-summary-refresh" });

    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: sourceConversation.title }));
    await user.type(
      await screen.findByRole("textbox", { name: "Message Aster" }),
      "Keep the local summary{enter}",
    );
    await waitFor(() => {
      expect(desktopMocks.adapter.sendMessage).toHaveBeenCalledOnce();
    });
    act(() => {
      const listener = desktopMocks.state.listener as
        ((event: ChatStreamEvent) => void) | undefined;
      listener?.({
        requestId: "request-summary-refresh",
        conversationId: sourceConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 0,
        kind: "started",
      });
      listener?.({
        requestId: "request-summary-refresh",
        conversationId: sourceConversation.id,
        providerId: "zai",
        modelId: "glm-5.1",
        sequence: 1,
        kind: "completed",
        message: {
          id: "summary-source-assistant",
          conversationId: sourceConversation.id,
          role: "assistant",
          content: "Summary response",
          createdAt: "2026-07-11T12:05:00.000Z",
          status: "complete",
          finishReason: "stop",
        },
      });
    });
    await waitFor(() => {
      expect(desktopMocks.adapter.listConversations).toHaveBeenCalledTimes(2);
    });

    await user.click(screen.getByRole("button", { name: /New chat/i }));
    expect(await screen.findByRole("button", { name: "New conversation" })).toBeTruthy();

    await act(async () => {
      staleSummaryRefresh.resolve([sourceSummary]);
      await staleSummaryRefresh.promise;
    });
    expect(screen.getByRole("button", { name: "New conversation" })).toBeTruthy();
  });

  it("does not let a delayed new-chat completion overwrite a later navigation intent", async () => {
    const firstConversation: Conversation = {
      ...createdConversation,
      id: "conversation-create-source",
      title: "Create source",
    };
    const destinationConversation: Conversation = {
      ...createdConversation,
      id: "conversation-create-destination",
      title: "Create destination",
    };
    const delayedCreation = deferred<Conversation>();
    desktopMocks.adapter.listConversations.mockResolvedValue([
      { ...firstConversation, messages: undefined },
      { ...destinationConversation, messages: undefined },
    ]);
    desktopMocks.adapter.getConversation.mockImplementation((id: string) =>
      Promise.resolve(id === firstConversation.id ? firstConversation : destinationConversation),
    );
    desktopMocks.adapter.createConversation.mockReturnValue(delayedCreation.promise);

    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: firstConversation.title }));
    await user.click(screen.getByRole("button", { name: /New chat/i }));
    await waitFor(() => {
      expect(desktopMocks.adapter.createConversation).toHaveBeenCalledOnce();
    });

    await user.click(screen.getByRole("button", { name: destinationConversation.title }));
    const destinationDraft = await screen.findByRole<HTMLTextAreaElement>("textbox", {
      name: "Message Aster",
    });
    await user.type(destinationDraft, "Draft for the destination");

    await act(async () => {
      delayedCreation.resolve(createdConversation);
      await delayedCreation.promise;
    });
    expect(
      screen
        .getByRole("button", { name: destinationConversation.title })
        .getAttribute("aria-current"),
    ).toBe("page");
    expect(screen.getByRole("heading", { name: destinationConversation.title })).toBeTruthy();
    expect(screen.getByRole<HTMLTextAreaElement>("textbox", { name: "Message Aster" }).value).toBe(
      "Draft for the destination",
    );
    expect(screen.getByRole("button", { name: "New conversation" })).toBeTruthy();
  });

  it("preserves a home draft when creating a chat with a newly chosen model", async () => {
    desktopMocks.adapter.createConversation.mockResolvedValue({
      ...createdConversation,
      modelId: "glm-5.2",
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    const draft = screen.getByRole<HTMLTextAreaElement>("textbox", { name: "Message Aster" });
    await user.type(draft, "Keep this draft while choosing a model");
    await user.click(
      screen.getByRole("button", {
        name: /Choose model\. Current selection: Z\.AI .* GLM-5\.1/i,
      }),
    );
    await user.click(await screen.findByRole("radio", { name: /GLM-5\.2/i }));
    await user.click(screen.getByRole("button", { name: "Use this model" }));

    await waitFor(() => {
      expect(desktopMocks.adapter.createConversation).toHaveBeenCalledWith(undefined, {
        providerId: "zai",
        modelId: "glm-5.2",
      });
    });
    expect(screen.getByRole<HTMLTextAreaElement>("textbox", { name: "Message Aster" }).value).toBe(
      "Keep this draft while choosing a model",
    );
    expect(
      screen.getByRole("button", {
        name: /Choose model\. Current selection: Z\.AI .* GLM-5\.2/i,
      }),
    ).toBeTruthy();
  });

  it("ignores a stale empty-conversation model update after navigation", async () => {
    const emptyConversation: Conversation = {
      ...createdConversation,
      id: "conversation-empty-selection",
      title: "Empty selection conversation",
    };
    const destinationConversation: Conversation = {
      ...createdConversation,
      id: "conversation-destination",
      title: "Destination conversation",
      messageCount: 1,
      messages: [
        {
          id: "destination-user",
          conversationId: "conversation-destination",
          role: "user",
          content: "Destination remains authoritative",
          createdAt: "2026-07-11T12:02:00.000Z",
          status: "complete",
        },
      ],
    };
    const selectionUpdate = deferred<Conversation>();
    desktopMocks.adapter.listConversations.mockResolvedValue([
      { ...emptyConversation, messages: undefined },
      { ...destinationConversation, messages: undefined },
    ]);
    desktopMocks.adapter.getConversation.mockImplementation((id: string) =>
      Promise.resolve(id === emptyConversation.id ? emptyConversation : destinationConversation),
    );
    desktopMocks.adapter.updateConversationSelection.mockReturnValue(selectionUpdate.promise);

    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: emptyConversation.title }));
    await user.click(
      await screen.findByRole("button", {
        name: /Choose model\. Current selection: Z\.AI .* GLM-5\.1/i,
      }),
    );
    await user.click(await screen.findByRole("radio", { name: /GLM-5\.2/i }));
    await user.click(screen.getByRole("button", { name: "Use this model" }));
    await waitFor(() => {
      expect(desktopMocks.adapter.updateConversationSelection).toHaveBeenCalledWith(
        emptyConversation.id,
        "zai",
        "glm-5.2",
      );
    });
    expect(
      screen.getByRole<HTMLTextAreaElement>("textbox", { name: "Message Aster" }).disabled,
    ).toBe(true);
    expect(screen.getByRole<HTMLButtonElement>("button", { name: /New chat/i }).disabled).toBe(
      true,
    );

    await user.click(screen.getByRole("button", { name: destinationConversation.title }));
    expect(await screen.findByText("Destination remains authoritative")).toBeTruthy();

    await act(async () => {
      selectionUpdate.resolve({
        ...emptyConversation,
        modelId: "glm-5.2",
      });
      await selectionUpdate.promise;
    });
    expect(screen.getByText("Destination remains authoritative")).toBeTruthy();
    expect(screen.queryByText("GLM-5.2", { exact: true })).toBeNull();
  });

  it("suppresses global new-chat and settings shortcuts while the model picker is modal", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getByRole("button", { name: /New chat/i }));
    await user.click(
      screen.getByRole("button", {
        name: /Choose model\. Current selection: Z\.AI · GLM-5\.1/i,
      }),
    );
    expect(await screen.findByRole("dialog", { name: "Choose model" })).toBeTruthy();

    fireEvent.keyDown(window, { key: "n", ctrlKey: true });
    fireEvent.keyDown(window, { key: ",", ctrlKey: true });
    await Promise.resolve();

    expect(desktopMocks.adapter.createConversation).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("dialog", { name: "Choose model" })).toBeTruthy();
    expect(screen.queryByRole("dialog", { name: "Settings" })).toBeNull();
  });
});
