import type {
  AdvisoryBudget,
  CatalogAccountAction,
  CatalogModel,
  CatalogProfile,
  CatalogProvider,
  DeepSeekBalance,
  DeepSeekBalanceEntry,
  ModelCatalog,
  ModelId,
  ModelSelection,
  ProviderAccountAction,
  ProviderId,
  ProviderStatus,
  ResponseProfile,
  TokenUsage,
  UsageSummary,
} from "../types/providers";

const providerIds = ["zai", "deepseek", "alibaba-us", "google", "nvidia"] as const;
const responseProfiles = ["fast", "standard", "deep"] as const;
const accountActions = ["usage", "billing", "addCredits", "spend", "deployment"] as const;

const modelIdsByProvider = {
  zai: ["glm-4.7", "glm-5", "glm-5.1", "glm-5.2"],
  deepseek: ["deepseek-v4-flash", "deepseek-v4-pro"],
  "alibaba-us": [
    "qwen3.5-plus",
    "qwen3.5-flash",
    "qwen3.6-plus",
    "qwen3.6-flash",
    "qwen3.7-plus",
    "qwen3.7-max",
  ],
  google: ["gemini-2.5-flash", "gemini-2.5-flash-lite", "gemini-2.5-pro"],
  nvidia: ["nvidia/nemotron-3-super-120b-a12b", "nvidia/nemotron-3-ultra-550b-a55b"],
} as const satisfies Record<ProviderId, readonly ModelId[]>;

const actionsByProvider = {
  zai: ["billing", "addCredits"],
  deepseek: ["usage", "addCredits"],
  "alibaba-us": ["usage", "billing", "addCredits"],
  google: ["usage", "billing", "spend"],
  nvidia: ["deployment"],
} as const satisfies Record<ProviderId, readonly ProviderAccountAction[]>;

const exactKeys = (value: Record<string, unknown>, allowed: readonly string[], label: string) => {
  if (Object.keys(value).some((key) => !allowed.includes(key))) {
    throw new Error(`Received unsupported ${label} metadata.`);
  }
};

const record = (value: unknown, label: string): Record<string, unknown> => {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`Received an invalid ${label}.`);
  }
  return value as Record<string, unknown>;
};

const boundedText = (value: unknown, label: string, maximum = 320) => {
  if (typeof value !== "string" || !value.trim() || value.length > maximum) {
    throw new Error(`Received an invalid ${label}.`);
  }
  return value;
};

const boundedDate = (value: unknown, label: string) => {
  const candidate = boundedText(value, label, 64);
  if (Number.isNaN(Date.parse(candidate))) throw new Error(`Received an invalid ${label}.`);
  return candidate;
};

const safeInteger = (value: unknown, label: string, minimum = 0) => {
  if (!Number.isSafeInteger(value) || Number(value) < minimum) {
    throw new Error(`Received an invalid ${label}.`);
  }
  return Number(value);
};

const optionalSafeInteger = (value: unknown, label: string) =>
  value === null ? null : safeInteger(value, label);

export function isProviderId(value: unknown): value is ProviderId {
  return typeof value === "string" && providerIds.includes(value as ProviderId);
}

export function isResponseProfile(value: unknown): value is ResponseProfile {
  return typeof value === "string" && responseProfiles.includes(value as ResponseProfile);
}

export function isProviderAccountAction(value: unknown): value is ProviderAccountAction {
  return typeof value === "string" && accountActions.includes(value as ProviderAccountAction);
}

export function isActionForProvider(providerId: ProviderId, action: ProviderAccountAction) {
  return (actionsByProvider[providerId] as readonly ProviderAccountAction[]).includes(action);
}

export function isModelForProvider(providerId: ProviderId, value: unknown): value is ModelId {
  return (
    typeof value === "string" &&
    (modelIdsByProvider[providerId] as readonly string[]).includes(value)
  );
}

export function asSelection(providerValue: unknown, modelValue: unknown): ModelSelection {
  if (!isProviderId(providerValue) || !isModelForProvider(providerValue, modelValue)) {
    throw new Error("Received an unsupported provider/model pair.");
  }
  return { providerId: providerValue, modelId: modelValue } as ModelSelection;
}

