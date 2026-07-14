import type { ModelCatalog, ProviderId, ProviderStatus } from "../types/providers";
import { Icon } from "./Icon";

interface ProviderSettingsProps {
  catalog: ModelCatalog;
  providerStatuses: ProviderStatus[];
  runtime: "tauri" | "browser-demo";
  workingProviderId: ProviderId | null;
  onPromptCredential: (providerId: ProviderId) => void;
  onRequestRemove: (providerId: ProviderId) => void;
}

function reachabilityLabel(status: ProviderStatus | undefined) {
  if (status?.reachability === "reachable") return "Reachable";
  if (status?.reachability === "unreachable") return "Last request could not connect";
  return "Not checked";
}

export function ProviderSettings({
  catalog,
  providerStatuses,
  runtime,
  workingProviderId,
  onPromptCredential,
  onRequestRemove,
}: ProviderSettingsProps) {
  return (
    <div className="provider-settings">
      <div className="provider-settings-intro">
        <Icon name="key" size={19} />
        <p>
          {runtime === "tauri"
            ? "Credentials are stored separately in Windows Credential Manager. API keys never enter this interface."
            : "Synthetic provider catalog. Credentials are unavailable in browser demo."}
        </p>
      </div>
      <div className="provider-settings-grid">
        {catalog.providers.map((provider) => {
          const status = providerStatuses.find((candidate) => candidate.providerId === provider.id);
          const working = workingProviderId === provider.id;
          return (
            <article className="provider-card" id={`provider-${provider.id}`} key={provider.id}>
              <header>
                <div>
                  <h4>{provider.displayName}</h4>
                  <p>
                    {provider.models.length} models
                    {provider.regionLabel ? ` · ${provider.regionLabel} region` : ""}
                  </p>
                </div>
                <span
                  className={`status-chip ${runtime === "browser-demo" ? "demo" : status?.configured ? "positive" : "neutral"}`}
                >
                  <span aria-hidden="true" className="status-dot" />
                  {runtime === "browser-demo"
                    ? "Demo only"
                    : status?.configured
                      ? "Key configured"
                      : "Setup required"}
                </span>
              </header>
              {provider.id === "nvidia" && (
                <p className="provider-card-note">
                  Hosted prototype for evaluation, not a production deployment.
                </p>
              )}
              {runtime === "tauri" ? (
                <>
                  <dl className="provider-card-status">
                    <div>
                      <dt>Connection</dt>
                      <dd>{reachabilityLabel(status)}</dd>
                    </div>
                    <div>
                      <dt>Processing notice</dt>
                      <dd>{status?.noticeAcknowledged ? "Acknowledged" : "Review required"}</dd>
                    </div>
                  </dl>
                  <div className="provider-card-actions">
                    <button
                      aria-label={`${status?.configured ? "Replace" : "Add"} ${provider.displayName} API key`}
                      className="button secondary"
                      disabled={workingProviderId !== null}
                      onClick={() => {
                        onPromptCredential(provider.id);
                      }}
                      type="button"
                    >
                      <Icon name="key" size={15} />
                      {working
                        ? "Opening native prompt…"
                        : status?.configured
                          ? "Replace API key"
                          : "Add API key"}
                    </button>
                    {status?.configured && (
                      <button
                        aria-label={`Remove ${provider.displayName} API key`}
                        className="button danger-quiet provider-remove-key"
                        disabled={workingProviderId !== null}
                        onClick={() => {
                          onRequestRemove(provider.id);
                        }}
                        type="button"
                      >
                        <Icon name="trash" size={15} /> Remove key
                      </button>
                    )}
                  </div>
                </>
              ) : (
                <p className="provider-card-note">
                  Visual catalog preview only. No key can be entered, stored, or transmitted.
                </p>
              )}
            </article>
          );
        })}
      </div>
    </div>
  );
}
