import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ChangeEvent,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import asterMark from "./assets/aster-mark.svg";
import { Dialog } from "./components/Dialog";
import { Icon } from "./components/Icon";
import { Markdown } from "./components/Markdown";
import { assistantAdapter } from "./services/assistantAdapter";
import type {
  AppStatus,
  ChatMessage,
  ChatStreamEvent,
  Conversation,
  ConversationSummary,
  CredentialStatus,
  ExportResult,
  ReasoningMode,
} from "./types/chat";

const MAX_MESSAGE_LENGTH = 32_000;

type ActiveGeneration = {
  prompt: string;
  requestId: string;
  state: "connecting" | "streaming" | "stopping";
};

type PendingSubmission = {
  content: string;
  regenerateFromMessageId?: string;
};

type PendingConsent = {
  submission: PendingSubmission;
  conversation: Conversation;
};

type PublicError = {
  code?: string;
  message: string;
  retryable?: boolean;
};

type ScopedGenerationError = {
  conversationId: string;
  error: PublicError;
};

const emptyCredential: CredentialStatus = { configured: false, source: "none" };
const emptyStatus: AppStatus = {
  mode: assistantAdapter.runtime === "tauri" ? "desktop" : "demo",
  online: true,
  databaseReady: true,
};

function asRecord(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === "object" ? (value as Record<string, unknown>) : {};
}

function publicError(value: unknown): PublicError {
  if (value instanceof Error) return { message: value.message };
  if (typeof value === "string") return { message: value };
  const raw = asRecord(value);
  return {
    code: typeof raw.code === "string" ? raw.code : undefined,
    message:
      typeof raw.message === "string" ? raw.message : "Aster could not complete this action.",
    retryable: typeof raw.retryable === "boolean" ? raw.retryable : undefined,
  };
}

function sortSummaries(items: ConversationSummary[]) {
  return [...items].sort((left, right) => {
    const dateDifference = Date.parse(right.updatedAt) - Date.parse(left.updatedAt);
    return dateDifference || left.id.localeCompare(right.id);
  });
}

function groupLabel(value: string) {
  const date = new Date(value);
  const today = new Date();
  const startToday = new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime();
  const startDate = new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  const days = Math.round((startToday - startDate) / 86_400_000);
  if (days <= 0) return "Today";
  if (days === 1) return "Yesterday";
  if (days < 7) return "Previous 7 days";
  if (date.getFullYear() === today.getFullYear()) {
    return date.toLocaleDateString(undefined, { month: "long" });
  }
  return date.toLocaleDateString(undefined, { month: "long", year: "numeric" });
}

function formatTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "Unknown time";
  return date.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
}

async function copyPlainText(content: string) {
  await navigator.clipboard.writeText(content);
}

function downloadDemoExport(serialized: string, title: string) {
  const safeName =
    title
      .trim()
      .replace(/[^a-zA-Z0-9 _-]/g, "")
      .replace(/\s+/g, "-")
      .slice(0, 64) || "conversation";
  const url = URL.createObjectURL(new Blob([serialized], { type: "application/json" }));
  const anchor = document.createElement("a");
  anchor.download = `${safeName}.aster.json`;
  anchor.href = url;
  anchor.click();
  window.setTimeout(() => {
    URL.revokeObjectURL(url);
  }, 0);
}

function isExportResult(value: unknown): value is ExportResult {
  return typeof asRecord(value).cancelled === "boolean";
}

function streamMessageId(conversationId: string) {
  return `stream-${conversationId}`;
}

interface SettingsDialogProps {
  appStatus: AppStatus;
  credential: CredentialStatus;
  currentConversation: Conversation | null;
  onClose: () => void;
  onRequestDeleteKey: () => void;
  onExport: () => Promise<void>;
  onImport: () => Promise<void>;
  onImportFile: (event: ChangeEvent<HTMLInputElement>) => Promise<void>;
  onPromptSaveKey: () => Promise<void>;
  runtime: "tauri" | "browser-demo";
}

function SettingsDialog({
  appStatus,
  credential,
  currentConversation,
  onClose,
  onRequestDeleteKey,
  onExport,
  onImport,
  onImportFile,
  onPromptSaveKey,
  runtime,
}: SettingsDialogProps) {
  const importInputRef = useRef<HTMLInputElement>(null);
  const [working, setWorking] = useState<string | null>(null);
  const [inlineError, setInlineError] = useState<string | null>(null);

  const run = async (name: string, action: () => Promise<void>) => {
    setWorking(name);
    setInlineError(null);
    try {
      await action();
    } catch (error) {
      setInlineError(publicError(error).message);
    } finally {
      setWorking(null);
    }
  };

  return (
    <Dialog
      label="Settings"
      description="Security, local data, and application status"
      onClose={onClose}
      size="large"
    >
      <div className="settings-layout">
        <nav aria-label="Settings sections" className="settings-nav">
          <a href="#provider-settings">
            <Icon name="key" size={16} /> Provider
          </a>
          <a href="#data-settings">
            <Icon name="archive" size={16} /> Local data
          </a>
          <a href="#about-settings">
            <Icon name="shield" size={16} /> About
          </a>
        </nav>
        <div className="settings-content">
          <section className="settings-section" id="provider-settings">
            <div className="section-heading-row">
              <div>
                <p className="eyebrow">Provider</p>
                <h3>Z.AI connection</h3>
              </div>
              {runtime === "tauri" ? (
                <span className={`status-chip ${credential.configured ? "positive" : "neutral"}`}>
                  <span aria-hidden="true" className="status-dot" />
                  {credential.configured ? "Key configured" : "Setup required"}
                </span>
              ) : (
                <span className="status-chip demo">
                  <span aria-hidden="true" className="status-dot" /> Demo only
                </span>
              )}
            </div>
            {runtime === "tauri" ? (
              <>
                <p className="settings-copy">
                  Aster opens a native Windows credential prompt owned by the Rust process. The key
                  never enters this interface and is stored in Windows Credential Manager.
                </p>
                <button
                  className="button primary"
                  disabled={working !== null}
                  type="button"
                  onClick={() => {
                    void run("key", onPromptSaveKey);
                  }}
                >
                  <Icon name="key" size={16} />
                  {working === "key"
                    ? "Opening native prompt…"
                    : credential.configured
                      ? "Replace API key"
                      : "Add API key"}
                </button>
                {credential.configured && (
                  <button
                    className="button danger-quiet"
                    disabled={working !== null}
                    type="button"
                    onClick={onRequestDeleteKey}
                  >
                    <Icon name="trash" size={16} />
                    Remove saved key
                  </button>
                )}
              </>
            ) : (
              <div className="demo-security-panel">
                <Icon name="shield" size={22} />
                <div>
                  <strong>No credentials in browser demo</strong>
                  <p>
                    This preview has no provider connection and cannot accept, store, or transmit an
                    API key. Install and open the Windows desktop app to configure Z.AI through the
                    native credential workflow.
                  </p>
                </div>
              </div>
            )}
          </section>

          <section className="settings-section" id="data-settings">
            <p className="eyebrow">Local data</p>
            <h3>Conversation history</h3>
            <p className="settings-copy">
              Desktop history is stored locally in SQLite. It is not synchronized, and Aster does
              not claim application-level database encryption. Exports contain sensitive plaintext
              conversation content.
            </p>
            <div className="settings-actions">
              <button
                className="button secondary"
                disabled={working !== null}
                type="button"
                onClick={() => {
                  if (runtime === "browser-demo") importInputRef.current?.click();
                  else void run("import", onImport);
                }}
              >
                <Icon name="upload" size={16} />
                {working === "import" ? "Importing…" : "Import conversations"}
              </button>
              <button
                className="button secondary"
                disabled={!currentConversation || working !== null}
                type="button"
                onClick={() => void run("export", onExport)}
              >
                <Icon name="download" size={16} />
                {working === "export" ? "Exporting…" : "Export current chat"}
              </button>
              {runtime === "browser-demo" && (
                <input
                  accept="application/json,.json"
                  aria-label="Choose an Aster JSON export"
                  className="visually-hidden"
                  onChange={(event) => void run("import", () => onImportFile(event))}
                  ref={importInputRef}
                  type="file"
                />
              )}
            </div>
            {runtime === "browser-demo" && (
              <p className="field-help">
                Demo imports and exports stay in this browser tab and reset when it reloads.
              </p>
            )}
          </section>

          <section className="settings-section" id="about-settings">
            <p className="eyebrow">Application</p>
            <h3>Aster {appStatus.version ? `v${appStatus.version}` : "MVP v1"}</h3>
            <div className="status-grid">
              <span>Runtime</span>
              <strong>{runtime === "tauri" ? "Windows desktop" : "Browser demo"}</strong>
              <span>Model contract</span>
              <strong>glm-5.1</strong>
              <span>Local database</span>
              <strong>
                {runtime === "tauri"
                  ? appStatus.databaseReady
                    ? "Ready"
                    : "Unavailable"
                  : "In memory"}
              </strong>
              <span>Connection</span>
              <strong>
                {runtime === "browser-demo"
                  ? "Demo only"
                  : appStatus.providerReachability === "reachable"
                    ? "Reachable"
                    : appStatus.providerReachability === "unreachable"
                      ? "Unreachable"
                      : "Not checked"}
              </strong>
              <span>Telemetry</span>
              <strong>None</strong>
            </div>
          </section>
          {inlineError && (
            <p className="inline-error" role="alert">
              {inlineError}
            </p>
          )}
        </div>
      </div>
    </Dialog>
  );
}