export function normalizeTokenUsage(value: unknown): TokenUsage {
  const raw = record(value, "token usage");
  exactKeys(
    raw,
    ["inputTokens", "cachedInputTokens", "outputTokens", "totalTokens"],
    "token usage",
  );
  const normalized = {
    inputTokens: optionalSafeInteger(raw.inputTokens, "input token count"),
    cachedInputTokens: optionalSafeInteger(raw.cachedInputTokens, "cached-input token count"),
    outputTokens: optionalSafeInteger(raw.outputTokens, "output token count"),
    totalTokens: optionalSafeInteger(raw.totalTokens, "total token count"),
  };
  if (
    normalized.inputTokens !== null &&
    normalized.cachedInputTokens !== null &&
    normalized.outputTokens !== null &&
    normalized.totalTokens !== null &&
    normalized.inputTokens + normalized.cachedInputTokens + normalized.outputTokens !==
      normalized.totalTokens
  ) {
    throw new Error("Received inconsistent token usage.");
  }
  return normalized;
}

function normalizeProfile(value: unknown): CatalogProfile {
  const raw = record(value, "catalog profile");
  exactKeys(raw, ["id", "label", "description", "enabled", "disabledReason"], "catalog profile");
  if (!isResponseProfile(raw.id) || typeof raw.enabled !== "boolean") {
    throw new Error("Received an invalid catalog profile.");
  }
  if (raw.disabledReason !== null && typeof raw.disabledReason !== "string") {
    throw new Error("Received an invalid disabled-profile explanation.");
  }
  if (!raw.enabled && !raw.disabledReason) {
    throw new Error("A disabled profile must explain why it is unavailable.");
  }
  return {
    id: raw.id,
    label: boundedText(raw.label, "profile label", 40),
    description: boundedText(raw.description, "profile description", 240),
    enabled: raw.enabled,
    disabledReason: raw.disabledReason,
  };
}

function normalizeModel(value: unknown, providerId: ProviderId): CatalogModel {
  const raw = record(value, "catalog model");
  exactKeys(raw, ["id", "displayName", "delivery", "profiles"], "catalog model");
  if (!isModelForProvider(providerId, raw.id)) {
    throw new Error("Received an unsupported catalog provider/model pair.");
  }
  const expectedDelivery = providerId === "nvidia" ? "hosted-prototype" : "official-api";
  if (raw.delivery !== expectedDelivery || !Array.isArray(raw.profiles)) {
    throw new Error("Received invalid catalog model delivery metadata.");
  }
  const profiles = raw.profiles.map(normalizeProfile);
  if (
    profiles.length !== responseProfiles.length ||
    responseProfiles.some((profile) => profiles.filter((item) => item.id === profile).length !== 1)
  ) {
    throw new Error("Every catalog model must define each response profile exactly once.");
  }
  return {
    id: raw.id,
    displayName: boundedText(raw.displayName, "model display name", 80),
    delivery: expectedDelivery,
    profiles,
  };
}

function normalizeAction(value: unknown, providerId: ProviderId): CatalogAccountAction {
  const raw = record(value, "provider account action");
  exactKeys(raw, ["action", "label", "description"], "provider account action");
  if (
    !isProviderAccountAction(raw.action) ||
    !(actionsByProvider[providerId] as readonly string[]).includes(raw.action)
  ) {
    throw new Error("Received an unsupported provider account action.");
  }
  return {
    action: raw.action,
    label: boundedText(raw.label, "provider account action label", 60),
    description: boundedText(raw.description, "provider account action description", 180),
  };
}

function normalizeProvider(value: unknown): CatalogProvider {
  const raw = record(value, "catalog provider");
  exactKeys(
    raw,
    [
      "id",
      "displayName",
      "regionLabel",
      "noticeVersion",
      "processingNotice",
      "accountActions",
      "models",
    ],
    "catalog provider",
  );
  if (!isProviderId(raw.id) || !Array.isArray(raw.accountActions) || !Array.isArray(raw.models)) {
    throw new Error("Received an invalid catalog provider.");
  }
  if (raw.regionLabel !== null && typeof raw.regionLabel !== "string") {
    throw new Error("Received an invalid provider region label.");
  }
  const providerId = raw.id;
  const models = raw.models.map((model) => normalizeModel(model, providerId));
  const actions = raw.accountActions.map((action) => normalizeAction(action, providerId));
  const expectedModels = modelIdsByProvider[providerId];
  const expectedActions = actionsByProvider[providerId];
  if (
    models.length !== expectedModels.length ||
    expectedModels.some((modelId) => models.filter((model) => model.id === modelId).length !== 1) ||
    actions.length !== expectedActions.length ||
    expectedActions.some(
      (action) => actions.filter((candidate) => candidate.action === action).length !== 1,
    )
  ) {
    throw new Error("The provider catalog does not match the approved MVP v2 inventory.");
  }
  return {
    id: providerId,
    displayName: boundedText(raw.displayName, "provider display name", 80),
    regionLabel:
      raw.regionLabel === null ? null : boundedText(raw.regionLabel, "provider region label", 80),
    noticeVersion: safeInteger(raw.noticeVersion, "provider notice version", 1),
    processingNotice: boundedText(raw.processingNotice, "provider processing notice", 600),
    accountActions: actions,
    models,
  };
}

