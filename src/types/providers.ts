export type ResponseProfile = "fast" | "standard" | "deep";

export type ProviderId = "zai" | "deepseek" | "alibaba-us" | "google" | "nvidia";

export type ProviderAccountAction = "usage" | "billing" | "addCredits" | "spend" | "deployment";

export type ZaiModelId = "glm-4.7" | "glm-5" | "glm-5.1" | "glm-5.2";
export type DeepSeekModelId = "deepseek-v4-flash" | "deepseek-v4-pro";
export type AlibabaModelId =
  | "qwen3.5-plus"
  | "qwen3.5-flash"
  | "qwen3.6-plus"
  | "qwen3.6-flash"
  | "qwen3.7-plus"
  | "qwen3.7-max";
export type GoogleModelId = "gemini-2.5-flash" | "gemini-2.5-flash-lite" | "gemini-2.5-pro";
export type NvidiaModelId =
  "nvidia/nemotron-3-super-120b-a12b" | "nvidia/nemotron-3-ultra-550b-a55b";

export type ModelId = ZaiModelId | DeepSeekModelId | AlibabaModelId | GoogleModelId | NvidiaModelId;

export type ModelSelection =
  | Readonly<{ providerId: "zai"; modelId: ZaiModelId }>
  | Readonly<{ providerId: "deepseek"; modelId: DeepSeekModelId }>
  | Readonly<{ providerId: "alibaba-us"; modelId: AlibabaModelId }>
  | Readonly<{ providerId: "google"; modelId: GoogleModelId }>
  | Readonly<{ providerId: "nvidia"; modelId: NvidiaModelId }>;

export interface CatalogProfile {
  id: ResponseProfile;
  label: string;
  description: string;
  enabled: boolean;
  disabledReason: string | null;
}

export interface CatalogModel {
  id: ModelId;
  displayName: string;
  delivery: "official-api" | "hosted-prototype";
  profiles: CatalogProfile[];
}

export interface CatalogAccountAction {
  action: ProviderAccountAction;
  label: string;
  description: string;
}

export interface CatalogProvider {
  id: ProviderId;
  displayName: string;
  regionLabel: string | null;
  noticeVersion: number;
  processingNotice: string;
  accountActions: CatalogAccountAction[];
  models: CatalogModel[];
}

export interface ModelCatalog {
  version: 2;
  defaultSelection: ModelSelection;
  providers: CatalogProvider[];
}

export type ProviderReachability = "unknown" | "reachable" | "unreachable";

export interface ProviderStatus {
  providerId: ProviderId;
  configured: boolean;
  reachability: ProviderReachability;
  noticeVersion: number;
  noticeAcknowledged: boolean;
}

export interface TokenUsage {
  inputTokens: number | null;
  cachedInputTokens: number | null;
  outputTokens: number | null;
  totalTokens: number | null;
}

export type UsageQuality = "empty" | "complete" | "partial";

export interface AdvisoryBudget {
  tokenBudget: number;
  knownUsedTokens: number | null;
  remainingTokens: number;
  remainingPercentage: number;
  state: "normal" | "low" | "exhausted";
}

export interface UsageSummary {
  providerId: ProviderId;
  modelId: ModelId | null;
  windowStart: string;
  windowEnd: string;
  observedAt: string;
  usage: TokenUsage;
  completeObservations: number;
  partialObservations: number;
  coverage: UsageQuality;
  budget: AdvisoryBudget | null;
}

export interface DeepSeekBalanceEntry {
  currency: string;
  totalBalance: string;
  grantedBalance: string;
  toppedUpBalance: string;
}

export interface PublicErrorState {
  code: string;
  message: string;
  retryable: boolean;
}

export interface DeepSeekBalance {
  status: "notChecked" | "current" | "stale" | "error";
  observedAt: string | null;
  isAvailable: boolean | null;
  balanceInfos: DeepSeekBalanceEntry[];
  error: PublicErrorState | null;
}

export const CATALOG_MODEL_COUNT = 17;

export function sameSelection(left: ModelSelection, right: ModelSelection) {
  return left.providerId === right.providerId && left.modelId === right.modelId;
}

export function selectionLabel(catalog: ModelCatalog, selection: ModelSelection) {
  const provider = catalog.providers.find((candidate) => candidate.id === selection.providerId);
  const model = provider?.models.find((candidate) => candidate.id === selection.modelId);
  return `${provider?.displayName ?? selection.providerId} · ${model?.displayName ?? selection.modelId}`;
}