interface ComposerProps {
  active: ActiveGeneration | undefined;
  credentialConfigured: boolean;
  draft: string;
  editing: ChatMessage | null;
  mode: ReasoningMode;
  providerUnreachable: boolean;
  onCancelEdit: () => void;
  onChange: (value: string) => void;
  onModeChange: (mode: ReasoningMode) => void;
  onOpenSettings: () => void;
  onStop: () => Promise<void>;
  onSubmit: () => void;
  runtime: "tauri" | "browser-demo";
  textareaRef: React.RefObject<HTMLTextAreaElement | null>;
}

function Composer({
  active,
  credentialConfigured,
  draft,
  editing,
  mode,
  providerUnreachable,
  onCancelEdit,
  onChange,
  onModeChange,
  onOpenSettings,
  onStop,
  onSubmit,
  runtime,
  textareaRef,
}: ComposerProps) {
  const isComposing = useRef(false);
  const blockedForCredential = runtime === "tauri" && !credentialConfigured;
  const submitDisabled = !draft.trim() || draft.length > MAX_MESSAGE_LENGTH || Boolean(active);

  const onKeyDown = (event: ReactKeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && !event.shiftKey && !isComposing.current) {
      event.preventDefault();
      if (!active && !submitDisabled) onSubmit();
    }
  };

  return (
    <div className="composer-region">
      {editing && (
        <div className="edit-banner" role="status">
          <Icon name="edit" size={15} />
          <span>
            <strong>Editing an earlier message.</strong> Resending replaces it and removes every
            later reply.
          </span>
          <button type="button" onClick={onCancelEdit}>
            Cancel edit
          </button>
        </div>
      )}
      {blockedForCredential && (
        <button className="credential-callout" type="button" onClick={onOpenSettings}>
          <Icon name="key" size={16} />
          <span>
            <strong>Connect Z.AI to send</strong>
            <small>Add an API key securely in Settings</small>
          </span>
          <span className="callout-action">Open settings</span>
        </button>
      )}
      <div className={`composer ${active ? "composer-active" : ""}`}>
        <textarea
          aria-describedby="composer-hint"
          aria-label={editing ? "Edit message" : "Message Aster"}
          maxLength={MAX_MESSAGE_LENGTH + 1}
          onChange={(event) => {
            onChange(event.target.value);
          }}
          onCompositionEnd={() => {
            isComposing.current = false;
          }}
          onCompositionStart={() => {
            isComposing.current = true;
          }}
          onKeyDown={onKeyDown}
          placeholder={
            providerUnreachable
              ? "Z.AI is currently unreachable"
              : editing
                ? "Revise your message"
                : "Ask Aster anything"
          }
          ref={textareaRef}
          rows={1}
          value={draft}
        />
        <div className="composer-toolbar">
          <div className="composer-tools">
            <button
              aria-disabled="true"
              className="icon-button muted"
              title="Attachments are coming soon"
              type="button"
              onClick={() => undefined}
            >
              <Icon name="plus" size={19} />
              <span className="visually-hidden">Attachments, coming soon</span>
            </button>
            <label className="mode-select">
              <Icon name="spark" size={15} />
              <span className="visually-hidden">Reasoning mode</span>
              <select
                aria-label="Reasoning mode"
                onChange={(event) => {
                  onModeChange(event.target.value as ReasoningMode);
                }}
                value={mode}
              >
                <option value="fast">Fast · concise</option>
                <option value="standard">Standard · balanced</option>
                <option value="deep">Deep · longer output</option>
              </select>
              <Icon name="chevron-down" size={13} />
            </label>
          </div>
          <div className="composer-submit-row">
            {draft.length > MAX_MESSAGE_LENGTH - 1_000 && (
              <span
                className={
                  draft.length > MAX_MESSAGE_LENGTH ? "character-count over" : "character-count"
                }
              >
                {draft.length.toLocaleString()}/{MAX_MESSAGE_LENGTH.toLocaleString()}
              </span>
            )}
            {active ? (
              <button
                className="send-button stop-button"
                disabled={active.state === "stopping"}
                type="button"
                onClick={() => void onStop()}
                aria-label={active.state === "stopping" ? "Stopping generation" : "Stop generation"}
              >
                <Icon name="stop" size={16} />
              </button>
            ) : (
              <button
                className="send-button"
                disabled={submitDisabled}
                type="button"
                onClick={onSubmit}
                aria-label={editing ? "Resend edited message" : "Send message"}
              >
                <Icon name="arrow-up" size={17} />
              </button>
            )}
          </div>
        </div>
      </div>
      <p className="composer-hint" id="composer-hint">
        {runtime === "browser-demo"
          ? "In-memory demo · no messages leave this tab"
          : "Enter to send · Shift+Enter for a new line · Messages are processed by Z.AI"}
      </p>
    </div>
  );
}