export function normalizeModelCatalog(value: unknown): ModelCatalog {
  const raw = record(value, "model catalog");
  exactKeys(raw, ["version", "defaultSelection", "providers"], "model catalog");
  if (raw.version !== 2 || !Array.isArray(raw.providers)) {
    throw new Error("Received an unsupported model catalog version.");
  }
  const defaultRaw = record(raw.defaultSelection, "default model selection");
  exactKeys(defaultRaw, ["providerId", "modelId"], "default model selection");
  const defaultSelection = asSelection(defaultRaw.providerId, defaultRaw.modelId);
  if (defaultSelection.providerId !== "zai" || defaultSelection.modelId !== "glm-5.1") {
    throw new Error("Received an unexpected default model selection.");
  }
  const providers = raw.providers.map(normalizeProvider);
  if (
    providers.length !== providerIds.length ||
    providerIds.some(
      (providerId) => providers.filter((provider) => provider.id === providerId).length !== 1,
    )
  ) {
    throw new Error("The model catalog must contain the exact five approved providers.");
  }
  return { version: 2, defaultSelection, providers };
}

export function normalizeProviderStatuses(value: unknown): ProviderStatus[] {
  if (!Array.isArray(value)) throw new Error("Received an invalid provider status collection.");
  const statuses = value.map((candidate) => {
    const raw = record(candidate, "provider status");
    exactKeys(
      raw,
      ["providerId", "configured", "reachability", "noticeVersion", "noticeAcknowledged"],
      "provider status",
    );
    if (
      !isProviderId(raw.providerId) ||
      typeof raw.configured !== "boolean" ||
      (raw.reachability !== "unknown" &&
        raw.reachability !== "reachable" &&
        raw.reachability !== "unreachable") ||
      typeof raw.noticeAcknowledged !== "boolean"
    ) {
      throw new Error("Received an invalid provider status.");
    }
    return {
      providerId: raw.providerId,
      configured: raw.configured,
      reachability: raw.reachability,
      noticeVersion: safeInteger(raw.noticeVersion, "provider notice version", 1),
      noticeAcknowledged: raw.noticeAcknowledged,
    } satisfies ProviderStatus;
  });
  if (
    statuses.length !== providerIds.length ||
    providerIds.some(
      (providerId) => statuses.filter((status) => status.providerId === providerId).length !== 1,
    )
  ) {
    throw new Error("Received incomplete or duplicate provider statuses.");
  }
  return statuses;
}

function normalizeBudget(value: unknown): AdvisoryBudget | null {
  if (value === null) return null;
  const raw = record(value, "usage budget");
  exactKeys(
    raw,
    ["tokenBudget", "knownUsedTokens", "remainingTokens", "remainingPercentage", "state"],
    "usage budget",
  );
  const tokenBudget = safeInteger(raw.tokenBudget, "usage token budget", 1);
  const knownUsedTokens =
    raw.knownUsedTokens === null
      ? null
      : safeInteger(raw.knownUsedTokens, "known used token count");
  const remainingTokens = safeInteger(raw.remainingTokens, "remaining token count");
  if (
    typeof raw.remainingPercentage !== "number" ||
    !Number.isFinite(raw.remainingPercentage) ||
    raw.remainingPercentage < 0 ||
    raw.remainingPercentage > 100 ||
    (raw.state !== "normal" && raw.state !== "low" && raw.state !== "exhausted")
  ) {
    throw new Error("Received invalid advisory usage budget state.");
  }
  if (knownUsedTokens === null) {
    if (remainingTokens !== 0 || raw.remainingPercentage !== 0 || raw.state !== "exhausted") {
      throw new Error("Received inconsistent overflowed advisory usage budget state.");
    }
    return {
      tokenBudget,
      knownUsedTokens,
      remainingTokens,
      remainingPercentage: raw.remainingPercentage,
      state: raw.state,
    };
  }
  const expectedRemaining = Math.max(tokenBudget - knownUsedTokens, 0);
  const expectedPercentage = (expectedRemaining / tokenBudget) * 100;
  const percentageTolerance = Number.EPSILON * Math.max(1, Math.abs(expectedPercentage)) * 4;
  const expectedState =
    expectedRemaining === 0
      ? "exhausted"
      : expectedRemaining <= Math.floor(tokenBudget / 10)
        ? "low"
        : "normal";
  if (Math.abs(raw.remainingPercentage - expectedPercentage) > percentageTolerance) {
    throw new Error("Received inconsistent advisory usage budget percentage.");
  }
  if (remainingTokens !== expectedRemaining || raw.state !== expectedState) {
    throw new Error("Received inconsistent advisory usage budget state.");
  }
  return {
    tokenBudget,
    knownUsedTokens,
    remainingTokens,
    remainingPercentage: raw.remainingPercentage,
    state: raw.state,
  };
}

