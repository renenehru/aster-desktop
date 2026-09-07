import { describe, expect, it } from "vitest";

import {
  DEMO_MODEL_CATALOG,
  normalizeDeepSeekBalance,
  normalizeModelCatalog,
  normalizeTokenUsage,
  normalizeUsageSummary,
} from "./providerCatalog";

describe("MVP v2 provider contract", () => {
  it("contains exactly five providers and seventeen selectable models", () => {
    expect(DEMO_MODEL_CATALOG.providers.map((provider) => provider.id)).toEqual([
      "zai",
      "deepseek",
      "alibaba-us",
      "google",
      "nvidia",
    ]);
    expect(
      DEMO_MODEL_CATALOG.providers.flatMap((provider) =>
        provider.models.map((model) => `${provider.id}/${model.id}`),
      ),
    ).toHaveLength(17);
    expect(JSON.stringify(DEMO_MODEL_CATALOG)).not.toMatch(/unavailable|coming soon/i);
    expect(
      DEMO_MODEL_CATALOG.providers
        .find((provider) => provider.id === "google")
        ?.models.find((model) => model.id === "gemini-2.5-pro")
        ?.profiles.find((profile) => profile.id === "fast")?.description,
    ).toMatch(/minimum documented thinking/i);
    expect(
      DEMO_MODEL_CATALOG.providers
        .find((provider) => provider.id === "nvidia")
        ?.models.every((model) => model.delivery === "hosted-prototype"),
    ).toBe(true);
  });

  it("rejects unknown, missing, duplicate, and URL-bearing catalog metadata", () => {
    const unknown = structuredClone(DEMO_MODEL_CATALOG) as unknown as Record<string, unknown>;
    const providers = unknown.providers as Array<Record<string, unknown>>;
    const models = providers[0]?.models as Array<Record<string, unknown>>;
    if (models[0]) models[0].id = "unverified-model";
    expect(() => normalizeModelCatalog(unknown)).toThrow(/unsupported/i);

    const missing = structuredClone(DEMO_MODEL_CATALOG);
    missing.providers.pop();
    expect(() => normalizeModelCatalog(missing)).toThrow(/five approved providers/i);

    const duplicate = structuredClone(DEMO_MODEL_CATALOG);
    const firstProvider = duplicate.providers[0];
    const firstModel = firstProvider?.models[0];
    expect(firstProvider).toBeDefined();
    expect(firstModel).toBeDefined();
    if (!firstProvider || !firstModel) throw new Error("The demo catalog fixture is incomplete.");
    firstProvider.models.splice(1, 1, structuredClone(firstModel));
    expect(() => normalizeModelCatalog(duplicate)).toThrow(/approved MVP v2 inventory/i);

    const withUrl = structuredClone(DEMO_MODEL_CATALOG) as unknown as Record<string, unknown>;
    const firstRawProvider = (withUrl.providers as Array<Record<string, unknown>>)[0];
    if (!firstRawProvider) throw new Error("The demo catalog fixture is incomplete.");
    firstRawProvider.accountUrl = "https://not-allowed.example";
    expect(() => normalizeModelCatalog(withUrl)).toThrow(/unsupported catalog provider metadata/i);
  });

  it("preserves unknown token fields as null and rejects rounded or inconsistent usage", () => {
    expect(
      normalizeTokenUsage({
        inputTokens: null,
        cachedInputTokens: null,
        outputTokens: null,
        totalTokens: 42,
      }),
    ).toEqual({
      inputTokens: null,
      cachedInputTokens: null,
      outputTokens: null,
      totalTokens: 42,
    });
    expect(() =>
      normalizeTokenUsage({
        inputTokens: 1,
        cachedInputTokens: 2,
        outputTokens: 3,
        totalTokens: 7,
      }),
    ).toThrow(/inconsistent/i);
    expect(() =>
      normalizeTokenUsage({
        inputTokens: Number.MAX_SAFE_INTEGER + 1,
        cachedInputTokens: null,
        outputTokens: null,
        totalTokens: null,
      }),
    ).toThrow(/input token count/i);
  });

  it("normalizes partial usage and enforces the advisory warning threshold", () => {
    expect(
      normalizeUsageSummary({
        providerId: "zai",
        modelId: null,
        windowStart: "2026-07-06T12:00:00.000Z",
        windowEnd: "2026-07-13T12:00:00.000Z",
        observedAt: "2026-07-13T12:00:00.000Z",
        usage: {
          inputTokens: 4_000,
          cachedInputTokens: null,
          outputTokens: 5_000,
          totalTokens: 9_000,
        },
        completeObservations: 2,
        partialObservations: 1,
        coverage: "partial",
        budget: {
          tokenBudget: 10_000,
          knownUsedTokens: 9_000,
          remainingTokens: 1_000,
          remainingPercentage: 10,
          state: "low",
        },
      }).budget?.state,
    ).toBe("low");
  });

  it("uses an exact safe-integer threshold and preserves an overflowed budget", () => {
    const tokenBudget = 9_007_199_254_740_989;
    const knownUsedTokens = 8_106_479_329_266_890;
    const remainingTokens = 900_719_925_474_099;
    const base = {
      providerId: "zai",
      modelId: null,
      windowStart: "2026-07-06T12:00:00.000Z",
      windowEnd: "2026-07-13T12:00:00.000Z",
      observedAt: "2026-07-13T12:00:00.000Z",
      usage: {
        inputTokens: null,
        cachedInputTokens: null,
        outputTokens: null,
        totalTokens: null,
      },
      completeObservations: 2,
      partialObservations: 1,
      coverage: "partial",
    };

    expect(
      normalizeUsageSummary({
        ...base,
        budget: {
          tokenBudget,
          knownUsedTokens,
          remainingTokens,
          remainingPercentage: (remainingTokens / tokenBudget) * 100,
          state: "normal",
        },
      }).budget?.state,
    ).toBe("normal");

    expect(
      normalizeUsageSummary({
        ...base,
        budget: {
          tokenBudget: 100,
          knownUsedTokens: null,
          remainingTokens: 0,
          remainingPercentage: 0,
          state: "exhausted",
        },
      }).budget,
    ).toMatchObject({ knownUsedTokens: null, state: "exhausted" });
  });

  it("rejects an advisory percentage that does not match the checked remaining-token formula", () => {
    expect(() =>
      normalizeUsageSummary({
        providerId: "zai",
        modelId: null,
        windowStart: "2026-07-06T12:00:00.000Z",
        windowEnd: "2026-07-13T12:00:00.000Z",
        observedAt: "2026-07-13T12:00:00.000Z",
        usage: {
          inputTokens: 4_000,
          cachedInputTokens: 0,
          outputTokens: 5_000,
          totalTokens: 9_000,
        },
        completeObservations: 2,
        partialObservations: 0,
        coverage: "complete",
        budget: {
          tokenBudget: 10_000,
          knownUsedTokens: 9_000,
          remainingTokens: 1_000,
          remainingPercentage: 90,
          state: "low",
        },
      }),
    ).toThrow(/percentage/i);
  });

  it("keeps DeepSeek money as decimal strings and retains stale state explicitly", () => {
    expect(
      normalizeDeepSeekBalance({
        status: "stale",
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
        error: { code: "balance_timeout", message: "Refresh timed out.", retryable: true },
      }),
    ).toMatchObject({ status: "stale", balanceInfos: [{ totalBalance: "19.25" }] });
  });

  it.each([
    {
      label: "current state with an error",
      value: {
        status: "current",
        observedAt: "2026-07-13T12:00:00.000Z",
        isAvailable: true,
        balanceInfos: [],
        error: { code: "unexpected", message: "Must not coexist.", retryable: false },
      },
    },
    {
      label: "stale state without an error",
      value: {
        status: "stale",
        observedAt: "2026-07-13T12:00:00.000Z",
        isAvailable: true,
        balanceInfos: [],
        error: null,
      },
    },
    {
      label: "error state carrying previously current data",
      value: {
        status: "error",
        observedAt: "2026-07-13T12:00:00.000Z",
        isAvailable: false,
        balanceInfos: [],
        error: { code: "balance_timeout", message: "Refresh timed out.", retryable: true },
      },
    },
    {
      label: "not-checked state carrying an error",
      value: {
        status: "notChecked",
        observedAt: null,
        isAvailable: null,
        balanceInfos: [],
        error: { code: "unexpected", message: "Must be absent.", retryable: false },
      },
    },
  ])("rejects a contradictory DeepSeek $label", ({ value }) => {
    expect(() => normalizeDeepSeekBalance(value)).toThrow(/inconsistent DeepSeek balance state/i);
  });
});