export default function App() {
  const runtime = assistantAdapter.runtime;
  const [appStatus, setAppStatus] = useState<AppStatus>(emptyStatus);
  const [credential, setCredential] = useState<CredentialStatus>(emptyCredential);
  const [summaries, setSummaries] = useState<ConversationSummary[]>([]);
  const [conversation, setConversation] = useState<Conversation | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingConversation, setLoadingConversation] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [searchOpen, setSearchOpen] = useState(false);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [confirmKeyRemoval, setConfirmKeyRemoval] = useState(false);
  const [removingKey, setRemovingKey] = useState(false);
  const [menuFor, setMenuFor] = useState<string | null>(null);
  const [renameTarget, setRenameTarget] = useState<ConversationSummary | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<ConversationSummary | null>(null);
  const [pendingNotice, setPendingNotice] = useState<PendingConsent | null>(null);
  const [externalNoticeAccepted, setExternalNoticeAccepted] = useState(false);
  const [acknowledgingExternal, setAcknowledgingExternal] = useState(false);
  const [draft, setDraft] = useState("");
  const [mode, setMode] = useState<ReasoningMode>("standard");
  const [editing, setEditing] = useState<ChatMessage | null>(null);
  const [activeByConversation, setActiveByConversation] = useState<
    Record<string, ActiveGeneration>
  >({});
  const [generationError, setGenerationError] = useState<ScopedGenerationError | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [onboardingVisible, setOnboardingVisible] = useState(true);

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const renameRef = useRef<HTMLInputElement>(null);
  const selectedIdRef = useRef<string | null>(null);
  const activeRef = useRef(new Map<string, ActiveGeneration>());
  const pendingConversationRef = useRef(new Set<string>());
  const pendingPromptRef = useRef(new Map<string, string>());
  const snapshotsRef = useRef(new Map<string, Conversation>());
  const partialContentRef = useRef(new Map<string, string>());
  const conversationFetchVersionRef = useRef(0);
  const streamSequenceRef = useRef(new Map<string, number>());
  const failedSequenceRequestsRef = useRef(new Set<string>());
  const sequenceRecoveryTimersRef = useRef(new Map<string, number>());

  useEffect(() => {
    selectedIdRef.current = selectedId;
  }, [selectedId]);

  useEffect(() => {
    if (!toast) return;
    const timeout = window.setTimeout(() => {
      setToast(null);
    }, 2400);
    return () => {
      window.clearTimeout(timeout);
    };
  }, [toast]);

  useEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;
    textarea.style.height = "0px";
    textarea.style.height = `${String(Math.min(textarea.scrollHeight, 200))}px`;
  }, [draft]);

  const setActive = useCallback((conversationId: string, next?: ActiveGeneration) => {
    if (next) activeRef.current.set(conversationId, next);
    else activeRef.current.delete(conversationId);
    setActiveByConversation((current) => {
      if (next) return { ...current, [conversationId]: next };
      return Object.fromEntries(Object.entries(current).filter(([key]) => key !== conversationId));
    });
  }, []);

  const refreshSummaries = useCallback(async () => {
    const next = sortSummaries(await assistantAdapter.listConversations());
    setSummaries(next);
    return next;
  }, []);

  const loadConversation = useCallback(async (conversationId: string) => {
    const loadToken = ++conversationFetchVersionRef.current;
    setLoadingConversation(true);
    setGenerationError((current) => (current?.conversationId === conversationId ? null : current));
    try {
      const next = await assistantAdapter.getConversation(conversationId);
      if (
        loadToken !== conversationFetchVersionRef.current ||
        selectedIdRef.current !== conversationId
      )
        return;
      const active = activeRef.current.get(conversationId);
      const partial = partialContentRef.current.get(conversationId) ?? "";
      const hydrated = active
        ? {
            ...next,
            messages: [
              ...next.messages,
              {
                id: streamMessageId(conversationId),
                conversationId,
                role: "assistant" as const,
                content: partial,
                createdAt: new Date().toISOString(),
                status: "streaming" as const,
              },
            ],
          }
        : next;
      setConversation(hydrated);
      setMode(next.reasoningMode ?? "standard");
      setLoadError(null);
    } catch (error) {
      if (
        loadToken !== conversationFetchVersionRef.current ||
        selectedIdRef.current !== conversationId
      )
        return;
      setConversation(null);
      setLoadError(publicError(error).message);
    } finally {
      if (loadToken === conversationFetchVersionRef.current) setLoadingConversation(false);
    }
  }, []);

  const reconcileConversation = useCallback(
    async (conversationId: string, releaseActive = false) => {
      const fetchVersion = ++conversationFetchVersionRef.current;
      try {
        const authoritative = await assistantAdapter.getConversation(conversationId);
        if (
          fetchVersion === conversationFetchVersionRef.current &&
          selectedIdRef.current === conversationId
        ) {
          setConversation(authoritative);
          setMode(authoritative.reasoningMode ?? "standard");
        }
      } catch (error) {
        if (
          fetchVersion === conversationFetchVersionRef.current &&
          selectedIdRef.current === conversationId
        ) {
          setLoadError(publicError(error).message);
        }
      }
      try {
        await refreshSummaries();
      } finally {
        if (releaseActive) setActive(conversationId);
      }
    },
    [refreshSummaries, setActive],
  );

  const handleStream = useCallback(
    (event: ChatStreamEvent) => {
      if (failedSequenceRequestsRef.current.has(event.requestId)) {
        if (event.kind === "completed" || event.kind === "cancelled" || event.kind === "error") {
          failedSequenceRequestsRef.current.delete(event.requestId);
          const timer = sequenceRecoveryTimersRef.current.get(event.requestId);
          if (timer !== undefined) window.clearTimeout(timer);
          sequenceRecoveryTimersRef.current.delete(event.requestId);
          void reconcileConversation(event.conversationId, true);
        }
        return;
      }
      const active = activeRef.current.get(event.conversationId);
      const isPending = pendingConversationRef.current.has(event.conversationId);
      if (active && active.requestId && active.requestId !== event.requestId) return;
      if (!active && !isPending) return;
      const expectedSequence = streamSequenceRef.current.get(event.requestId) ?? 0;
      if (event.sequence < expectedSequence) return;
      if (event.sequence > expectedSequence) {
        failedSequenceRequestsRef.current.add(event.requestId);
        pendingConversationRef.current.delete(event.conversationId);
        pendingPromptRef.current.delete(event.conversationId);
        snapshotsRef.current.delete(event.conversationId);
        partialContentRef.current.delete(event.conversationId);
        streamSequenceRef.current.delete(event.requestId);
        setGenerationError({
          conversationId: event.conversationId,
          error: {
            code: "stream_sequence_error",
            message: "The response stream arrived out of order. Retry the response.",
            retryable: true,
          },
        });
        void assistantAdapter.cancelGeneration(event.requestId).catch(() => undefined);
        const recoveryTimer = window.setTimeout(() => {
          if (failedSequenceRequestsRef.current.delete(event.requestId)) {
            void reconcileConversation(event.conversationId, true);
          }
          sequenceRecoveryTimersRef.current.delete(event.requestId);
        }, 1_500);
        sequenceRecoveryTimersRef.current.set(event.requestId, recoveryTimer);
        return;
      }
      streamSequenceRef.current.set(event.requestId, expectedSequence + 1);

      const prompt = active?.prompt ?? pendingPromptRef.current.get(event.conversationId) ?? "";
      const tracked: ActiveGeneration = {
        prompt,
        requestId: event.requestId,
        state: event.kind === "started" ? "connecting" : "streaming",
      };

      if (event.kind === "started") {
        setActive(event.conversationId, tracked);
        return;
      }

      if (event.kind === "delta") {
        const delta = event.delta;
        if (!delta) return;
        partialContentRef.current.set(
          event.conversationId,
          (partialContentRef.current.get(event.conversationId) ?? "") + delta,
        );
        setActive(event.conversationId, { ...tracked, state: "streaming" });
        setConversation((current) => {
          if (current?.id !== event.conversationId) return current;
          return {
            ...current,
            messages: current.messages.map((message) =>
              message.id === streamMessageId(event.conversationId)
                ? { ...message, content: message.content + delta }
                : message,
            ),
          };
        });
        return;
      }

      pendingConversationRef.current.delete(event.conversationId);
      pendingPromptRef.current.delete(event.conversationId);
      snapshotsRef.current.delete(event.conversationId);
      partialContentRef.current.delete(event.conversationId);
      streamSequenceRef.current.delete(event.requestId);
      if (!active) setActive(event.conversationId, tracked);

      const completedMessage = event.message;
      if (event.kind === "completed" && completedMessage) {
        setConversation((current) => {
          if (current?.id !== event.conversationId) return current;
          return {
            ...current,
            updatedAt: completedMessage.createdAt,
            messages: current.messages.some(
              (message) => message.id === streamMessageId(event.conversationId),
            )
              ? current.messages.map((message) =>
                  message.id === streamMessageId(event.conversationId) ? completedMessage : message,
                )
              : [...current.messages, completedMessage],
          };
        });
        setGenerationError((current) =>
          current?.conversationId === event.conversationId ? null : current,
        );
        void reconcileConversation(event.conversationId, true);
        window.setTimeout(() => textareaRef.current?.focus(), 0);
        return;
      }

      if (event.kind === "cancelled") {
        setConversation((current) => {
          if (current?.id !== event.conversationId) return current;
          return {
            ...current,
            messages: current.messages.map((message) =>
              message.id === streamMessageId(event.conversationId)
                ? (event.message ?? { ...message, status: "cancelled" })
                : message,
            ),
          };
        });
        setToast("Generation stopped");
        void reconcileConversation(event.conversationId, true);
        window.setTimeout(() => textareaRef.current?.focus(), 0);
        return;
      }

      if (event.kind === "error") {
        const error = {
          code: event.errorCode,
          message: event.error ?? "Aster could not finish this response.",
          retryable: event.retryable,
        };
        setGenerationError({ conversationId: event.conversationId, error });
        setConversation((current) => {
          if (current?.id !== event.conversationId) return current;
          return {
            ...current,
            messages: current.messages.map((message) =>
              message.id === streamMessageId(event.conversationId)
                ? (event.message ?? { ...message, status: "error" })
                : message,
            ),
          };
        });
        if (selectedIdRef.current === event.conversationId)
          setDraft((current) => current || prompt);
        void reconcileConversation(event.conversationId, true);
      }
    },
    [reconcileConversation, setActive],
  );

  const handleUnscopedStreamError = useCallback(() => {
    const affectedConversations = new Set([
      ...activeRef.current.keys(),
      ...pendingConversationRef.current,
    ]);
    for (const conversationId of affectedConversations) {
      const active = activeRef.current.get(conversationId);
      if (active?.requestId) {
        failedSequenceRequestsRef.current.add(active.requestId);
        streamSequenceRef.current.delete(active.requestId);
        void assistantAdapter.cancelGeneration(active.requestId).catch(() => undefined);
      }
      pendingConversationRef.current.delete(conversationId);
      pendingPromptRef.current.delete(conversationId);
      snapshotsRef.current.delete(conversationId);
      partialContentRef.current.delete(conversationId);
      if (selectedIdRef.current === conversationId) {
        setGenerationError({
          conversationId,
          error: {
            code: "malformed_stream_event",
            message: "Aster rejected an invalid response stream event. Retry the response.",
            retryable: true,
          },
        });
      }
      void reconcileConversation(conversationId, true).finally(() => {
        if (active?.requestId) failedSequenceRequestsRef.current.delete(active.requestId);
      });
    }
  }, [reconcileConversation]);

  useEffect(() => {
    let alive = true;
    let unsubscribe: (() => void) | undefined;
    const initialize = async () => {
      try {
        unsubscribe = await assistantAdapter.onChatStream(handleStream, handleUnscopedStreamError);
        const [nextStatus, nextCredential, nextSummaries] = await Promise.all([
          assistantAdapter.appStatus(),
          assistantAdapter.credentialStatus(),
          assistantAdapter.listConversations(),
        ]);
        if (!alive) return;
        setAppStatus(nextStatus);
        setExternalNoticeAccepted(
          runtime === "browser-demo" || nextStatus.externalProcessingAcknowledged === true,
        );
        setCredential(runtime === "browser-demo" ? emptyCredential : nextCredential);
        setSummaries(sortSummaries(nextSummaries));
      } catch (error) {
        if (alive) setLoadError(publicError(error).message);
      } finally {
        if (alive) setLoading(false);
      }
    };
    void initialize();
    return () => {
      alive = false;
      unsubscribe?.();
    };
  }, [handleStream, handleUnscopedStreamError, runtime]);

  const groupedSummaries = useMemo(() => {
    const filtered = summaries.filter((item) =>
      item.title.toLowerCase().includes(query.trim().toLowerCase()),
    );
    const groups = new Map<string, ConversationSummary[]>();
    for (const item of filtered) {
      const label = groupLabel(item.updatedAt);
      groups.set(label, [...(groups.get(label) ?? []), item]);
    }
    return [...groups.entries()];
  }, [query, summaries]);

  const selectConversation = useCallback(
    async (id: string) => {
      selectedIdRef.current = id;
      setSelectedId(id);
      setSidebarOpen(false);
      setMenuFor(null);
      setEditing(null);
      setDraft("");
      await loadConversation(id);
    },
    [loadConversation],
  );

  const createConversation = useCallback(async () => {
    try {
      const next = await assistantAdapter.createConversation();
      setSummaries((current) => sortSummaries([{ ...next, messageCount: 0 }, ...current]));
      selectedIdRef.current = next.id;
      setSelectedId(next.id);
      setConversation(next);
      setMode(next.reasoningMode ?? "standard");
      setDraft("");
      setEditing(null);
      setGenerationError(null);
      setSidebarOpen(false);
      window.setTimeout(() => textareaRef.current?.focus(), 0);
      return next;
    } catch (error) {
      setToast(publicError(error).message);
      return null;
    }
  }, []);

  useEffect(() => {
    const shortcuts = (event: KeyboardEvent) => {
      if (settingsOpen || confirmKeyRemoval || renameTarget || deleteTarget || pendingNotice)
        return;
      if (!(event.ctrlKey || event.metaKey)) return;
      if (event.key.toLowerCase() === "k") {
        event.preventDefault();
        setSearchOpen(true);
        window.setTimeout(() => searchRef.current?.focus(), 0);
      } else if (event.key === ",") {
        event.preventDefault();
        setSettingsOpen(true);
      } else if (event.key.toLowerCase() === "n") {
        event.preventDefault();
        void createConversation();
      }
    };
    window.addEventListener("keydown", shortcuts);
    return () => {
      window.removeEventListener("keydown", shortcuts);
    };
  }, [
    confirmKeyRemoval,
    createConversation,
    deleteTarget,
    pendingNotice,
    renameTarget,
    settingsOpen,
  ]);

  const renameConversation = async (target: ConversationSummary, title: string) => {
    const trimmed = title.trim();
    if (!trimmed) return;
    try {
      const updated = await assistantAdapter.renameConversation(target.id, trimmed);
      setSummaries((current) =>
        sortSummaries(
          current.map((item) => (item.id === updated.id ? { ...item, ...updated } : item)),
        ),
      );
      setConversation((current) =>
        current?.id === updated.id ? { ...current, ...updated } : current,
      );
      setRenameTarget(null);
      setToast("Conversation renamed");
    } catch (error) {
      setToast(publicError(error).message);
    }
  };

  const deleteConversation = async (target: ConversationSummary) => {
    const index = summaries.findIndex((item) => item.id === target.id);
    const nextSelection = summaries[index + 1]?.id ?? summaries[index - 1]?.id ?? null;
    try {
      await assistantAdapter.deleteConversation(target.id);
      setSummaries((current) => current.filter((item) => item.id !== target.id));
      setDeleteTarget(null);
      if (selectedId === target.id) {
        selectedIdRef.current = nextSelection;
        setSelectedId(nextSelection);
        setConversation(null);
        if (nextSelection) await loadConversation(nextSelection);
        else
          window.setTimeout(
            () => document.querySelector<HTMLElement>("[data-new-chat]")?.focus(),
            0,
          );
      }
      setToast("Conversation deleted");
    } catch (error) {
      setToast(publicError(error).message);
    }
  };

  const performSend = async (
    submission: PendingSubmission,
    targetConversation: Conversation | null = conversation,
  ) => {
    if (!targetConversation) return;
    const content = submission.content.trim();
    if (
      !content ||
      content.length > MAX_MESSAGE_LENGTH ||
      activeRef.current.has(targetConversation.id)
    )
      return;

    const conversationId = targetConversation.id;
    snapshotsRef.current.set(conversationId, targetConversation);
    pendingConversationRef.current.add(conversationId);
    pendingPromptRef.current.set(conversationId, content);
    partialContentRef.current.set(conversationId, "");
    setGenerationError(null);

    const targetIndex = submission.regenerateFromMessageId
      ? targetConversation.messages.findIndex(
          (message) => message.id === submission.regenerateFromMessageId,
        )
      : -1;
    const target = targetIndex >= 0 ? targetConversation.messages[targetIndex] : undefined;
    const userMessage: ChatMessage =
      target?.role === "user"
        ? { ...target, content, createdAt: new Date().toISOString(), status: "complete" }
        : {
            id: `pending-user-${String(Date.now())}`,
            conversationId,
            role: "user",
            content,
            createdAt: new Date().toISOString(),
            status: "complete",
          };
    const streamMessage: ChatMessage = {
      id: streamMessageId(conversationId),
      conversationId,
      role: "assistant",
      content: "",
      createdAt: new Date().toISOString(),
      status: "streaming",
    };
    const retained = target
      ? targetConversation.messages.slice(0, targetIndex)
      : targetConversation.messages;
    const optimisticMessages =
      target?.role === "assistant"
        ? [...retained, streamMessage]
        : target?.role === "user"
          ? [...retained, userMessage, streamMessage]
          : [...retained, userMessage, streamMessage];
    const nextTitle =
      targetConversation.title === "New conversation"
        ? content.replace(/\s+/g, " ").slice(0, 42)
        : targetConversation.title;
    setConversation({
      ...targetConversation,
      title: nextTitle,
      reasoningMode: mode,
      messages: optimisticMessages,
    });
    setEditing(null);

    try {
      const result = await assistantAdapter.sendMessage({
        conversationId,
        content,
        reasoningMode: mode,
        regenerateFromMessageId: submission.regenerateFromMessageId,
      });
      const existing = activeRef.current.get(conversationId);
      if (pendingConversationRef.current.has(conversationId) || existing) {
        setActive(conversationId, {
          prompt: content,
          requestId: existing?.requestId || result.requestId,
          state: existing?.state ?? "connecting",
        });
      }
      setDraft("");
      setSummaries((current) =>
        sortSummaries(
          current.map((item) =>
            item.id === conversationId
              ? {
                  ...item,
                  title: nextTitle,
                  updatedAt: new Date().toISOString(),
                  reasoningMode: mode,
                }
              : item,
          ),
        ),
      );
    } catch (error) {
      pendingConversationRef.current.delete(conversationId);
      pendingPromptRef.current.delete(conversationId);
      partialContentRef.current.delete(conversationId);
      const failedActive = activeRef.current.get(conversationId);
      if (failedActive?.requestId) streamSequenceRef.current.delete(failedActive.requestId);
      setActive(conversationId);
      const snapshot = snapshotsRef.current.get(conversationId);
      snapshotsRef.current.delete(conversationId);
      if (snapshot && selectedIdRef.current === conversationId) setConversation(snapshot);
      const safeError = publicError(error);
      setGenerationError({ conversationId, error: safeError });
      setDraft(content);
      window.setTimeout(() => textareaRef.current?.focus(), 0);
    }
  };

  const requestSubmit = () => {
    if (runtime === "tauri" && !credential.configured) {
      setSettingsOpen(true);
      setToast("Add your Z.AI API key before sending");
      return;
    }
    const submission: PendingSubmission = {
      content: draft,
      regenerateFromMessageId: editing?.id,
    };
    if (!conversation) {
      void createConversation().then((created) => {
        if (!created) return;
        if (runtime === "tauri" && !externalNoticeAccepted) {
          setPendingNotice({ submission, conversation: created });
        } else {
          void performSend(submission, created);
        }
      });
      return;
    }
    if (runtime === "tauri" && !externalNoticeAccepted) {
      setPendingNotice({ submission, conversation });
      return;
    }
    void performSend(submission);
  };

  const stopGeneration = async () => {
    if (!conversation) return;
    const active = activeRef.current.get(conversation.id);
    if (!active) return;
    setActive(conversation.id, { ...active, state: "stopping" });
    try {
      await assistantAdapter.cancelGeneration(active.requestId);
    } catch (error) {
      setActive(conversation.id, active);
      setToast(publicError(error).message);
    }
  };

  const startEditing = (message: ChatMessage) => {
    if (activeRef.current.has(message.conversationId)) return;
    setEditing(message);
    setDraft(message.content);
    window.setTimeout(() => {
      textareaRef.current?.focus();
      textareaRef.current?.setSelectionRange(message.content.length, message.content.length);
    }, 0);
  };

  const dispatchRegeneration = (submission: PendingSubmission) => {
    if (!conversation) return;
    if (runtime === "tauri" && !credential.configured) {
      setSettingsOpen(true);
      setToast("Add your Z.AI API key before regenerating");
    } else if (runtime === "tauri" && !externalNoticeAccepted) {
      setPendingNotice({ submission, conversation });
    } else {
      void performSend(submission);
    }
  };

  const regenerate = () => {
    if (!conversation || activeRef.current.has(conversation.id)) return;
    const latest = conversation.messages.at(-1);
    if (!latest || latest.role !== "assistant" || latest.status === "streaming") return;
    const priorUser = [...conversation.messages]
      .reverse()
      .find((message) => message.role === "user");
    if (!priorUser) return;
    const submission = { content: priorUser.content, regenerateFromMessageId: latest.id };
    dispatchRegeneration(submission);
  };

  const retryGeneration = () => {
    if (!conversation || activeRef.current.has(conversation.id)) return;
    const latest = conversation.messages.at(-1);
    if (!latest || latest.status === "streaming") return;
    if (latest.role === "user") {
      dispatchRegeneration({ content: latest.content, regenerateFromMessageId: latest.id });
      return;
    }
    const priorUser = [...conversation.messages]
      .reverse()
      .find((message) => message.role === "user");
    if (priorUser) {
      dispatchRegeneration({ content: priorUser.content, regenerateFromMessageId: latest.id });
    }
  };

  const promptSaveKey = async () => {
    if (runtime !== "tauri") throw new Error("API key setup is available only in the desktop app.");
    const next = await assistantAdapter.promptStoreApiKey();
    if (next.cancelled) {
      setToast("API key setup cancelled");
      return;
    }
    setCredential({ configured: next.configured, source: next.source });
    setToast("API key saved in Windows Credential Manager");
  };

  const deleteKey = async () => {
    const next = await assistantAdapter.deleteApiKey();
    setCredential(next);
    setToast("Saved API key removed");
  };

  const importConversations = async () => {
    const imported = await assistantAdapter.importConversations();
    if (imported.length === 0) {
      setToast("Import cancelled");
      return;
    }
    await refreshSummaries();
    setToast(`${String(imported.length)} conversation${imported.length === 1 ? "" : "s"} imported`);
  };

  const importDemoFile = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    if (file.size > 5_000_000) throw new Error("Import is larger than the 5 MB limit.");
    const serialized = await file.text();
    const imported = await assistantAdapter.importConversations(serialized);
    await refreshSummaries();
    setToast(
      `${String(imported.length)} demo conversation${imported.length === 1 ? "" : "s"} available`,
    );
  };

  const exportConversation = async () => {
    if (!conversation) throw new Error("Select a conversation to export.");
    const result = await assistantAdapter.exportConversation(conversation.id);
    if (typeof result === "string" && runtime === "browser-demo") {
      downloadDemoExport(result, conversation.title);
      setToast("Demo export downloaded as plaintext JSON");
      return;
    }
    if (isExportResult(result)) {
      setToast(
        result.cancelled ? "Export cancelled" : `Exported ${result.fileName ?? "conversation"}`,
      );
      return;
    }
    setToast("Conversation exported");
  };

  const runWindowCommand = async (command: "minimize" | "maximize" | "close") => {
    if (runtime !== "tauri") return;
    try {
      const appWindow = getCurrentWindow();
      if (command === "minimize") await appWindow.minimize();
      else if (command === "maximize") await appWindow.toggleMaximize();
      else await appWindow.close();
    } catch {
      setToast("Aster could not control the application window.");
    }
  };

  const active = selectedId ? activeByConversation[selectedId] : undefined;
  const visibleGenerationError =
    conversation && generationError?.conversationId === conversation.id
      ? generationError.error
      : null;
  const lastMessage = conversation?.messages.at(-1);
  const canRegenerate =
    lastMessage?.role === "assistant" && lastMessage.status !== "streaming" && !active;
  const connectionLabel =
    runtime === "browser-demo"
      ? "Demo"
      : appStatus.providerReachability === "reachable"
        ? "Reachable"
        : appStatus.providerReachability === "unreachable"
          ? "Unreachable"
          : "Not checked";
  const connectionClass =
    appStatus.providerReachability === "reachable"
      ? "online"
      : appStatus.providerReachability === "unreachable"
        ? "offline"
        : "unknown";

  return (
    <div className="app-shell">
      <a className="skip-link" href="#main-content">
        Skip to conversation
      </a>
      {runtime === "browser-demo" && (
        <div className="demo-banner" role="status">
          <Icon name="shield" size={14} />
          <strong>Browser demo</strong>
          <span>In-memory only · no provider network · no API keys</span>
        </div>
      )}
      <div className="titlebar" data-tauri-drag-region>
        <div className="titlebar-left">
          <button
            className="icon-button mobile-menu"
            type="button"
            onClick={() => {
              setSidebarOpen(true);
            }}
            aria-label="Open sidebar"
          >
            <Icon name="menu" size={18} />
          </button>
          <span className="app-menu">File</span>
          <span className="app-menu">Edit</span>
          <span className="app-menu">View</span>
          <span className="app-menu">Help</span>
        </div>
        <div className="window-title" data-tauri-drag-region>
          <img src={asterMark} alt="" /> Aster
        </div>
        <div className="window-controls" aria-label="Window controls" role="group">
          <button
            aria-label="Minimize window"
            disabled={runtime !== "tauri"}
            type="button"
            onClick={() => {
              void runWindowCommand("minimize");
            }}
          >
            <span aria-hidden="true">—</span>
          </button>
          <button
            aria-label="Maximize or restore window"
            disabled={runtime !== "tauri"}
            type="button"
            onClick={() => {
              void runWindowCommand("maximize");
            }}
          >
            <span aria-hidden="true">□</span>
          </button>
          <button
            aria-label="Close window"
            className="window-close"
            disabled={runtime !== "tauri"}
            type="button"
            onClick={() => {
              void runWindowCommand("close");
            }}
          >
            <span aria-hidden="true">×</span>
          </button>
        </div>
      </div>

      <div className="workspace">
        {sidebarOpen && (
          <button
            className="sidebar-scrim"
            aria-label="Close sidebar"
            type="button"
            onClick={() => {
              setSidebarOpen(false);
            }}
          />
        )}
        <aside
          className={`sidebar ${sidebarOpen ? "sidebar-open" : ""}`}
          aria-label="Conversation navigation"
        >
          <div className="brand-row">
            <div className="brand">
              <img src={asterMark} alt="" />
              <span>Aster</span>
              <small>AI</small>
            </div>
            <button
              className="icon-button"
              type="button"
              onClick={() => {
                setSidebarOpen(false);
              }}
              aria-label="Collapse sidebar"
            >
              <Icon name="sidebar" size={17} />
            </button>
          </div>
          <div className="primary-nav">
            <button
              data-new-chat
              className="nav-button new-chat"
              type="button"
              onClick={() => void createConversation()}
            >
              <Icon name="edit" size={17} />
              <span>New chat</span>
              <kbd>Ctrl N</kbd>
            </button>
            <button
              className="nav-button"
              type="button"
              onClick={() => {
                setSearchOpen((current) => !current);
                window.setTimeout(() => searchRef.current?.focus(), 0);
              }}
            >
              <Icon name="search" size={17} />
              <span>Search titles</span>
              <kbd>Ctrl K</kbd>
            </button>
          </div>
          {searchOpen && (
            <div className="sidebar-search">
              <Icon name="search" size={15} />
              <input
                aria-label="Search conversation titles"
                onChange={(event) => {
                  setQuery(event.target.value);
                }}
                placeholder="Search conversation titles"
                ref={searchRef}
                value={query}
              />
              {query && (
                <button
                  className="icon-button"
                  type="button"
                  onClick={() => {
                    setQuery("");
                  }}
                  aria-label="Clear search"
                >
                  <Icon name="x" size={14} />
                </button>
              )}
            </div>
          )}

          <div className="history" aria-label="Recent conversations">
            <div className="sidebar-section-label">
              <span>Chats</span>
              <span>{summaries.length}</span>
            </div>
            {loading ? (
              <div className="history-skeleton" aria-label="Loading conversations">
                <span />
                <span />
                <span />
              </div>
            ) : groupedSummaries.length === 0 ? (
              <div className="sidebar-empty">
                <Icon name="message" size={20} />
                <span>{query ? "No matching titles" : "No conversations yet"}</span>
              </div>
            ) : (
              groupedSummaries.map(([label, items]) => (
                <section className="history-group" key={label}>
                  <h2>{label}</h2>
                  {items.map((item) => (
                    <div
                      className={`conversation-row ${selectedId === item.id ? "selected" : ""}`}
                      key={item.id}
                    >
                      <button
                        className="conversation-select"
                        type="button"
                        onClick={() => {
                          // The callback reads selection refs only after this user activation.
                          // eslint-disable-next-line react-hooks/refs
                          void selectConversation(item.id);
                        }}
                        aria-current={selectedId === item.id ? "page" : undefined}
                      >
                        <span>{item.title}</span>
                      </button>
                      <button
                        className="conversation-menu-button"
                        type="button"
                        aria-expanded={menuFor === item.id}
                        aria-label={`Actions for ${item.title}`}
                        onClick={() => {
                          setMenuFor((current) => (current === item.id ? null : item.id));
                        }}
                      >
                        <Icon name="more" size={16} />
                      </button>
                      {menuFor === item.id && (
                        <div className="context-menu">
                          <button
                            type="button"
                            onClick={() => {
                              setRenameTarget(item);
                              setMenuFor(null);
                            }}
                          >
                            <Icon name="edit" size={15} /> Rename
                          </button>
                          <button
                            className="danger"
                            type="button"
                            onClick={() => {
                              setDeleteTarget(item);
                              setMenuFor(null);
                            }}
                          >
                            <Icon name="trash" size={15} /> Delete
                          </button>
                        </div>
                      )}
                    </div>
                  ))}
                </section>
              ))
            )}
          </div>

          <div className="sidebar-lower">
            <div className="coming-soon-section">
              <div className="sidebar-section-label">
                <span>Workspace</span>
                <span className="soon-label">Coming soon</span>
              </div>
              <div className="disabled-feature" aria-disabled="true">
                <Icon name="folder" size={16} /> Projects
              </div>
              <div className="disabled-feature" aria-disabled="true">
                <Icon name="paperclip" size={16} /> Files & tools
              </div>
            </div>
            {onboardingVisible && (
              <div className="local-card">
                <button
                  className="card-close"
                  type="button"
                  onClick={() => {
                    setOnboardingVisible(false);
                  }}
                  aria-label="Dismiss local data notice"
                >
                  <Icon name="x" size={13} />
                </button>
                <div className="local-card-icon">
                  <Icon name="shield" size={16} />
                </div>
                <div>
                  <strong>Local by design</strong>
                  <p>
                    {runtime === "browser-demo"
                      ? "This demo resets on reload."
                      : "Chats stay on this device."}
                  </p>
                </div>
              </div>
            )}
            <button
              className="settings-button"
              type="button"
              onClick={() => {
                setSettingsOpen(true);
              }}
            >
              <span className="avatar">A</span>
              <span>
                <strong>Settings</strong>
                <small>
                  {runtime === "browser-demo"
                    ? "Demo mode"
                    : credential.configured
                      ? "Z.AI connected"
                      : "Setup required"}
                </small>
              </span>
              <Icon name="settings" size={17} />
            </button>
          </div>
        </aside>

        <main className="main-panel" id="main-content">
          <header className="conversation-header">
            <button
              className="icon-button mobile-menu"
              type="button"
              onClick={() => {
                setSidebarOpen(true);
              }}
              aria-label="Open sidebar"
            >
              <Icon name="menu" size={19} />
            </button>
            <div className="conversation-heading">
              <h1>{conversation?.title ?? "New conversation"}</h1>
              {conversation && (
                <span>
                  {conversation.model ?? "glm-5.1"} · {mode.charAt(0).toUpperCase() + mode.slice(1)}
                </span>
              )}
            </div>
            <div className="header-actions">
              <span className={`connection-indicator ${connectionClass}`}>
                <span aria-hidden="true" className="status-dot" />
                {connectionLabel}
              </span>
              <button
                className="icon-button"
                type="button"
                onClick={() => {
                  setSettingsOpen(true);
                }}
                aria-label="Open settings"
              >
                <Icon name="settings" size={18} />
              </button>
            </div>
          </header>

          {loadError ? (
            <div className="center-state error-state">
              <div className="state-icon">
                <Icon name="x" size={24} />
              </div>
              <h2>We could not load your chats</h2>
              <p>{loadError}</p>
              <button
                className="button primary"
                type="button"
                onClick={() => {
                  window.location.reload();
                }}
              >
                Try again
              </button>
            </div>
          ) : loadingConversation ? (
            <div className="message-loading" aria-label="Loading conversation">
              <span />
              <span />
              <span />
            </div>
          ) : !conversation ? (
            <div className="empty-workspace">
              <div className="empty-mark">
                <img src={asterMark} alt="" />
              </div>
              <p className="eyebrow">Local-first desktop assistant</p>
              <h2>What would you like to work through?</h2>
              <p>Start a focused conversation. Your history stays local on this device.</p>
              <div className="starter-grid">
                <button type="button" onClick={() => void createConversation()}>
                  <Icon name="shield" size={18} />
                  <span>
                    <strong>Review a security decision</strong>
                    <small>Map risks and controls</small>
                  </span>
                </button>
                <button type="button" onClick={() => void createConversation()}>
                  <Icon name="code" size={18} />
                  <span>
                    <strong>Plan an implementation</strong>
                    <small>Turn requirements into steps</small>
                  </span>
                </button>
              </div>
            </div>
          ) : conversation.messages.length === 0 ? (
            <div className="empty-workspace conversation-empty">
              <div className="empty-mark">
                <img src={asterMark} alt="" />
              </div>
              <h2>Start with a question</h2>
              <p>
                Aster can help you reason, draft, and build. Keep credentials and sensitive data out
                of messages.
              </p>
            </div>
          ) : (
            <div className="message-scroll" aria-live="polite" aria-busy={Boolean(active)}>
              <div className="messages">
                {conversation.messages.map((message) => (
                  <article className={`message message-${message.role}`} key={message.id}>
                    {message.role === "user" ? (
                      <div className="user-message-wrap">
                        <div className="user-bubble">
                          <p>{message.content}</p>
                        </div>
                        <div className="message-meta user-meta">
                          <span>{formatTime(message.createdAt)}</span>
                          <button
                            type="button"
                            disabled={Boolean(active)}
                            onClick={() => {
                              startEditing(message);
                            }}
                            aria-label="Edit and resend this message"
                          >
                            <Icon name="edit" size={14} /> Edit
                          </button>
                        </div>
                      </div>
                    ) : (
                      <div className="assistant-message-wrap">
                        <div className="assistant-avatar">
                          <img src={asterMark} alt="Aster" />
                        </div>
                        <div className="assistant-content">
                          {message.status === "streaming" && !message.content ? (
                            <div className="thinking">
                              <span />
                              <span />
                              <span />{" "}
                              {active?.state === "connecting" ? "Connecting securely" : "Thinking"}
                            </div>
                          ) : (
                            <Markdown
                              content={
                                message.content ||
                                (message.status === "error"
                                  ? "The response could not be completed."
                                  : "Generation stopped before any text arrived.")
                              }
                              streaming={message.status === "streaming"}
                              onCopied={() => {
                                setToast("Code copied as plain text");
                              }}
                              onOpenLink={(url) => {
                                void assistantAdapter
                                  .openExternalUrl(url)
                                  .catch((error: unknown) => {
                                    setToast(publicError(error).message);
                                  });
                              }}
                            />
                          )}
                          <div className="message-meta">
                            <span>
                              {message.status === "streaming"
                                ? "Responding…"
                                : message.status === "cancelled"
                                  ? "Stopped"
                                  : message.status === "error"
                                    ? "Response failed"
                                    : formatTime(message.createdAt)}
                            </span>
                            {message.status !== "streaming" && message.content && (
                              <button
                                type="button"
                                onClick={() => {
                                  void copyPlainText(message.content).then(
                                    () => {
                                      setToast("Response copied as plain text");
                                    },
                                    () => {
                                      setToast("Could not copy response");
                                    },
                                  );
                                }}
                                aria-label="Copy response"
                              >
                                <Icon name="copy" size={14} /> Copy
                              </button>
                            )}
                          </div>
                        </div>
                      </div>
                    )}
                  </article>
                ))}
                {visibleGenerationError && (
                  <div className="generation-error" role="alert">
                    <Icon name="x" size={17} />
                    <div>
                      <strong>Response interrupted</strong>
                      <p>{visibleGenerationError.message}</p>
                    </div>
                    {visibleGenerationError.retryable && (
                      <button type="button" disabled={Boolean(active)} onClick={retryGeneration}>
                        Retry response
                      </button>
                    )}
                    {visibleGenerationError.code === "credential_not_configured" && (
                      <button
                        type="button"
                        onClick={() => {
                          setSettingsOpen(true);
                        }}
                      >
                        Open settings
                      </button>
                    )}
                  </div>
                )}
                {canRegenerate && (
                  <div className="regenerate-row">
                    <button className="button quiet" type="button" onClick={regenerate}>
                      <Icon name="refresh" size={15} /> Regenerate latest response
                    </button>
                  </div>
                )}
              </div>
            </div>
          )}

          <Composer
            active={active}
            credentialConfigured={credential.configured}
            draft={draft}
            editing={editing}
            mode={mode}
            providerUnreachable={
              runtime === "tauri" && appStatus.providerReachability === "unreachable"
            }
            onCancelEdit={() => {
              setEditing(null);
              setDraft("");
              textareaRef.current?.focus();
            }}
            onChange={setDraft}
            onModeChange={setMode}
            onOpenSettings={() => {
              setSettingsOpen(true);
            }}
            onStop={stopGeneration}
            onSubmit={requestSubmit}
            runtime={runtime}
            textareaRef={textareaRef}
          />
        </main>
      </div>

      {settingsOpen && (
        <SettingsDialog
          appStatus={appStatus}
          credential={credential}
          currentConversation={conversation}
          onClose={() => {
            setSettingsOpen(false);
          }}
          onRequestDeleteKey={() => {
            setSettingsOpen(false);
            setConfirmKeyRemoval(true);
          }}
          onExport={exportConversation}
          onImport={importConversations}
          onImportFile={importDemoFile}
          onPromptSaveKey={promptSaveKey}
          runtime={runtime}
        />
      )}

      {confirmKeyRemoval && (
        <Dialog
          label="Remove saved API key?"
          description="Aster will delete the Z.AI credential from Windows Credential Manager. You will not be able to send another provider request until a new key is saved."
          onClose={() => {
            setConfirmKeyRemoval(false);
            setSettingsOpen(true);
          }}
          size="small"
        >
          <div className="danger-notice">
            <Icon name="key" size={20} />
            <p>This removes only the saved credential. Local conversation history is unchanged.</p>
          </div>
          <div className="dialog-actions">
            <button
              className="button secondary"
              disabled={removingKey}
              type="button"
              onClick={() => {
                setConfirmKeyRemoval(false);
                setSettingsOpen(true);
              }}
            >
              Keep saved key
            </button>
            <button
              className="button danger"
              disabled={removingKey}
              type="button"
              onClick={() => {
                setRemovingKey(true);
                void deleteKey()
                  .then(() => {
                    setConfirmKeyRemoval(false);
                    setSettingsOpen(true);
                  })
                  .catch((error: unknown) => {
                    setToast(publicError(error).message);
                  })
                  .finally(() => {
                    setRemovingKey(false);
                  });
              }}
            >
              {removingKey ? "Removing…" : "Remove key"}
            </button>
          </div>
        </Dialog>
      )}

      {renameTarget && (
        <Dialog
          label="Rename conversation"
          description="Choose a short, recognizable title."
          onClose={() => {
            setRenameTarget(null);
          }}
          size="small"
        >
          <form
            className="dialog-form"
            onSubmit={(event) => {
              event.preventDefault();
              const title = renameRef.current?.value ?? "";
              void renameConversation(renameTarget, title);
            }}
          >
            <label htmlFor="conversation-title">Conversation title</label>
            <input
              defaultValue={renameTarget.title}
              id="conversation-title"
              maxLength={80}
              ref={renameRef}
              required
            />
            <div className="dialog-actions">
              <button
                className="button secondary"
                type="button"
                onClick={() => {
                  setRenameTarget(null);
                }}
              >
                Cancel
              </button>
              <button className="button primary" type="submit">
                Save title
              </button>
            </div>
          </form>
        </Dialog>
      )}

      {deleteTarget && (
        <Dialog
          label="Delete conversation?"
          description={`“${deleteTarget.title}” and all of its messages will be permanently removed from local storage.`}
          onClose={() => {
            setDeleteTarget(null);
          }}
          size="small"
        >
          <div className="danger-notice">
            <Icon name="trash" size={20} />
            <p>This cannot be undone and does not affect any other conversation.</p>
          </div>
          <div className="dialog-actions">
            <button
              className="button secondary"
              type="button"
              onClick={() => {
                setDeleteTarget(null);
              }}
            >
              Keep conversation
            </button>
            <button
              className="button danger"
              type="button"
              onClick={() => void deleteConversation(deleteTarget)}
            >
              Delete permanently
            </button>
          </div>
        </Dialog>
      )}

      {pendingNotice && (
        <Dialog
          label="Before your first message"
          description="Review how your conversation is processed."
          onClose={() => {
            if (!acknowledgingExternal) setPendingNotice(null);
          }}
          size="small"
        >
          <div className="provider-notice">
            <div className="notice-icon">
              <Icon name="shield" size={22} />
            </div>
            <div>
              <h3>Messages are processed by Z.AI</h3>
              <p>
                Your message and relevant conversation context will leave this device. Do not send
                secrets or sensitive data without reviewing Z.AI’s policies.
              </p>
            </div>
          </div>
          <p className="local-disclosure">
            Conversation history is also stored locally in SQLite and is not synchronized by Aster.
          </p>
          <div className="dialog-actions">
            <button
              className="button secondary"
              disabled={acknowledgingExternal}
              type="button"
              onClick={() => {
                setPendingNotice(null);
              }}
            >
              Go back
            </button>
            <button
              className="button primary"
              disabled={acknowledgingExternal}
              type="button"
              onClick={() => {
                const consent = pendingNotice;
                setAcknowledgingExternal(true);
                void assistantAdapter
                  .acknowledgeExternalProcessing()
                  .then(() => {
                    setPendingNotice(null);
                    setExternalNoticeAccepted(true);
                    selectedIdRef.current = consent.conversation.id;
                    setSelectedId(consent.conversation.id);
                    setConversation(consent.conversation);
                    void performSend(consent.submission, consent.conversation);
                  })
                  .catch((error: unknown) => {
                    setToast(publicError(error).message);
                  })
                  .finally(() => {
                    setAcknowledgingExternal(false);
                  });
              }}
            >
              {acknowledgingExternal ? "Confirming…" : "Continue and send"}
            </button>
          </div>
        </Dialog>
      )}

      <div className="toast-region" aria-live="polite" aria-atomic="true">
        {toast && (
          <div className="toast">
            <Icon name="check" size={15} />
            {toast}
          </div>
        )}
      </div>
    </div>
  );
}
