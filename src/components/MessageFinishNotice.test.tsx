import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import type { FinishReason } from "../types/chat";
import { MessageFinishNotice } from "./MessageFinishNotice";

afterEach(cleanup);

describe("MessageFinishNotice", () => {
  it("renders a persistent keyboard-readable non-error notice for outputLimit", () => {
    render(<MessageFinishNotice finishReason="outputLimit" />);

    const notice = screen.getByRole("note", { name: "Incomplete response notice" });
    expect(notice.textContent).toBe(
      "The provider reached Aster's output limit. This response may be incomplete. This is not a provider error.",
    );
    expect(notice.getAttribute("tabindex")).toBe("0");
    expect(notice.querySelector("svg[aria-hidden='true']")).not.toBeNull();
  });

  it.each(["stop", "unknown", undefined] satisfies Array<FinishReason | undefined>)(
    "does not render the incomplete-response notice for %s",
    (finishReason) => {
      render(<MessageFinishNotice finishReason={finishReason} />);

      expect(screen.queryByRole("note", { name: "Incomplete response notice" })).toBeNull();
    },
  );
});
