import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const nativeWindowMocks = vi.hoisted(() => ({
  close: vi.fn(),
  getCurrentWindow: vi.fn(),
  minimize: vi.fn(),
  toggleMaximize: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: nativeWindowMocks.getCurrentWindow,
}));

import App from "./App";

const writeText = vi.fn<(content: string) => Promise<void>>();

beforeEach(() => {
  writeText.mockReset();
  writeText.mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText },
  });
  nativeWindowMocks.getCurrentWindow.mockReset();
  nativeWindowMocks.getCurrentWindow.mockReturnValue({
    close: nativeWindowMocks.close,
    minimize: nativeWindowMocks.minimize,
    toggleMaximize: nativeWindowMocks.toggleMaximize,
  });
});

afterEach(cleanup);

describe("Aster browser demo", () => {
  it("is visibly isolated and never renders credential input", async () => {
    const { container } = render(<App />);

    expect(await screen.findByText("Browser demo")).toBeTruthy();
    expect(screen.getByText(/no provider network · no API keys/i)).toBeTruthy();
    await screen.findByText("Threat model review");

    fireEvent.click(screen.getAllByRole("button", { name: /settings/i })[0] as HTMLElement);
    expect(await screen.findByRole("dialog", { name: "Settings" })).toBeTruthy();
    expect(screen.getByText(/Synthetic provider catalog/i)).toBeTruthy();
    expect(screen.getAllByText(/No key can be entered, stored, or transmitted/i)).toHaveLength(5);
    expect(screen.getAllByText("Demo only").length).toBeGreaterThan(0);
    expect(screen.getByText("Aster v0.2.0-preview")).toBeTruthy();
    expect(screen.queryByLabelText(/^API key$/i)).toBeNull();
    expect(container.querySelectorAll("#provider-settings input")).toHaveLength(0);
    expect(
      screen.getByRole<HTMLButtonElement>("button", { name: "Minimize window" }).disabled,
    ).toBe(true);
    expect(
      screen.getByRole<HTMLButtonElement>("button", { name: "Maximize or restore window" })
        .disabled,
    ).toBe(true);
    expect(screen.getByRole<HTMLButtonElement>("button", { name: "Close window" }).disabled).toBe(
      true,
    );
    expect(nativeWindowMocks.getCurrentWindow).not.toHaveBeenCalled();
    expect(container.querySelector(".titlebar")?.hasAttribute("data-tauri-drag-region")).toBe(true);
  });

  it("supports title search, chat creation, keyboard send, and stop", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Threat model review");

    await user.click(screen.getByRole("button", { name: /Search titles/i }));
    await user.type(screen.getByRole("textbox", { name: "Search conversation titles" }), "Rust");
    expect(screen.getByText("Rust streaming notes")).toBeTruthy();
    expect(screen.queryByText("Threat model review")).toBeNull();

    await user.click(screen.getByRole("button", { name: /New chat/i }));
    const composer = await screen.findByRole("textbox", { name: "Message Aster" });
    await user.type(composer, "Review this security incident{enter}");

    expect((await screen.findAllByText("Review this security incident")).length).toBeGreaterThan(0);
    const stop = await screen.findByRole("button", { name: "Stop generation" });
    await user.click(stop);

    await waitFor(() => {
      expect(screen.getByText("Stopped")).toBeTruthy();
    });
    expect(screen.getByText(/no messages leave this tab/i)).toBeTruthy();
  });

  it("loads a conversation, copies plain text, enters edit mode, and regenerates", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Threat model review" }));
    expect(await screen.findByRole("heading", { name: "MVP security checklist" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Copy response" }));
    await screen.findByText("Response copied as plain text");

    await user.click(screen.getByRole("button", { name: "Edit and resend this message" }));
    expect(screen.getByText(/Editing an earlier message/i)).toBeTruthy();
    expect(
      screen.getByRole<HTMLTextAreaElement>("textbox", { name: "Edit message" }).value,
    ).toContain("threat-model checklist");
    await user.click(screen.getByRole("button", { name: "Cancel edit" }));
    expect(screen.queryByText(/Editing an earlier message/i)).toBeNull();

    await user.click(screen.getByRole("button", { name: "Regenerate latest response" }));
    expect(await screen.findByRole("button", { name: "Stop generation" })).toBeTruthy();
    await waitFor(
      () => {
        expect(screen.getByRole("button", { name: "Regenerate latest response" })).toBeTruthy();
      },
      { timeout: 10_000 },
    );
  }, 12_000);

  it("renames and deliberately deletes a local conversation", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Rust streaming notes");
    await user.click(screen.getByRole("button", { name: "Actions for Rust streaming notes" }));
    await user.click(screen.getByRole("button", { name: "Rename" }));

    const title = screen.getByRole("textbox", { name: "Conversation title" });
    await user.clear(title);
    await user.type(title, "Streaming contract notes");
    await user.click(screen.getByRole("button", { name: "Save title" }));
    expect(await screen.findByText("Streaming contract notes")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Actions for Streaming contract notes" }));
    await user.click(screen.getByRole("button", { name: "Delete" }));
    expect(await screen.findByRole("dialog", { name: "Delete conversation?" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Keep conversation" }));
    expect(screen.getByText("Streaming contract notes")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Actions for Streaming contract notes" }));
    await user.click(screen.getByRole("button", { name: "Delete" }));
    await user.click(screen.getByRole("button", { name: "Delete permanently" }));
    await waitFor(() => {
      expect(screen.queryByText("Streaming contract notes")).toBeNull();
    });
  });

  it("supports shortcuts, empty search feedback, and dismissible local notice", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Threat model review");

    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    const search = await screen.findByRole("textbox", { name: "Search conversation titles" });
    await user.type(search, "does not exist");
    expect(screen.getByText("No matching titles")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Clear search" }));

    await user.click(screen.getByRole("button", { name: "Dismiss local data notice" }));
    expect(screen.queryByText("Local by design")).toBeNull();

    fireEvent.keyDown(window, { key: ",", ctrlKey: true });
    expect(await screen.findByRole("dialog", { name: "Settings" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Close Settings" }));

    fireEvent.keyDown(window, { key: "n", ctrlKey: true });
    expect(await screen.findByText("Start with a question")).toBeTruthy();
  });

  it("starts a new chat when a populated conversation changes model and opens advisory Usage", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Threat model review" }));

    await user.click(
      screen.getByRole("button", {
        name: /Choose model\. Current selection: Z\.AI · GLM-5\.1/i,
      }),
    );
    expect(await screen.findByRole("dialog", { name: "Choose model" })).toBeTruthy();
    await user.type(screen.getByRole("searchbox", { name: "Search catalog models" }), "V4 Pro");
    await user.click(screen.getByText("DeepSeek V4 Pro"));
    await user.click(screen.getByRole("button", { name: "Use this model" }));

    expect(await screen.findByRole("dialog", { name: "Start a new chat?" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Start a new chat with this model" }));
    expect(
      await screen.findByRole("button", {
        name: /Current selection: DeepSeek · DeepSeek V4 Pro/i,
      }),
    ).toBeTruthy();

    await user.click(screen.getByRole("button", { name: /Usage7 days/i }));
    expect(await screen.findByRole("dialog", { name: "Usage" })).toBeTruthy();
    expect(await screen.findByText("Locally observed usage")).toBeTruthy();
    expect(screen.getByText(/Synthetic demo data/i)).toBeTruthy();
  });
});
