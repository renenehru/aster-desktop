import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { Markdown } from "./Markdown";

const writeText = vi.fn<(value: string) => Promise<void>>();

beforeEach(() => {
  writeText.mockReset();
  writeText.mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText },
  });
});

afterEach(cleanup);

describe("Markdown", () => {
  it("renders useful Markdown without interpreting raw HTML or unsafe links", () => {
    const onOpenLink = vi.fn();
    const { container } = render(
      <Markdown
        content={[
          "## Safe response",
          "",
          "<img src=x onerror=alert(1)><script>alert(1)</script>",
          "",
          "[Unsafe](javascript:alert(1)) and [Documentation](https://example.com/guide)",
          "",
          "| Control | State |",
          "| --- | --- |",
          "| HTML | Disabled |",
        ].join("\n")}
        onOpenLink={onOpenLink}
      />,
    );

    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("img")).toBeNull();
    expect(screen.queryByRole("button", { name: /Unsafe/i })).toBeNull();
    const safeLink = screen.getByRole("button", { name: /opens example\.com/i });
    expect(safeLink.hasAttribute("href")).toBe(false);
    fireEvent.click(safeLink);
    expect(onOpenLink).toHaveBeenCalledWith("https://example.com/guide");
    expect(screen.getByText("example.com")).toBeTruthy();
    expect(screen.getByRole("table")).toBeTruthy();
  });

  it("copies fenced code as plain text and reports success", async () => {
    render(<Markdown content={"```typescript\nconst safe = true;\n```"} />);

    fireEvent.click(screen.getByRole("button", { name: "Copy code" }));

    await screen.findByText("Copied");
    expect(writeText).toHaveBeenCalledOnce();
    expect(writeText).toHaveBeenCalledWith("const safe = true;");
  });

  it("shows non-disruptive feedback when clipboard access fails", async () => {
    writeText.mockRejectedValueOnce(new Error("Clipboard unavailable"));
    render(<Markdown content={"```text\nplain text\n```"} />);

    fireEvent.click(screen.getByRole("button", { name: "Copy code" }));

    expect(await screen.findByText("Copy failed")).toBeTruthy();
  });

  it("renders headings, quotes, ordered and unordered lists, rules, and inline styles", () => {
    const { container } = render(
      <Markdown
        content={[
          "# Heading one",
          "### Heading three",
          "",
          "> Quoted **guidance**",
          "",
          "1. First",
          "2. Second with `code`",
          "",
          "- Alpha",
          "- *Beta*",
          "",
          "---",
          "",
          "A paragraph that",
          "continues safely.",
        ].join("\n")}
      />,
    );

    expect(screen.getByRole("heading", { level: 1, name: "Heading one" })).toBeTruthy();
    expect(screen.getByRole("heading", { level: 3, name: "Heading three" })).toBeTruthy();
    expect(container.querySelector("blockquote")?.textContent).toContain("Quoted guidance");
    expect(container.querySelector("ol")?.children).toHaveLength(2);
    expect(container.querySelector("ul")?.children).toHaveLength(2);
    expect(container.querySelector("hr")).toBeTruthy();
    expect(container.querySelector("strong")?.textContent).toBe("guidance");
    expect(container.querySelector("em")?.textContent).toBe("Beta");
    expect(container.querySelector(".inline-code")?.textContent).toBe("code");
    expect(screen.getByText("A paragraph that continues safely.")).toBeTruthy();
  });

  it("keeps incomplete fences readable, sanitizes language labels, and marks streaming output", () => {
    const { container } = render(
      <Markdown content={"```type<script>\nconst value = '<safe>';"} streaming />,
    );

    expect(screen.getByLabelText("typescript code block")).toBeTruthy();
    expect(screen.getByText("const value = '<safe>';")).toBeTruthy();
    expect(container.querySelector(".stream-caret")).toBeTruthy();
    expect(container.querySelector("script")).toBeNull();
  });

  it("calls the optional copy callback after a successful code copy", async () => {
    const onCopied = vi.fn();
    render(<Markdown content={"```\ncopy me\n```"} onCopied={onCopied} />);
    fireEvent.click(screen.getByRole("button", { name: "Copy code" }));
    await screen.findByText("Copied");
    expect(onCopied).toHaveBeenCalledOnce();
    expect(screen.getByLabelText("Plain text code block")).toBeTruthy();
  });
});
