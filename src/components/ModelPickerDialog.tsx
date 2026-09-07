import { useMemo, useRef, useState } from "react";

import type { ModelCatalog, ModelSelection, ProviderStatus } from "../types/providers";
import { sameSelection } from "../types/providers";
import { Dialog } from "./Dialog";
import { Icon } from "./Icon";

interface ModelPickerDialogProps {
  catalog: ModelCatalog;
  currentSelection: ModelSelection;
  providerStatuses: ProviderStatus[];
  onChoose: (selection: ModelSelection) => void;
  onClose: () => void;
}

export function ModelPickerDialog({
  catalog,
  currentSelection,
  providerStatuses,
  onChoose,
  onClose,
}: ModelPickerDialogProps) {
  const searchRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<ModelSelection>(currentSelection);
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const groups = useMemo(
    () =>
      catalog.providers
        .map((provider) => ({
          provider,
          models: provider.models.filter((model) =>
            `${provider.displayName} ${model.displayName} ${model.id}`
              .toLocaleLowerCase()
              .includes(normalizedQuery),
          ),
        }))
        .filter((group) => group.models.length > 0),
    [catalog.providers, normalizedQuery],
  );
  const resultCount = groups.reduce((total, group) => total + group.models.length, 0);
  const selectedVisible = groups.some(({ provider, models }) =>
    models.some((model) => provider.id === selected.providerId && model.id === selected.modelId),
  );

  return (
    <Dialog
      label="Choose model"
      description="Choose from models in Aster's curated catalog."
      initialFocusRef={searchRef}
      onClose={onClose}
      size="large"
    >
      <div className="model-picker">
        <label className="catalog-search">
          <Icon name="search" size={16} />
          <span className="visually-hidden">Search catalog models</span>
          <input
            aria-describedby="catalog-result-count"
            onChange={(event) => {
              setQuery(event.target.value);
            }}
            placeholder="Search models or providers"
            ref={searchRef}
            type="search"
            value={query}
          />
        </label>
        <p className="visually-hidden" id="catalog-result-count" aria-live="polite">
          {resultCount} catalog {resultCount === 1 ? "model" : "models"}
        </p>

        <div className="catalog-groups">
          {groups.length === 0 ? (
            <div className="catalog-empty">
              <Icon name="search" size={21} />
              <p>No catalog models match your search.</p>
            </div>
          ) : (
            groups.map(({ provider, models }) => {
              const status = providerStatuses.find((item) => item.providerId === provider.id);
              return (
                <section className="catalog-group" key={provider.id}>
                  <div className="catalog-provider-heading">
                    <div>
                      <h3>{provider.displayName}</h3>
                      {provider.regionLabel && <span>{provider.regionLabel} region</span>}
                    </div>
                    <span className={`provider-mini-status ${status?.configured ? "ready" : ""}`}>
                      {status?.configured ? "Key configured" : "Key not configured"}
                    </span>
                  </div>
                  <div className="catalog-models">
                    {models.map((model) => {
                      const selection = {
                        providerId: provider.id,
                        modelId: model.id,
                      } as ModelSelection;
                      const checked = sameSelection(selected, selection);
                      return (
                        <label
                          className={`catalog-model ${checked ? "selected" : ""}`}
                          key={model.id}
                        >
                          <input
                            checked={checked}
                            name="catalog-model"
                            onChange={() => {
                              setSelected(selection);
                            }}
                            type="radio"
                          />
                          <span className="catalog-model-copy">
                            <strong>{model.displayName}</strong>
                            <small>{model.id}</small>
                          </span>
                          {model.delivery === "hosted-prototype" && (
                            <span className="prototype-badge">Hosted prototype</span>
                          )}
                          {checked && <Icon name="check" size={17} />}
                        </label>
                      );
                    })}
                  </div>
                </section>
              );
            })
          )}
        </div>
        <div className="dialog-actions model-picker-actions">
          <button className="button secondary" type="button" onClick={onClose}>
            Cancel
          </button>
          <button
            className="button primary"
            disabled={
              resultCount === 0 || !selectedVisible || sameSelection(selected, currentSelection)
            }
            type="button"
            onClick={() => {
              if (selectedVisible) onChoose(selected);
            }}
          >
            Use this model
          </button>
        </div>
      </div>
    </Dialog>
  );
}