export function normalizeUsageSummary(value: unknown): UsageSummary {
  const raw = record(value, "usage summary");
  exactKeys(
    raw,
    [
      "providerId",
      "modelId",
      "windowStart",
      "windowEnd",
      "observedAt",
      "usage",
      "completeObservations",
      "partialObservations",
      "coverage",
      "budget",
    ],
    "usage summary",
  );
  if (!isProviderId(raw.providerId)) throw new Error("Received an invalid usage provider.");
  const providerId = raw.providerId;
  if (raw.modelId !== null && !isModelForProvider(providerId, raw.modelId)) {
    throw new Error("Received an invalid usage model filter.");
  }
  if (raw.coverage !== "empty" && raw.coverage !== "complete" && raw.coverage !== "partial") {
    throw new Error("Received an invalid usage coverage state.");
  }
  const completeObservations = safeInteger(raw.completeObservations, "complete observation count");
  const partialObservations = safeInteger(raw.partialObservations, "partial observation count");
  const coverage = raw.coverage;
  if (
    (coverage === "empty" && (completeObservations !== 0 || partialObservations !== 0)) ||
    (coverage === "complete" && (completeObservations === 0 || partialObservations !== 0)) ||
    (coverage === "partial" && partialObservations === 0)
  ) {
    throw new Error("Received inconsistent usage coverage counts.");
  }
  return {
    providerId,
    modelId: raw.modelId,
    windowStart: boundedDate(raw.windowStart, "usage window start"),
    windowEnd: boundedDate(raw.windowEnd, "usage window end"),
    observedAt: boundedDate(raw.observedAt, "usage observation time"),
    usage: normalizeTokenUsage(raw.usage),
    completeObservations,
    partialObservations,
    coverage,
    budget: normalizeBudget(raw.budget),
  };
}

function decimalString(value: unknown, label: string) {
  const candidate = boundedText(value, label, 64);
  if (!/^\d{1,24}(?:\.\d{1,24})?$/.test(candidate)) {
    throw new Error(`Received an invalid ${label}.`);
  }
  return candidate;
}

function normalizeBalanceEntry(value: unknown): DeepSeekBalanceEntry {
  const raw = record(value, "DeepSeek balance entry");
  exactKeys(
    raw,
    ["currency", "totalBalance", "grantedBalance", "toppedUpBalance"],
    "DeepSeek balance entry",
  );
  if (raw.currency !== "CNY" && raw.currency !== "USD") {
    throw new Error("Received an unsupported DeepSeek balance currency.");
  }
  return {
    currency: raw.currency,
    totalBalance: decimalString(raw.totalBalance, "DeepSeek total balance"),
    grantedBalance: decimalString(raw.grantedBalance, "DeepSeek granted balance"),
    toppedUpBalance: decimalString(raw.toppedUpBalance, "DeepSeek topped-up balance"),
  };
}

