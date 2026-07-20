import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { DEMO_MODEL_CATALOG, DEMO_PROVIDER_STATUSES } from "../services/providerCatalog";
import type { DeepSeekBalance, UsageSummary } from "../types/providers";
import { UsageDialog } from "./UsageDialog";

const usage: UsageSummary = {
  providerId: "zai",
  modelId: null,
  windowStart: "2026-07-06T12:00:00.000Z",
  windowEnd: "2026-07-13T12:00:00.000Z",
  observedAt: "2026-07-13T12:00:00.000Z",
  usage: {
    inputTokens: 20_000,
    cachedInputTokens: null,
    outputTokens: 31_000,
    totalTokens: 51_000,
  },
  completeObservations: 3,
  partialObservations: 1,
  coverage: "partial",
  budget: {
    tokenBudget: 100_000,
    knownUsedTokens: 51_000,
    remainingTokens: 49_000,
    remainingPercentage: 49,
    state: "normal",
  },
};

function deferred<T>() {
  let resolvePromise: (value: T) => void = () => undefined;
  let rejectPromise: (reason?: unknown) => void = () => undefined;
  const promise = new Promise<T>((resolve, reject) => {
    resolvePromise = resolve;
    rejectPromise = reject;
  });
  return { promise, reject: rejectPromise, resolve: resolvePromise };
}

function currentBalance(totalBalance: string): DeepSeekBalance {
  return {
    status: "current",
    observedAt: "2026-07-13T12:00:00.000Z",
    isAvailable: true,
    balanceInfos: [
      {
        currency: "USD",
        totalBalance,
        grantedBalance: "4.00",
        toppedUpBalance: "15.25",
      },
    ],
    error: null,
  };
}

afterEach(cleanup);

