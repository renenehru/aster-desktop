import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
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
      credentialStatus: vi.fn(),
      promptStoreApiKey: vi.fn(),
      deleteApiKey: vi.fn(),
      listConversations: vi.fn(),
      getConversation: vi.fn(),
      createConversation: vi.fn(),
      renameConversation: vi.fn(),
      deleteConversation: vi.fn(),
      sendMessage: vi.fn(),
      cancelGeneration: vi.fn(),
      onChatStream: vi.fn(),
      acknowledgeExternalProcessing: vi.fn(),
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
  model: "glm-5.1",
  reasoningMode: "standard",
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
    desktopMocks.adapter.credentialStatus.mockResolvedValue({
      configured: true,
      source: "credential-vault",
    });
    desktopMocks.adapter.listConversations.mockResolvedValue([]);
    desktopMocks.adapter.createConversation.mockResolvedValue(createdConversation);
    desktopMocks.adapter.onChatStream.mockImplementation(
      (listener: (event: ChatStreamEvent) => void) => {
        desktopMocks.state.listener = listener as (event: unknown) => void;
        return Promise.resolve(() => undefined);
      },
    );
    desktopMocks.adapter.acknowledgeExternalProcessing.mockResolvedValue(undefined);
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
    desktopMocks.adapter.sendMessage.mockImplementationOnce(() => {
      const listener = desktopMocks.state.listener as
        ((event: ChatStreamEvent) => void) | undefined;
      listener?.({
        requestId: "request-1",
        conversationId: createdConversation.id,
        sequence: 0,
        kind: "started",
      });
      listener?.({
        requestId: "request-1",
        conversationId: createdConversation.id,
        sequence: 1,
        kind: "completed",
        message: {
          id: "assistant-authoritative",
          conversationId: createdConversation.id,
          role: "assistant",
          content: "Authoritative answer",
          createdAt: "2026-07-11T12:01:00.000Z",
          status: "complete",
        },
      });
      return Promise.resolve({ requestId: "request-1" });
    });
    desktopMocks.adapter.sendMessage.mockResolvedValueOnce({ requestId: "request-2" });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getByRole("button", { name: /New chat/i }));
    await user.type(
      screen.getByRole("textbox", { name: "Message Aster" }),
      "Original prompt{enter}",
    );
    await user.click(await screen.findByRole("button", { name: "Continue and send" }));

    expect(
      (await screen.findByRole<HTMLButtonElement>("button", { name: /Edit and resend/i })).disabled,
    ).toBe(true);
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
        },
      ],
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
        sequence: 0,
        kind: "started",
      });
      listener?.({
        requestId: "request-error",
        conversationId: createdConversation.id,
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
        sequence: 0,
        kind: "started",
      });
      listener?.({
        requestId: "request-db-error",
        conversationId: createdConversation.id,
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
    await user.click(await screen.findByRole("button", { name: "Replace API key" }));
    await waitFor(() => {
      expect(desktopMocks.adapter.promptStoreApiKey).toHaveBeenCalledWith();
    });
    await user.click(await screen.findByRole("button", { name: "Remove saved key" }));
    expect(await screen.findByRole("dialog", { name: "Remove saved API key?" })).toBeTruthy();
    expect(desktopMocks.adapter.deleteApiKey).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Keep saved key" }));
    expect(await screen.findByRole("dialog", { name: "Settings" })).toBeTruthy();
    expect(desktopMocks.adapter.deleteApiKey).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Remove saved key" }));
    await user.click(await screen.findByRole("button", { name: "Remove key" }));
    await waitFor(() => {
      expect(desktopMocks.adapter.deleteApiKey).toHaveBeenCalledOnce();
    });
  });

  it("keeps API keys out of desktop DOM and opens the native Add prompt without arguments", async () => {
    desktopMocks.adapter.credentialStatus.mockResolvedValue({ configured: false, source: "none" });
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getAllByRole("button", { name: /settings/i })[0] as HTMLElement);

    const settings = await screen.findByRole("dialog", { name: "Settings" });
    expect(settings.querySelectorAll("input")).toHaveLength(0);
    await user.click(screen.getByRole("button", { name: "Add API key" }));

    await waitFor(() => {
      expect(desktopMocks.adapter.promptStoreApiKey).toHaveBeenCalledOnce();
    });
    expect(desktopMocks.adapter.promptStoreApiKey.mock.calls[0]).toEqual([]);
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
    desktopMocks.adapter.sendMessage.mockResolvedValue({ requestId: "request-probe" });
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("No conversations yet");
    await user.click(screen.getByRole("button", { name: /New chat/i }));

    const composer = screen.getByRole<HTMLTextAreaElement>("textbox", { name: "Message Aster" });
    expect(composer.placeholder).toBe("Z.AI is currently unreachable");
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
    await user.click(await screen.findByRole("button", { name: "Replace API key" }));

    expect(await screen.findByText("API key setup cancelled")).toBeTruthy();
    expect(screen.getByText("Key configured")).toBeTruthy();
    expect(screen.queryByText(/saved in Windows Credential Manager/i)).toBeNull();
    expect(desktopMocks.adapter.promptStoreApiKey).toHaveBeenCalledWith();
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
        sequence: 0,
        kind: "started",
      });
      listener?.({
        requestId: "request-gap",
        conversationId: createdConversation.id,
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
});