export function normalizeDeepSeekBalance(value: unknown): DeepSeekBalance {
  const raw = record(value, "DeepSeek balance status");
  exactKeys(
    raw,
    ["status", "observedAt", "isAvailable", "balanceInfos", "error"],
    "DeepSeek balance status",
  );
  if (
    raw.status !== "notChecked" &&
    raw.status !== "current" &&
    raw.status !== "stale" &&
    raw.status !== "error"
  ) {
    throw new Error("Received an invalid DeepSeek balance status.");
  }
  if (!Array.isArray(raw.balanceInfos) || raw.balanceInfos.length > 16) {
    throw new Error("Received an invalid DeepSeek balance collection.");
  }
  if (raw.isAvailable !== null && typeof raw.isAvailable !== "boolean") {
    throw new Error("Received an invalid DeepSeek balance availability state.");
  }
  const observedAt = raw.observedAt === null ? null : boundedDate(raw.observedAt, "balance time");
  let error = null;
  if (raw.error !== null) {
    const errorRaw = record(raw.error, "balance error");
    exactKeys(errorRaw, ["code", "message", "retryable"], "balance error");
    if (typeof errorRaw.retryable !== "boolean")
      throw new Error("Received an invalid balance error.");
    error = {
      code: boundedText(errorRaw.code, "balance error code", 64),
      message: boundedText(errorRaw.message, "balance error message", 240),
      retryable: errorRaw.retryable,
    };
  }
  const isNotChecked =
    raw.status === "notChecked" &&
    observedAt === null &&
    raw.isAvailable === null &&
    raw.balanceInfos.length === 0 &&
    error === null;
  const isCurrent =
    raw.status === "current" &&
    observedAt !== null &&
    typeof raw.isAvailable === "boolean" &&
    error === null;
  const isStale =
    raw.status === "stale" &&
    observedAt !== null &&
    typeof raw.isAvailable === "boolean" &&
    error !== null;
  const isError =
    raw.status === "error" &&
    observedAt === null &&
    raw.isAvailable === null &&
    raw.balanceInfos.length === 0 &&
    error !== null;
  if (!isNotChecked && !isCurrent && !isStale && !isError) {
    throw new Error("Received inconsistent DeepSeek balance state.");
  }
  return {
    status: raw.status,
    observedAt,
    isAvailable: raw.isAvailable,
    balanceInfos: raw.balanceInfos.map(normalizeBalanceEntry),
    error,
  };
}

const profiles = (fast: string, standard: string, deep: string): CatalogProfile[] => [
  { id: "fast", label: "Fast", description: fast, enabled: true, disabledReason: null },
  {
    id: "standard",
    label: "Standard",
    description: standard,
    enabled: true,
    disabledReason: null,
  },
  { id: "deep", label: "Deep", description: deep, enabled: true, disabledReason: null },
];

const model = (
  id: ModelId,
  displayName: string,
  modelProfiles: CatalogProfile[],
  delivery: CatalogModel["delivery"] = "official-api",
): CatalogModel => ({ id, displayName, delivery, profiles: modelProfiles });

const action = (
  id: ProviderAccountAction,
  label: string,
  description: string,
): CatalogAccountAction => ({ action: id, label, description });

const zaiProfiles = profiles(
  "Thinking off · 4,096-token output cap",
  "Thinking on · 8,192-token output cap",
  "Thinking on · 16,384-token output cap",
);
const deepSeekProfiles = profiles(
  "Thinking off · 4,096-token output cap",
  "High documented reasoning effort · 8,192-token output cap",
  "Maximum documented reasoning effort · 16,384-token output cap",
);
const alibabaProfiles = profiles(
  "Thinking off · 4,096-token output cap",
  "Thinking on · 8,192-token output cap",
  "Thinking on · 16,384-token output cap",
);
const geminiFlashProfiles = profiles(
  "Thinking off · 4,096-token output cap",
  "1,024-token thinking budget · 8,192-token output cap",
  "8,192-token thinking budget · 16,384-token output cap",
);
const geminiProProfiles = profiles(
  "Minimum documented thinking · 4,096-token output cap",
  "4,096-token thinking budget · 8,192-token output cap",
  "16,384-token thinking budget · 16,384-token output cap",
);
const nvidiaProfiles = profiles(
  "Thinking off · 4,096-token output cap",
  "4,096-token reasoning budget · 8,192-token output cap",
  "16,384-token reasoning budget · 16,384-token output cap",
);

