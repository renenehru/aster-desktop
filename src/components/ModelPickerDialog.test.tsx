import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { DEMO_MODEL_CATALOG, DEMO_PROVIDER_STATUSES } from "../services/providerCatalog";
import { ModelPickerDialog } from "./ModelPickerDialog";

afterEach(cleanup);

describe("ModelPickerDialog", () => {
  it("shows only the curated 17-model catalog and returns an exact provider/model pair", async () => {
    const user = userEvent.setup();
    const onChoose = vi.fn();
    render(
      <ModelPickerDialog
        catalog={DEMO_MODEL_CATALOG}
        currentSelection={DEMO_MODEL_CATALOG.defaultSelection}
        onChoose={onChoose}
        onClose={vi.fn()}
        providerStatuses={DEMO_PROVIDER_STATUSES}
      />,
    );

    expect(screen.getAllByRole("radio")).toHaveLength(17);
    expect(screen.queryByText(/unavailable/i)).toBeNull();
    await user.type(screen.getByRole("searchbox", { name: "Search catalog models" }), "DeepSeek");
    expect(screen.getAllByRole("radio")).toHaveLength(2);
    await user.click(screen.getByText("DeepSeek V4 Pro"));
    await user.click(screen.getByRole("button", { name: "Use this model" }));

    expect(onChoose).toHaveBeenCalledWith({
      providerId: "deepseek",
      modelId: "deepseek-v4-pro",
    });
  });

  it("describes credential state without claiming provider reachability", () => {
    render(
      <ModelPickerDialog
        catalog={DEMO_MODEL_CATALOG}
        currentSelection={DEMO_MODEL_CATALOG.defaultSelection}
        onChoose={vi.fn()}
        onClose={vi.fn()}
        providerStatuses={DEMO_PROVIDER_STATUSES.map((status) => ({
          ...status,
          configured: true,
        }))}
      />,
    );

    expect(screen.getAllByText("Key configured")).toHaveLength(5);
    expect(screen.queryByText("Connected")).toBeNull();
  });

  it("does not apply a previously selected model after search hides it", async () => {
    const user = userEvent.setup();
    const onChoose = vi.fn();
    render(
      <ModelPickerDialog
        catalog={DEMO_MODEL_CATALOG}
        currentSelection={DEMO_MODEL_CATALOG.defaultSelection}
        onChoose={onChoose}
        onClose={vi.fn()}
        providerStatuses={DEMO_PROVIDER_STATUSES}
      />,
    );

    const search = screen.getByRole("searchbox", { name: "Search catalog models" });
    await user.type(search, "DeepSeek");
    await user.click(screen.getByText("DeepSeek V4 Pro"));
    await user.clear(search);
    await user.type(search, "Gemini");

    const apply = screen.getByRole<HTMLButtonElement>("button", { name: "Use this model" });
    expect(apply.disabled).toBe(true);
    await user.click(apply);
    expect(onChoose).not.toHaveBeenCalled();
  });
});
