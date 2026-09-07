import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { DEMO_MODEL_CATALOG, DEMO_PROVIDER_STATUSES } from "../services/providerCatalog";
import { ProviderSettings } from "./ProviderSettings";

afterEach(cleanup);

describe("ProviderSettings", () => {
  it("never renders a credential field in browser demo", () => {
    const { container } = render(
      <ProviderSettings
        catalog={DEMO_MODEL_CATALOG}
        onPromptCredential={vi.fn()}
        onRequestRemove={vi.fn()}
        providerStatuses={DEMO_PROVIDER_STATUSES}
        runtime="browser-demo"
        workingProviderId={null}
      />,
    );

    expect(container.querySelectorAll("input")).toHaveLength(0);
    expect(screen.getAllByText("Demo only")).toHaveLength(5);
    expect(screen.queryByRole("button", { name: /API key/i })).toBeNull();
  });

  it("scopes native credential actions to the selected provider", async () => {
    const user = userEvent.setup();
    const onPromptCredential = vi.fn();
    const onRequestRemove = vi.fn();
    render(
      <ProviderSettings
        catalog={DEMO_MODEL_CATALOG}
        onPromptCredential={onPromptCredential}
        onRequestRemove={onRequestRemove}
        providerStatuses={DEMO_PROVIDER_STATUSES.map((status) => ({
          ...status,
          configured: status.providerId === "deepseek",
        }))}
        runtime="tauri"
        workingProviderId={null}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Replace DeepSeek API key" }));
    expect(onPromptCredential).toHaveBeenCalledWith("deepseek");
    await user.click(screen.getByRole("button", { name: "Remove DeepSeek API key" }));
    expect(onRequestRemove).toHaveBeenCalledWith("deepseek");
  });
});