export const DEMO_MODEL_CATALOG: ModelCatalog = {
  version: 2,
  defaultSelection: { providerId: "zai", modelId: "glm-5.1" },
  providers: [
    {
      id: "zai",
      displayName: "Z.AI",
      regionLabel: null,
      noticeVersion: 1,
      processingNotice:
        "Aster sends this conversation's relevant messages directly to Z.AI. Review Z.AI's data handling before sending sensitive information.",
      accountActions: [
        action("billing", "Billing details", "Review billing and plan options on Z.AI."),
        action("addCredits", "Add credits", "Open Z.AI's billing page to add credits manually."),
      ],
      models: [
        model("glm-4.7", "GLM-4.7", zaiProfiles),
        model("glm-5", "GLM-5", zaiProfiles),
        model("glm-5.1", "GLM-5.1", zaiProfiles),
        model("glm-5.2", "GLM-5.2", zaiProfiles),
      ],
    },
    {
      id: "deepseek",
      displayName: "DeepSeek",
      regionLabel: null,
      noticeVersion: 1,
      processingNotice:
        "Aster sends this conversation's relevant messages directly to DeepSeek. Review DeepSeek's data handling before sending sensitive information.",
      accountActions: [
        action("usage", "View usage", "Review authoritative usage on DeepSeek."),
        action("addCredits", "Add credits", "Open DeepSeek's manual top-up page."),
      ],
      models: [
        model("deepseek-v4-flash", "DeepSeek V4 Flash", deepSeekProfiles),
        model("deepseek-v4-pro", "DeepSeek V4 Pro", deepSeekProfiles),
      ],
    },
    {
      id: "alibaba-us",
      displayName: "Alibaba Cloud (US)",
      regionLabel: "United States",
      noticeVersion: 1,
      processingNotice:
        "Aster sends this conversation's relevant messages to Alibaba Cloud's fixed United States region. Review Alibaba Cloud's data handling before sending sensitive information.",
      accountActions: [
        action("usage", "View usage", "Review usage in Alibaba Cloud Model Studio."),
        action("billing", "Billing details", "Review Alibaba Cloud billing details."),
        action("addCredits", "Add credits", "Open Alibaba Cloud's manual funding page."),
      ],
      models: [
        model("qwen3.5-plus", "Qwen3.5 Plus", alibabaProfiles),
        model("qwen3.5-flash", "Qwen3.5 Flash", alibabaProfiles),
        model("qwen3.6-plus", "Qwen3.6 Plus", alibabaProfiles),
        model("qwen3.6-flash", "Qwen3.6 Flash", alibabaProfiles),
        model("qwen3.7-plus", "Qwen3.7 Plus", alibabaProfiles),
        model("qwen3.7-max", "Qwen3.7 Max", alibabaProfiles),
      ],
    },
    {
      id: "google",
      displayName: "Google Gemini",
      regionLabel: null,
      noticeVersion: 1,
      processingNotice:
        "Aster sends this conversation's relevant messages directly to the Google Gemini API. Review Google's data handling before sending sensitive information.",
      accountActions: [
        action("usage", "View usage", "Review Gemini API usage in Google AI Studio."),
        action("billing", "Billing details", "Review Gemini API billing settings."),
        action("spend", "View spend", "Review recorded Gemini API spend."),
      ],
      models: [
        model("gemini-2.5-flash", "Gemini 2.5 Flash", geminiFlashProfiles),
        model("gemini-2.5-flash-lite", "Gemini 2.5 Flash-Lite", geminiFlashProfiles),
        model("gemini-2.5-pro", "Gemini 2.5 Pro", geminiProProfiles),
      ],
    },
    {
      id: "nvidia",
      displayName: "NVIDIA NIM (Prototype)",
      regionLabel: null,
      noticeVersion: 1,
      processingNotice:
        "Aster sends this conversation's relevant messages to NVIDIA's hosted prototype service for evaluation. This is not a production deployment or evidence of NVIDIA AI Enterprise coverage.",
      accountActions: [
        action(
          "deployment",
          "Model and deployment",
          "Review NVIDIA's official hosted-model and deployment information.",
        ),
      ],
      models: [
        model(
          "nvidia/nemotron-3-super-120b-a12b",
          "Nemotron 3 Super",
          nvidiaProfiles,
          "hosted-prototype",
        ),
        model(
          "nvidia/nemotron-3-ultra-550b-a55b",
          "Nemotron 3 Ultra",
          nvidiaProfiles,
          "hosted-prototype",
        ),
      ],
    },
  ],
};

export const DEMO_PROVIDER_STATUSES: ProviderStatus[] = DEMO_MODEL_CATALOG.providers.map(
  (provider) => ({
    providerId: provider.id,
    configured: false,
    reachability: "unknown",
    noticeVersion: provider.noticeVersion,
    noticeAcknowledged: true,
  }),
);

// Validate the fixture through the same strict boundary used for desktop IPC.
normalizeModelCatalog(structuredClone(DEMO_MODEL_CATALOG));
normalizeProviderStatuses(structuredClone(DEMO_PROVIDER_STATUSES));
