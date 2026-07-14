import { useEffect, useMemo, useRef, useState } from "react";

import type {
  DeepSeekBalance,
  ModelCatalog,
  ModelId,
  ProviderAccountAction,
  ProviderId,
  ProviderStatus,
  UsageSummary,
} from "../types/providers";
import { Dialog } from "./Dialog";
import { Icon } from "./Icon";

interface UsageDialogProps {
  catalog: ModelCatalog;
  initialProviderId: ProviderId;
  providerStatuses: ProviderStatus[];
  runtime: "tauri" | "browser-demo";
  loadUsage: (providerId: ProviderId, modelId?: ModelId) => Promise<UsageSummary>;
  setBudget: (providerId: ProviderId, tokenBudget: number | null) => Promise<UsageSummary>;
  loadDeepSeekBalance: () => Promise<DeepSeekBalance | null>;
  refreshDeepSeekBalance: () => Promise<DeepSeekBalance>;
  openProviderAccount?: (providerId: ProviderId, action: ProviderAccountAction) => Promise<void>;
  onClose: () => void;
}

const numberFormatter = new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 });

function remainingPercentageLabel(
  remainingPercentage: number,
  state: "normal" | "low" | "exhausted",
) {
  if (state === "normal" && remainingPercentage < 10.1) {
    return ">10";
  }
  return numberFormatter.format(remainingPercentage);
}

function tokenLabel(value: number | null) {
  return value === null ? "—" : numberFormatter.format(value);
}