describe("UsageDialog", () => {
  it("labels local usage as advisory and visualizes the percentage remaining", async () => {
    const { container } = render(
      <UsageDialog
        catalog={DEMO_MODEL_CATALOG}
        initialProviderId="zai"
        loadDeepSeekBalance={() =>
          Promise.resolve({
            status: "notChecked",
            observedAt: null,
            isAvailable: null,
            balanceInfos: [],
            error: null,
          })
        }
        loadUsage={() => Promise.resolve(usage)}
        onClose={vi.fn()}
        providerStatuses={DEMO_PROVIDER_STATUSES}
        refreshDeepSeekBalance={vi.fn()}
        runtime="browser-demo"
        setBudget={() => Promise.resolve(usage)}
      />,
    );

    expect(await screen.findByText("Locally observed usage")).toBeTruthy();
    expect(screen.getByText(/advisory data/i)).toBeTruthy();
    expect(screen.getByText(/not credit or billing data/i)).toBeTruthy();
    const progress = container.querySelector<HTMLProgressElement>("progress");
    expect(progress?.value).toBe(49);
    expect(progress?.getAttribute("aria-label")).toContain("49 percent");
    expect(screen.queryByRole("button", { name: /on Z\.AI/i })).toBeNull();
  });

  it("keeps an overflowed budget visible and explains the unavailable exact total", async () => {
    const overflowed: UsageSummary = {
      ...usage,
      coverage: "partial",
      usage: {
        inputTokens: null,
        cachedInputTokens: null,
        outputTokens: null,
        totalTokens: null,
      },
      budget: {
        tokenBudget: 100_000,
        knownUsedTokens: null,
        remainingTokens: 0,
        remainingPercentage: 0,
        state: "exhausted",
      },
    };
    render(
      <UsageDialog
        catalog={DEMO_MODEL_CATALOG}
        initialProviderId="zai"
        loadDeepSeekBalance={() => Promise.resolve(null)}
        loadUsage={() => Promise.resolve(overflowed)}
        onClose={vi.fn()}
        providerStatuses={DEMO_PROVIDER_STATUSES}
        refreshDeepSeekBalance={vi.fn()}
        runtime="tauri"
        setBudget={() => Promise.resolve(overflowed)}
      />,
    );

    expect(
      await screen.findByText("Exact known usage exceeds the supported display range"),
    ).toBeTruthy();
    expect(screen.getByText("Critical · 0% remaining")).toBeTruthy();
    expect(screen.queryByText(/No local token budget is set/)).toBeNull();
  });

  it("rejects a local usage response outside the requested provider and model scope", async () => {
    render(
      <UsageDialog
        catalog={DEMO_MODEL_CATALOG}
        initialProviderId="zai"
        loadDeepSeekBalance={() => Promise.resolve(null)}
        loadUsage={() => Promise.resolve({ ...usage, providerId: "deepseek" })}
        onClose={vi.fn()}
        providerStatuses={DEMO_PROVIDER_STATUSES}
        refreshDeepSeekBalance={vi.fn()}
        runtime="tauri"
        setBudget={() => Promise.resolve(usage)}
      />,
    );

    expect(await screen.findByText(/outside the requested scope/i)).toBeTruthy();
    expect(screen.queryByText("49% remaining")).toBeNull();
  });

  it("ignores an old usage result immediately after the provider scope changes", async () => {
    const user = userEvent.setup();
    const oldUsage = deferred<UsageSummary>();
    const deepSeekUsage: UsageSummary = {
      ...usage,
      providerId: "deepseek",
      usage: {
        inputTokens: 10,
        cachedInputTokens: 0,
        outputTokens: 10,
        totalTokens: 20,
      },
      budget: null,
    };
    render(
      <UsageDialog
        catalog={DEMO_MODEL_CATALOG}
        initialProviderId="zai"
        loadDeepSeekBalance={() => Promise.resolve(null)}
        loadUsage={(providerId) =>
          providerId === "zai" ? oldUsage.promise : Promise.resolve(deepSeekUsage)
        }
        onClose={vi.fn()}
        providerStatuses={DEMO_PROVIDER_STATUSES}
        refreshDeepSeekBalance={vi.fn()}
        runtime="tauri"
        setBudget={() => Promise.resolve(usage)}
      />,
    );

    await user.selectOptions(screen.getByRole("combobox", { name: "Usage provider" }), "deepseek");
    await act(async () => {
      oldUsage.resolve(usage);
      await oldUsage.promise;
    });

    expect(await screen.findByText("No local token budget is set for DeepSeek.")).toBeTruthy();
    expect(screen.queryByText("49% remaining")).toBeNull();
  });

  it("does not round an exact noncritical 10.04 percent to a misleading 10 percent", async () => {
    const nearThreshold: UsageSummary = {
      ...usage,
      budget: {
        tokenBudget: 100_000,
        knownUsedTokens: 89_960,
        remainingTokens: 10_040,
        remainingPercentage: 10.04,
        state: "normal",
      },
    };
    const { container } = render(
      <UsageDialog
        catalog={DEMO_MODEL_CATALOG}
        initialProviderId="zai"
        loadDeepSeekBalance={() => Promise.resolve(null)}
        loadUsage={() => Promise.resolve(nearThreshold)}
        onClose={vi.fn()}
        providerStatuses={DEMO_PROVIDER_STATUSES}
        refreshDeepSeekBalance={vi.fn()}
        runtime="tauri"
        setBudget={() => Promise.resolve(nearThreshold)}
      />,
    );

    expect(await screen.findByText(">10% remaining")).toBeTruthy();
    const card = screen.getByText("Advisory 7-day token budget").closest("section");
    expect(card?.classList.contains("critical")).toBe(false);
    const progress = container.querySelector<HTMLProgressElement>("progress");
    expect(progress?.value).toBe(10.04);
    expect(progress?.getAttribute("aria-label")).toContain("More than 10 percent");
    expect(screen.queryByText("10% remaining")).toBeNull();
  });

  it("uses the exact normal state when a maximum-safe percentage rounds to 10", async () => {
    const tokenBudget = 9_007_199_254_740_989;
    const remainingTokens = 900_719_925_474_099;
    const maxSafeNormal: UsageSummary = {
      ...usage,
      budget: {
        tokenBudget,
        knownUsedTokens: 8_106_479_329_266_890,
        remainingTokens,
        remainingPercentage: (remainingTokens / tokenBudget) * 100,
        state: "normal",
      },
    };
    const { container } = render(
      <UsageDialog
        catalog={DEMO_MODEL_CATALOG}
        initialProviderId="zai"
        loadDeepSeekBalance={() => Promise.resolve(null)}
        loadUsage={() => Promise.resolve(maxSafeNormal)}
        onClose={vi.fn()}
        providerStatuses={DEMO_PROVIDER_STATUSES}
        refreshDeepSeekBalance={vi.fn()}
        runtime="tauri"
        setBudget={() => Promise.resolve(maxSafeNormal)}
      />,
    );

    expect(await screen.findByText(">10% remaining")).toBeTruthy();
    const progress = container.querySelector<HTMLProgressElement>("progress");
    expect(progress?.getAttribute("aria-label")).toContain("More than 10 percent");
    expect(screen.queryByText("10% remaining")).toBeNull();
    expect(screen.queryByLabelText("Critical advisory token budget")).toBeNull();
  });

  it("renders a redundant critical alert when 10 percent or less remains", async () => {
    const criticalUsage: UsageSummary = {
      ...usage,
      budget: {
        tokenBudget: 100_000,
        knownUsedTokens: 90_000,
        remainingTokens: 10_000,
        remainingPercentage: 10,
        state: "low",
      },
    };
    const { container } = render(
      <UsageDialog
        catalog={DEMO_MODEL_CATALOG}
        initialProviderId="zai"
        loadDeepSeekBalance={() =>
          Promise.resolve({
            status: "notChecked",
            observedAt: null,
            isAvailable: null,
            balanceInfos: [],
            error: null,
          })
        }
        loadUsage={() => Promise.resolve(criticalUsage)}
        onClose={vi.fn()}
        providerStatuses={DEMO_PROVIDER_STATUSES}
        refreshDeepSeekBalance={vi.fn()}
        runtime="browser-demo"
        setBudget={() => Promise.resolve(criticalUsage)}
      />,
    );

    expect(await screen.findByText("Critical · 10% remaining")).toBeTruthy();
    const card = screen.getByText("Advisory 7-day token budget").closest("section");
    expect(card?.classList.contains("critical")).toBe(true);
    const progress = container.querySelector<HTMLProgressElement>("progress");
    expect(progress?.value).toBe(10);
    expect(progress?.getAttribute("aria-label")).toMatch(/^Critical:/);
    const alert = screen.getByRole("status", { name: "Critical advisory token budget" });
    expect(alert.textContent).toContain("Low advisory budget: 10% or less remains");
    expect(alert.querySelector("svg[aria-hidden='true']")).not.toBeNull();
  });

  it("retrieves exact DeepSeek balance only after an explicit refresh action", async () => {
    const user = userEvent.setup();
    const refreshBalance = vi.fn().mockResolvedValue({
      status: "current",
      observedAt: "2026-07-13T12:00:00.000Z",
      isAvailable: true,
      balanceInfos: [
        {
          currency: "USD",
          totalBalance: "19.25",
          grantedBalance: "4.00",
          toppedUpBalance: "15.25",
        },
      ],
      error: null,
    });
    render(
      <UsageDialog
        catalog={DEMO_MODEL_CATALOG}
        initialProviderId="deepseek"
        loadDeepSeekBalance={() =>
          Promise.resolve({
            status: "notChecked",
            observedAt: null,
            isAvailable: null,
            balanceInfos: [],
            error: null,
          })
        }
        loadUsage={(providerId) => {
          return Promise.resolve({ ...usage, providerId });
        }}
        onClose={vi.fn()}
        providerStatuses={DEMO_PROVIDER_STATUSES}
        refreshDeepSeekBalance={refreshBalance}
        runtime="browser-demo"
        setBudget={() => Promise.resolve(usage)}
      />,
    );

    expect(await screen.findByText("Not checked in this session.")).toBeTruthy();
    expect(refreshBalance).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Refresh synthetic balance" }));
    await waitFor(() => {
      expect(refreshBalance).toHaveBeenCalledOnce();
    });
    expect(await screen.findByText("19.25")).toBeTruthy();
  });

  it("keeps a newer explicit balance refresh when the older session load later rejects", async () => {
    const user = userEvent.setup();
    const sessionLoad = deferred<DeepSeekBalance | null>();
    const explicitRefresh = deferred<DeepSeekBalance>();
    render(
      <UsageDialog
        catalog={DEMO_MODEL_CATALOG}
        initialProviderId="deepseek"
        loadDeepSeekBalance={() => sessionLoad.promise}
        loadUsage={() => Promise.resolve({ ...usage, providerId: "deepseek" })}
        onClose={vi.fn()}
        providerStatuses={DEMO_PROVIDER_STATUSES}
        refreshDeepSeekBalance={() => explicitRefresh.promise}
        runtime="tauri"
        setBudget={() => Promise.resolve({ ...usage, providerId: "deepseek" })}
      />,
    );

    await user.click(await screen.findByRole("button", { name: "Refresh exact balance" }));
    await act(async () => {
      explicitRefresh.resolve(currentBalance("27.50"));
      await explicitRefresh.promise;
    });
    expect(await screen.findByText("27.50")).toBeTruthy();

    await act(async () => {
      sessionLoad.reject(new Error("Older session load failed."));
      await sessionLoad.promise.catch(() => undefined);
    });
    expect(screen.getByText("27.50")).toBeTruthy();
    expect(screen.queryByText("Older session load failed.")).toBeNull();
    expect(screen.queryByText("Exact balance could not be retrieved")).toBeNull();
  });

  it("keeps a newer explicit balance error when the older session load later resolves", async () => {
    const user = userEvent.setup();
    const sessionLoad = deferred<DeepSeekBalance | null>();
    const explicitRefresh = deferred<DeepSeekBalance>();
    render(
      <UsageDialog
        catalog={DEMO_MODEL_CATALOG}
        initialProviderId="deepseek"
        loadDeepSeekBalance={() => sessionLoad.promise}
        loadUsage={() => Promise.resolve({ ...usage, providerId: "deepseek" })}
        onClose={vi.fn()}
        providerStatuses={DEMO_PROVIDER_STATUSES}
        refreshDeepSeekBalance={() => explicitRefresh.promise}
        runtime="tauri"
        setBudget={() => Promise.resolve({ ...usage, providerId: "deepseek" })}
      />,
    );

    await user.click(await screen.findByRole("button", { name: "Refresh exact balance" }));
    await act(async () => {
      explicitRefresh.reject(new Error("The newer refresh failed."));
      await explicitRefresh.promise.catch(() => undefined);
    });
    expect(await screen.findByText("The newer refresh failed.")).toBeTruthy();

    await act(async () => {
      sessionLoad.resolve(currentBalance("99.00"));
      await sessionLoad.promise;
    });
    expect(screen.getByText("The newer refresh failed.")).toBeTruthy();
    expect(screen.queryByText("99.00")).toBeNull();
  });

  it("does not apply a completed budget mutation after the user changes provider scope", async () => {
    const user = userEvent.setup();
    const pendingBudget = deferred<UsageSummary>();
    const deepSeekUsage: UsageSummary = {
      ...usage,
      providerId: "deepseek",
      usage: {
        inputTokens: 10,
        cachedInputTokens: 0,
        outputTokens: 10,
        totalTokens: 20,
      },
      budget: null,
    };
    const setBudget = vi.fn(() => pendingBudget.promise);
    render(
      <UsageDialog
        catalog={DEMO_MODEL_CATALOG}
        initialProviderId="zai"
        loadDeepSeekBalance={() =>
          Promise.resolve({
            status: "notChecked",
            observedAt: null,
            isAvailable: null,
            balanceInfos: [],
            error: null,
          })
        }
        loadUsage={(providerId) =>
          Promise.resolve(providerId === "deepseek" ? deepSeekUsage : usage)
        }
        onClose={vi.fn()}
        providerStatuses={DEMO_PROVIDER_STATUSES}
        refreshDeepSeekBalance={vi.fn()}
        runtime="browser-demo"
        setBudget={setBudget}
      />,
    );

    await screen.findByText("49% remaining");
    await user.click(screen.getByRole("button", { name: "Edit budget" }));
    const input = screen.getByRole("textbox", { name: "Token budget" });
    await user.clear(input);
    await user.type(input, "120000");
    await user.click(screen.getByRole("button", { name: "Save budget" }));
    await waitFor(() => {
      expect(setBudget).toHaveBeenCalledWith("zai", 120_000);
    });

    await user.selectOptions(screen.getByRole("combobox", { name: "Usage provider" }), "deepseek");
    expect(await screen.findByText("No local token budget is set for DeepSeek.")).toBeTruthy();

    await act(async () => {
      pendingBudget.resolve({
        ...usage,
        budget: {
          tokenBudget: 120_000,
          knownUsedTokens: 51_000,
          remainingTokens: 69_000,
          remainingPercentage: 57.5,
          state: "normal",
        },
      });
      await pendingBudget.promise;
    });

    expect(screen.getByText("No local token budget is set for DeepSeek.")).toBeTruthy();
    expect(screen.queryByText("120,000 budget")).toBeNull();
  });

  it("keeps a newer saved budget when an older same-scope usage load later resolves", async () => {
    const user = userEvent.setup();
    const olderLoad = deferred<UsageSummary>();
    const savedBudget = deferred<UsageSummary>();
    const firstLoad = vi.fn(() => Promise.resolve(usage));
    const secondLoad = vi.fn(() => olderLoad.promise);
    const setBudget = vi.fn(() => savedBudget.promise);
    const stableBalanceLoad = () => Promise.resolve(null);
    const stableBalanceRefresh = vi.fn();
    const props = {
      catalog: DEMO_MODEL_CATALOG,
      initialProviderId: "zai" as const,
      onClose: vi.fn(),
      providerStatuses: DEMO_PROVIDER_STATUSES,
      runtime: "tauri" as const,
      setBudget,
    };
    const { rerender } = render(
      <UsageDialog
        {...props}
        loadDeepSeekBalance={stableBalanceLoad}
        loadUsage={firstLoad}
        refreshDeepSeekBalance={stableBalanceRefresh}
      />,
    );
    await screen.findByText("49% remaining");

    rerender(
      <UsageDialog
        {...props}
        loadDeepSeekBalance={stableBalanceLoad}
        loadUsage={secondLoad}
        refreshDeepSeekBalance={stableBalanceRefresh}
      />,
    );
    await waitFor(() => {
      expect(secondLoad).toHaveBeenCalledOnce();
    });
    await user.click(screen.getByRole("button", { name: "Edit budget" }));
    const input = screen.getByRole("textbox", { name: "Token budget" });
    await user.clear(input);
    await user.type(input, "120000");
    await user.click(screen.getByRole("button", { name: "Save budget" }));

    const newerSummary: UsageSummary = {
      ...usage,
      budget: {
        tokenBudget: 120_000,
        knownUsedTokens: 51_000,
        remainingTokens: 69_000,
        remainingPercentage: 57.5,
        state: "normal",
      },
    };
    await act(async () => {
      savedBudget.resolve(newerSummary);
      await savedBudget.promise;
    });
    expect(await screen.findByText("57.5% remaining")).toBeTruthy();

    await act(async () => {
      olderLoad.resolve({
        ...usage,
        budget: {
          tokenBudget: 100_000,
          knownUsedTokens: 75_000,
          remainingTokens: 25_000,
          remainingPercentage: 25,
          state: "normal",
        },
      });
      await olderLoad.promise;
    });
    expect(screen.getByText("57.5% remaining")).toBeTruthy();
    expect(screen.queryByText("25% remaining")).toBeNull();
  });

  it("keeps a newer budget error when an older same-scope usage load later resolves", async () => {
    const user = userEvent.setup();
    const olderLoad = deferred<UsageSummary>();
    const savedBudget = deferred<UsageSummary>();
    const firstLoad = vi.fn(() => Promise.resolve(usage));
    const secondLoad = vi.fn(() => olderLoad.promise);
    const setBudget = vi.fn(() => savedBudget.promise);
    const stableBalanceLoad = () => Promise.resolve(null);
    const stableBalanceRefresh = vi.fn();
    const props = {
      catalog: DEMO_MODEL_CATALOG,
      initialProviderId: "zai" as const,
      onClose: vi.fn(),
      providerStatuses: DEMO_PROVIDER_STATUSES,
      runtime: "tauri" as const,
      setBudget,
    };
    const { rerender } = render(
      <UsageDialog
        {...props}
        loadDeepSeekBalance={stableBalanceLoad}
        loadUsage={firstLoad}
        refreshDeepSeekBalance={stableBalanceRefresh}
      />,
    );
    await screen.findByText("49% remaining");
    rerender(
      <UsageDialog
        {...props}
        loadDeepSeekBalance={stableBalanceLoad}
        loadUsage={secondLoad}
        refreshDeepSeekBalance={stableBalanceRefresh}
      />,
    );
    await waitFor(() => {
      expect(secondLoad).toHaveBeenCalledOnce();
    });
    await user.click(screen.getByRole("button", { name: "Edit budget" }));
    const input = screen.getByRole("textbox", { name: "Token budget" });
    await user.clear(input);
    await user.type(input, "120000");
    await user.click(screen.getByRole("button", { name: "Save budget" }));

    await act(async () => {
      savedBudget.reject(new Error("The newer budget mutation failed."));
      await savedBudget.promise.catch(() => undefined);
    });
    expect(await screen.findByText("The newer budget mutation failed.")).toBeTruthy();

    await act(async () => {
      olderLoad.resolve({
        ...usage,
        budget: {
          tokenBudget: 100_000,
          knownUsedTokens: 75_000,
          remainingTokens: 25_000,
          remainingPercentage: 25,
          state: "normal",
        },
      });
      await olderLoad.promise;
    });
    expect(screen.getByText("The newer budget mutation failed.")).toBeTruthy();
    expect(screen.getByText("49% remaining")).toBeTruthy();
    expect(screen.queryByText("25% remaining")).toBeNull();
  });

  it.each([{ latestOutcome: "resolve" as const }, { latestOutcome: "reject" as const }])(
    "keeps the newest concurrent Clear authoritative on $latestOutcome",
    async ({ latestOutcome }) => {
      const user = userEvent.setup();
      const olderSave = deferred<UsageSummary>();
      const newerClear = deferred<UsageSummary>();
      const setBudget = vi
        .fn()
        .mockImplementationOnce(() => olderSave.promise)
        .mockImplementationOnce(() => newerClear.promise);
      render(
        <UsageDialog
          catalog={DEMO_MODEL_CATALOG}
          initialProviderId="zai"
          loadDeepSeekBalance={() => Promise.resolve(null)}
          loadUsage={() => Promise.resolve(usage)}
          onClose={vi.fn()}
          providerStatuses={DEMO_PROVIDER_STATUSES}
          refreshDeepSeekBalance={vi.fn()}
          runtime="tauri"
          setBudget={setBudget}
        />,
      );
      await screen.findByText("49% remaining");
      await user.click(screen.getByRole("button", { name: "Edit budget" }));
      const input = screen.getByRole<HTMLInputElement>("textbox", { name: "Token budget" });
      await user.clear(input);
      await user.type(input, "120000");
      const save = screen.getByRole<HTMLButtonElement>("button", { name: "Save budget" });
      const clear = screen.getByRole<HTMLButtonElement>("button", { name: "Clear budget" });

      act(() => {
        save.dispatchEvent(new MouseEvent("click", { bubbles: true }));
        clear.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      });
      expect(setBudget).toHaveBeenNthCalledWith(1, "zai", 120_000);
      expect(setBudget).toHaveBeenNthCalledWith(2, "zai", null);
      expect(save.disabled).toBe(true);
      expect(clear.disabled).toBe(true);
      expect(input.disabled).toBe(true);
      expect(screen.getByRole<HTMLButtonElement>("button", { name: "Edit budget" }).disabled).toBe(
        true,
      );

      if (latestOutcome === "resolve") {
        await act(async () => {
          newerClear.resolve({ ...usage, budget: null });
          await newerClear.promise;
        });
        expect(await screen.findByText("No local token budget is set for Z.AI.")).toBeTruthy();
      } else {
        await act(async () => {
          newerClear.reject(new Error("The newer Clear failed."));
          await newerClear.promise.catch(() => undefined);
        });
        expect(await screen.findByText("The newer Clear failed.")).toBeTruthy();
      }

      await act(async () => {
        olderSave.resolve({
          ...usage,
          budget: {
            tokenBudget: 120_000,
            knownUsedTokens: 51_000,
            remainingTokens: 69_000,
            remainingPercentage: 57.5,
            state: "normal",
          },
        });
        await olderSave.promise;
      });
      if (latestOutcome === "resolve") {
        expect(screen.getByText("No local token budget is set for Z.AI.")).toBeTruthy();
        expect(screen.queryByText("57.5% remaining")).toBeNull();
      } else {
        expect(screen.getByText("The newer Clear failed.")).toBeTruthy();
        expect(screen.getByText("49% remaining")).toBeTruthy();
        expect(screen.queryByText("57.5% remaining")).toBeNull();
      }
    },
  );

  it("shows a retrieval error without claiming that the DeepSeek account balance is unavailable", async () => {
    render(
      <UsageDialog
        catalog={DEMO_MODEL_CATALOG}
        initialProviderId="deepseek"
        loadDeepSeekBalance={() =>
          Promise.resolve({
            status: "error",
            observedAt: null,
            isAvailable: null,
            balanceInfos: [],
            error: {
              code: "balance_timeout",
              message: "The balance request timed out.",
              retryable: true,
            },
          })
        }
        loadUsage={() => Promise.resolve({ ...usage, providerId: "deepseek" })}
        onClose={vi.fn()}
        providerStatuses={DEMO_PROVIDER_STATUSES}
        refreshDeepSeekBalance={vi.fn()}
        runtime="tauri"
        setBudget={() => Promise.resolve({ ...usage, providerId: "deepseek" })}
      />,
    );

    expect(await screen.findByText("Exact balance could not be retrieved")).toBeTruthy();
    expect(screen.getByText("The balance request timed out.")).toBeTruthy();
    expect(screen.queryByText("Account balance is not available")).toBeNull();
  });

  it("treats a null DeepSeek session status as not checked", async () => {
    render(
      <UsageDialog
        catalog={DEMO_MODEL_CATALOG}
        initialProviderId="deepseek"
        loadDeepSeekBalance={() => Promise.resolve(null)}
        loadUsage={() => Promise.resolve({ ...usage, providerId: "deepseek" })}
        onClose={vi.fn()}
        providerStatuses={DEMO_PROVIDER_STATUSES}
        refreshDeepSeekBalance={vi.fn()}
        runtime="tauri"
        setBudget={() => Promise.resolve({ ...usage, providerId: "deepseek" })}
      />,
    );

    expect(await screen.findByText("Not checked in this session.")).toBeTruthy();
    expect(screen.queryByText(/Account balance is (?:not )?available/)).toBeNull();
    expect(screen.queryByText(/Exact balance could not be retrieved/)).toBeNull();
  });
});
