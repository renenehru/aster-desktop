import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { Dialog } from "./Dialog";

afterEach(cleanup);

describe("Dialog", () => {
  it("focuses its first control, exposes its description, and closes on Escape", () => {
    const onClose = vi.fn();
    render(
      <Dialog label="Confirm action" description="This action has a consequence." onClose={onClose}>
        <button type="button">Keep item</button>
        <button type="button">Delete item</button>
      </Dialog>,
    );

    const dialog = screen.getByRole("dialog", { name: "Confirm action" });
    expect(dialog.getAttribute("aria-describedby")).toBeTruthy();
    expect(document.activeElement).toBe(
      screen.getByRole("button", { name: "Close Confirm action" }),
    );
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("traps forward and reverse tab focus within the modal", () => {
    render(
      <Dialog label="Keyboard dialog" onClose={() => undefined}>
        <button type="button">First action</button>
        <button type="button">Last action</button>
      </Dialog>,
    );
    const close = screen.getByRole("button", { name: "Close Keyboard dialog" });
    const last = screen.getByRole("button", { name: "Last action" });

    last.focus();
    fireEvent.keyDown(document, { key: "Tab" });
    expect(document.activeElement).toBe(close);

    close.focus();
    fireEvent.keyDown(document, { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(last);
  });

  it("closes only when the backdrop itself is clicked and restores prior focus", () => {
    const onClose = vi.fn();
    const opener = document.createElement("button");
    document.body.append(opener);
    opener.focus();
    const { container, unmount } = render(
      <Dialog label="Backdrop dialog" onClose={onClose}>
        <button type="button">Inside</button>
      </Dialog>,
    );

    fireEvent.mouseDown(screen.getByRole("dialog", { name: "Backdrop dialog" }));
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.mouseDown(container.querySelector(".dialog-backdrop") as Element);
    expect(onClose).toHaveBeenCalledOnce();
    unmount();
    expect(document.activeElement).toBe(opener);
    opener.remove();
  });
});