function timeLabel(value: string | null) {
  if (!value) return "No observations yet";
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? "Unknown time"
    : date.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

function coverageLabel(summary: UsageSummary) {
  if (summary.coverage === "empty") return "No usage observed";
  if (summary.coverage === "partial") return "Partial data";
  return "Current local data";
}

function catalogProvider(catalog: ModelCatalog, providerId: ProviderId) {
  const provider = catalog.providers.find((candidate) => candidate.id === providerId);
  if (!provider) throw new Error("The selected provider is missing from the curated catalog.");
  return provider;
}

export function UsageDialog({
  catalog,
  initialProviderId,
  providerStatuses,
  runtime,
  loadUsage,
  setBudget,
  loadDeepSeekBalance,
  refreshDeepSeekBalance,
  openProviderAccount,
  onClose,
}: UsageDialogProps) {
  const [providerId, setProviderId] = useState(initialProviderId);
  const [modelId, setModelId] = useState<ModelId | null>(null);
  const [summary, setSummary] = useState<UsageSummary | null>(null);
  const [loading, setLoading] = useState(true);
  const [stale, setStale] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [refreshNonce, setRefreshNonce] = useState(0);
  const [editingBudget, setEditingBudget] = useState(false);
  const [budgetValue, setBudgetValue] = useState("");
  const [budgetError, setBudgetError] = useState<string | null>(null);
  const [savingBudget, setSavingBudget] = useState(false);
  const [balance, setBalance] = useState<DeepSeekBalance | null>(null);
  const [balanceLoading, setBalanceLoading] = useState(false);
  const [accountError, setAccountError] = useState<string | null>(null);
  const [openingAction, setOpeningAction] = useState<ProviderAccountAction | null>(null);
  const usageOperationVersion = useRef(0);
  const balanceOperationVersion = useRef(0);
  const scopeVersion = useRef(0);
  const providerScopeVersion = useRef(0);
  const lastSummary = useRef<UsageSummary | null>(null);

  const provider = useMemo(() => catalogProvider(catalog, providerId), [catalog, providerId]);
  const providerStatus = providerStatuses.find((candidate) => candidate.providerId === providerId);

  useEffect(() => {
    const operationVersion = ++usageOperationVersion.current;
    const operationScope = scopeVersion.current;
    const previous = lastSummary.current;
    const sameScope = previous?.providerId === providerId && previous.modelId === (modelId ?? null);
    if (!sameScope) {
      setSummary(null);
      setLoading(true);
    }
    setLoadError(null);
    void loadUsage(providerId, modelId ?? undefined).then(
      (next) => {
        if (
          operationVersion !== usageOperationVersion.current ||
          operationScope !== scopeVersion.current
        )
          return;
        if (next.providerId !== providerId || next.modelId !== (modelId ?? null)) {
          setLoading(false);
          setLoadError("Aster rejected local usage outside the requested scope.");
          if (sameScope) {
            setSummary(previous);
            setStale(true);
          } else {
            setSummary(null);
          }
          return;
        }
        lastSummary.current = next;
        setSummary(next);
        setLoading(false);
        setStale(false);
      },
      (error: unknown) => {
        if (
          operationVersion !== usageOperationVersion.current ||
          operationScope !== scopeVersion.current
        )
          return;
        const message =
          error instanceof Error ? error.message : "Aster could not load local usage.";
        setLoading(false);
        setLoadError(message);
        if (sameScope) {
          setSummary(previous);
          setStale(true);
        } else {
          setSummary(null);
        }
      },
    );
    return () => {
      if (operationVersion === usageOperationVersion.current) {
        usageOperationVersion.current += 1;
      }
    };
  }, [loadUsage, modelId, providerId, refreshNonce]);

  useEffect(() => {
    const operationVersion = ++balanceOperationVersion.current;
    const operationProviderScope = providerScopeVersion.current;
    setBalanceLoading(false);
    setBalance(null);
    if (providerId !== "deepseek") return;
    void loadDeepSeekBalance().then(
      (next) => {
        if (
          operationVersion === balanceOperationVersion.current &&
          operationProviderScope === providerScopeVersion.current
        )
          setBalance(next);
      },
      () => {
        if (
          operationVersion !== balanceOperationVersion.current ||
          operationProviderScope !== providerScopeVersion.current
        )
          return;
        setBalance({
          status: "error",
          observedAt: null,
          isAvailable: null,
          balanceInfos: [],
          error: {
            code: "balance_status_unavailable",
            message: "Aster could not read the in-session balance status.",
            retryable: true,
          },
        });
      },
    );
    return () => {
      if (operationVersion === balanceOperationVersion.current) {
        balanceOperationVersion.current += 1;
      }
    };
  }, [loadDeepSeekBalance, providerId]);

  const chooseProvider = (next: ProviderId) => {
    scopeVersion.current += 1;
    providerScopeVersion.current += 1;
    usageOperationVersion.current += 1;
    balanceOperationVersion.current += 1;
    setProviderId(next);
    setModelId(null);
    setEditingBudget(false);
    setSavingBudget(false);
    setBalanceLoading(false);
    setBudgetError(null);
    setAccountError(null);
    setOpeningAction(null);
  };

  const chooseModel = (next: ModelId | null) => {
    scopeVersion.current += 1;
    usageOperationVersion.current += 1;
    setModelId(next);
    setEditingBudget(false);
    setSavingBudget(false);
    setBudgetError(null);
  };

  const saveBudget = async (value: number | null) => {
    const operationVersion = ++usageOperationVersion.current;
    const operationScope = scopeVersion.current;
    const operationProviderId = providerId;
    setSavingBudget(true);
    setBudgetError(null);
    try {
      const next = await setBudget(operationProviderId, value);
      if (
        operationVersion !== usageOperationVersion.current ||
        operationScope !== scopeVersion.current
      )
        return;
      if (next.providerId !== operationProviderId || next.modelId !== null) {
        throw new Error("Aster rejected a budget result outside the requested provider scope.");
      }
      if (modelId === null) {
        lastSummary.current = next;
        setSummary(next);
      } else {
        setRefreshNonce((current) => current + 1);
      }
      setEditingBudget(false);
      setBudgetValue("");
      setStale(false);
    } catch (error) {
      if (
        operationVersion !== usageOperationVersion.current ||
        operationScope !== scopeVersion.current
      )
        return;
      setBudgetError(error instanceof Error ? error.message : "Aster could not save the budget.");
    } finally {
      if (
        operationVersion === usageOperationVersion.current &&
        operationScope === scopeVersion.current
      )
        setSavingBudget(false);
    }
  };

  const submitBudget = () => {
    if (!/^\d+$/.test(budgetValue)) {
      setBudgetError("Enter a positive whole-number token budget.");
      return;
    }
    const value = Number(budgetValue);
    if (!Number.isSafeInteger(value) || value < 1) {
      setBudgetError("Budget must be between 1 and 9,007,199,254,740,991 tokens.");
      return;
    }
    void saveBudget(value);
  };

  const refreshBalance = async () => {
    const operationVersion = ++balanceOperationVersion.current;
    const operationProviderScope = providerScopeVersion.current;
    setBalanceLoading(true);
    try {
      const next = await refreshDeepSeekBalance();
      if (
        operationVersion !== balanceOperationVersion.current ||
        operationProviderScope !== providerScopeVersion.current
      )
        return;
      setBalance(next);
    } catch (error) {
      if (
        operationVersion !== balanceOperationVersion.current ||
        operationProviderScope !== providerScopeVersion.current
      )
        return;
      setBalance((current) => ({
        status: current?.observedAt ? "stale" : "error",
        observedAt: current?.observedAt ?? null,
        isAvailable: current?.isAvailable ?? null,
        balanceInfos: current?.balanceInfos ?? [],
        error: {
          code: "balance_refresh_failed",
          message: error instanceof Error ? error.message : "Balance refresh failed.",
          retryable: true,
        },
      }));
    } finally {
      if (
        operationVersion === balanceOperationVersion.current &&
        operationProviderScope === providerScopeVersion.current
      )
        setBalanceLoading(false);
    }
  };

  const openAccount = async (action: ProviderAccountAction) => {
    if (!openProviderAccount) return;
    const operationProviderScope = providerScopeVersion.current;
    const operationProviderId = providerId;
    setOpeningAction(action);
    setAccountError(null);
    try {
      await openProviderAccount(operationProviderId, action);
    } catch (error) {
      if (operationProviderScope !== providerScopeVersion.current) return;
      setAccountError(
        error instanceof Error ? error.message : "Aster could not open the provider website.",
      );
    } finally {
      if (operationProviderScope === providerScopeVersion.current) setOpeningAction(null);
    }
  };

  const budget = summary?.budget ?? null;
  const criticalBudget = Boolean(budget && budget.state !== "normal");
  const remainingPercentage = budget ? Math.min(100, Math.max(0, budget.remainingPercentage)) : 0;
  const remainingLabel = budget
    ? remainingPercentageLabel(budget.remainingPercentage, budget.state)
    : "0";

  return (
    <Dialog label="Usage" onClose={onClose} size="medium">
      <div className="usage-dialog">
        {runtime === "browser-demo" && (
          <div className="usage-demo-banner" role="status">
            <Icon name="shield" size={15} /> Synthetic demo data · no provider or billing request
          </div>
        )}
        <div className="usage-filters">
          <label>
            <span>Provider</span>
            <select
              aria-label="Usage provider"
              onChange={(event) => {
                chooseProvider(event.target.value as ProviderId);
              }}
              value={providerId}
            >
              {catalog.providers.map((candidate) => (
                <option key={candidate.id} value={candidate.id}>
                  {candidate.displayName}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span>Model</span>
            <select
              aria-label="Usage model filter"
              onChange={(event) => {
                chooseModel((event.target.value || null) as ModelId | null);
              }}
              value={modelId ?? ""}
            >
              <option value="">All models</option>
              {provider.models.map((model) => (
                <option key={model.id} value={model.id}>
                  {model.displayName}
                </option>
              ))}
            </select>
          </label>
        </div>

        {loading && !summary ? (
          <div className="usage-loading" aria-label="Loading local usage">
            <span />
            <span />
            <span />
          </div>
        ) : loadError && !summary ? (
          <div className="usage-error" role="alert">
            <Icon name="warning" size={18} />
            <div>
              <strong>Local usage could not be loaded</strong>
              <p>{loadError}</p>
            </div>
            <button
              type="button"
              onClick={() => {
                usageOperationVersion.current += 1;
                setRefreshNonce((value) => value + 1);
              }}
            >
              Retry
            </button>
          </div>
        ) : summary ? (
          <>
            <section className="usage-card usage-observed-card">
              <div className="usage-card-heading">
                <div>
                  <strong>Locally observed usage</strong>
                  <span>Rolling 7 days · advisory data</span>
                </div>
                <span className={`usage-quality quality-${summary.coverage}`}>
                  {stale ? "Stale local data" : coverageLabel(summary)}
                </span>
              </div>
              {stale && (
                <div className="usage-stale" role="status">
                  <Icon name="warning" size={14} /> Refresh failed. Showing the last successful
                  local summary from {timeLabel(summary.observedAt)}.
                </div>
              )}
              {providerStatus?.reachability === "unreachable" && (
                <p className="usage-offline-note">
                  Provider connection is currently unreachable. Local SQLite usage remains
                  available.
                </p>
              )}
              <dl className="usage-token-grid">
                <div>
                  <dt>Input</dt>
                  <dd>{tokenLabel(summary.usage.inputTokens)}</dd>
                </div>
                <div>
                  <dt>Cached</dt>
                  <dd>{tokenLabel(summary.usage.cachedInputTokens)}</dd>
                </div>
                <div>
                  <dt>Output</dt>
                  <dd>{tokenLabel(summary.usage.outputTokens)}</dd>
                </div>
                <div>
                  <dt>Total</dt>
                  <dd>{tokenLabel(summary.usage.totalTokens)}</dd>
                </div>
              </dl>
              {summary.coverage === "partial" && (
                <p className="usage-partial-note">
                  Some provider responses omitted or returned invalid token fields. Unknown values
                  are shown as — and are not treated as zero.
                </p>
              )}
              <p className="usage-observed-time">Last observed: {timeLabel(summary.observedAt)}</p>
            </section>

            <section className={`usage-card usage-budget-card ${criticalBudget ? "critical" : ""}`}>
              <div className="usage-card-heading">
                <div>
                  <strong>Advisory 7-day token budget</strong>
                  <span>Local only · never blocks requests</span>
                </div>
                {budget && (
                  <b className={criticalBudget ? "budget-critical-label" : undefined}>
                    {criticalBudget && <Icon name="warning" size={13} />}
                    {criticalBudget ? "Critical · " : ""}
                    {remainingLabel}% remaining
                  </b>
                )}
              </div>
              {budget ? (
                <>
                  <progress
                    aria-label={`${criticalBudget ? "Critical: " : ""}${remainingLabel === ">10" ? "More than 10" : remainingLabel} percent of the advisory token budget remains`}
                    max={100}
                    value={remainingPercentage}
                  />
                  <div className="budget-totals">
                    <span>
                      {budget.knownUsedTokens === null
                        ? "Exact known usage exceeds the supported display range"
                        : `${numberFormatter.format(budget.knownUsedTokens)} known tokens used`}
                    </span>
                    <span>{numberFormatter.format(budget.tokenBudget)} budget</span>
                  </div>
                  {criticalBudget && (
                    <div
                      aria-label="Critical advisory token budget"
                      aria-live="polite"
                      className="budget-warning"
                      role="status"
                    >
                      <Icon name="warning" size={17} />
                      <span>
                        {budget.state === "exhausted"
                          ? "Known local usage has reached the advisory budget."
                          : "Low advisory budget: 10% or less remains based on known local usage."}
                        {summary.coverage === "partial" ? " Additional usage may be missing." : ""}
                      </span>
                    </div>
                  )}
                  <div className="budget-actions">
                    <button
                      className="button quiet"
                      disabled={savingBudget}
                      type="button"
                      onClick={() => {
                        setBudgetValue(String(budget.tokenBudget));
                        setEditingBudget(true);
                      }}
                    >
                      Edit budget
                    </button>
                    <button
                      className="button quiet"
                      disabled={savingBudget}
                      type="button"
                      onClick={() => void saveBudget(null)}
                    >
                      Clear budget
                    </button>
                  </div>
                </>
              ) : (
                <div className="budget-empty">
                  <p>No local token budget is set for {provider.displayName}.</p>
                  <button
                    className="button secondary"
                    disabled={savingBudget}
                    type="button"
                    onClick={() => {
                      setEditingBudget(true);
                    }}
                  >
                    Set budget
                  </button>
                </div>
              )}
              {editingBudget && (
                <div className="budget-editor">
                  <label>
                    <span>Token budget</span>
                    <input
                      aria-describedby="budget-help"
                      autoComplete="off"
                      disabled={savingBudget}
                      inputMode="numeric"
                      onChange={(event) => {
                        setBudgetValue(event.target.value);
                      }}
                      placeholder="100000"
                      value={budgetValue}
                    />
                  </label>
                  <p id="budget-help">
                    Positive whole tokens. This is not provider credit or balance.
                  </p>
                  {budgetError && (
                    <p className="inline-error" role="alert">
                      {budgetError}
                    </p>
                  )}
                  <div className="budget-editor-actions">
                    <button
                      className="button secondary"
                      disabled={savingBudget}
                      type="button"
                      onClick={() => {
                        setEditingBudget(false);
                      }}
                    >
                      Cancel
                    </button>
                    <button
                      className="button primary"
                      disabled={savingBudget}
                      type="button"
                      onClick={submitBudget}
                    >
                      {savingBudget ? "Saving…" : "Save budget"}
                    </button>
                  </div>
                </div>
              )}
            </section>

            <section className="usage-card balance-card">
              <div className="usage-card-heading">
                <div>
                  <strong>
                    {providerId === "deepseek" ? "Exact DeepSeek balance" : "Provider balance"}
                  </strong>
                  <span>
                    {providerId === "deepseek"
                      ? "Read only · refreshed only when you ask"
                      : "Aster does not retrieve this provider's balance"}
                  </span>
                </div>
              </div>
              {providerId === "deepseek" ? (
                <div className="deepseek-balance">
                  {!balance || balance.status === "notChecked" ? (
                    <p>Not checked in this session.</p>
                  ) : balance.status === "error" ? (
                    <div className="balance-error" role="alert">
                      <Icon name="warning" size={14} />
                      <div>
                        <strong>Exact balance could not be retrieved</strong>
                        <p>{balance.error?.message ?? "The balance request failed."}</p>
                      </div>
                    </div>
                  ) : (
                    <>
                      <p className="balance-availability">
                        {balance.isAvailable
                          ? "Account balance is available"
                          : "Account balance is not available"}
                        {balance.observedAt ? ` · ${timeLabel(balance.observedAt)}` : ""}
                      </p>
                      <div className="balance-grid">
                        {balance.balanceInfos.map((entry) => (
                          <div key={entry.currency}>
                            <span>{entry.currency}</span>
                            <strong>{entry.totalBalance}</strong>
                            <small>
                              Granted {entry.grantedBalance} · Topped up {entry.toppedUpBalance}
                            </small>
                          </div>
                        ))}
                      </div>
                    </>
                  )}
                  {balance?.error && balance.status !== "error" && (
                    <p className="balance-error" role="status">
                      <Icon name="warning" size={14} /> {balance.error.message}
                      {balance.status === "stale"
                        ? " The last successful balance remains above."
                        : ""}
                    </p>
                  )}
                  <button
                    className="button secondary"
                    disabled={balanceLoading}
                    type="button"
                    onClick={() => void refreshBalance()}
                  >
                    <Icon name="refresh" size={15} />
                    {balanceLoading
                      ? "Refreshing…"
                      : runtime === "browser-demo"
                        ? "Refresh synthetic balance"
                        : "Refresh exact balance"}
                  </button>
                </div>
              ) : (
                <p className="provider-balance-copy">
                  Check the authoritative balance on the provider website. Local token observations
                  are not credit or billing data.
                </p>
              )}
            </section>
          </>
        ) : null}

        <div className="usage-divider">
          <span>Manage on provider website</span>
        </div>
        <div className="provider-account-actions">
          {provider.accountActions.map((providerAction) =>
            runtime === "browser-demo" ? (
              <div className="account-action-card demo" key={providerAction.action}>
                <Icon name="external" size={21} />
                <strong>{providerAction.label}</strong>
                <span>Available only in the Windows desktop app</span>
              </div>
            ) : (
              <button
                aria-label={`${providerAction.label} on ${provider.displayName} (opens in default browser)`}
                className="account-action-card"
                disabled={openingAction !== null}
                key={providerAction.action}
                onClick={() => void openAccount(providerAction.action)}
                type="button"
              >
                <Icon name="external" size={21} />
                <strong>
                  {openingAction === providerAction.action ? "Opening…" : providerAction.label}
                </strong>
                <span>{providerAction.description}</span>
              </button>
            ),
          )}
        </div>
        {accountError && (
          <p className="inline-error" role="alert">
            {accountError}
          </p>
        )}
      </div>
    </Dialog>
  );
}
