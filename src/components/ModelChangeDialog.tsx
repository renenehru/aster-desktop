import { useRef } from "react";

import type { ModelCatalog, ModelSelection } from "../types/providers";
import { selectionLabel } from "../types/providers";
import { Dialog } from "./Dialog";

interface ModelChangeDialogProps {
  catalog: ModelCatalog;
  currentSelection: ModelSelection;
  requestedSelection: ModelSelection;
  onCancel: () => void;
  onConfirm: () => void;
  working: boolean;
}

export function ModelChangeDialog({
  catalog,
  currentSelection,
  requestedSelection,
  onCancel,
  onConfirm,
  working,
}: ModelChangeDialogProps) {
  const cancelRef = useRef<HTMLButtonElement>(null);
  return (
    <Dialog
      label="Start a new chat?"
      description={`${selectionLabel(catalog, currentSelection)} is locked to this conversation. ${selectionLabel(catalog, requestedSelection)} can only be used in a new chat.`}
      initialFocusRef={cancelRef}
      onClose={onCancel}
      size="small"
    >
      <div className="model-change-copy">
        <p>The current conversation and its usage record will remain unchanged.</p>
      </div>
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
        <button className="button primary" disabled={working} onClick={onConfirm} type="button">
          {working ? "Starting chat…" : "Start a new chat with this model"}
        </button>
      </div>
    </Dialog>
  );
}
