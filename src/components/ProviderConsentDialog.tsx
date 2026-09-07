import { useRef } from "react";

import type { CatalogProvider, ModelSelection } from "../types/providers";
import { Dialog } from "./Dialog";
import { Icon } from "./Icon";

interface ProviderConsentDialogProps {
  provider: CatalogProvider;
  selection: ModelSelection;
  modelName: string;
  onCancel: () => void;
  onContinue: () => void;
  working: boolean;
}

export function ProviderConsentDialog({
  provider,
  selection,
  modelName,
  onCancel,
  onContinue,
  working,
}: ProviderConsentDialogProps) {
  const cancelRef = useRef<HTMLButtonElement>(null);
  return (
    <Dialog
      label={`Messages will be processed by ${provider.displayName}`}
      description={`Review the external-processing boundary before using ${modelName}.`}
      initialFocusRef={cancelRef}
      onClose={onCancel}
      size="medium"
    >
      <div className="provider-notice provider-consent-notice">
        <div className="notice-icon">
          <Icon name="shield" size={20} />
        </div>
        <div>
          <h3>{provider.displayName} is an external provider</h3>
          <p>{provider.processingNotice}</p>
          {selection.providerId === "alibaba-us" && (
            <p className="notice-emphasis">
              This selection always uses the fixed United States region.
            </p>
          )}
          {selection.providerId === "nvidia" && (
            <p className="notice-emphasis">
              Hosted prototype for evaluation; not a production deployment or evidence of NVIDIA AI
              Enterprise coverage.
            </p>
          )}
        </div>
      </div>
      <p className="local-disclosure">
        Aster sends only the relevant visible conversation context. Local history remains on this
        device.
      </p>
      <div className="dialog-actions">
        <button
          className="button secondary"
          disabled={working}
          onClick={onCancel}
          ref={cancelRef}
          type="button"
        >
          Cancel
        </button>
        <button className="button primary" disabled={working} onClick={onContinue} type="button">
          {working ? "Acknowledging…" : "Continue and send"}
        </button>
      </div>
    </Dialog>
  );
}
