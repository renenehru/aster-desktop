import type {
  DeepSeekBalance,
  ModelCatalog,
  ModelId,
  ProviderAccountAction,
  ProviderId,
  ProviderStatus,
  ResponseProfile,
  TokenUsage,
  UsageSummary,
} from "./providers";

export type { ResponseProfile } from "./providers";
export type ReasoningMode = ResponseProfile;

export type MessageRole = "user" | "assistant";

export type MessageStatus = "complete" | "streaming" | "cancelled" | "error";

export type FinishReason = "stop" | "outputLimit" | "unknown";

export interface ChatMessage {
  id: string;
  conversationId: string;
  role: MessageRole;
  content: string;
  createdAt: string;
  status: MessageStatus;
  finishReason?: FinishReason;
  usage?: TokenUsage;
}

export interface ConversationSummary {
  id: string;
  title: string;
  providerId: ProviderId;
  modelId: ModelId;
  createdAt: string;
  updatedAt: string;
  messageCount?: number;
  responseProfile: ResponseProfile;
}

export interface Conversation extends ConversationSummary {
  messages: ChatMessage[];
}

export interface AppStatus {
  mode: "desktop" | "demo";
  version?: string;
  online: boolean;
  databaseReady?: boolean;
}

export interface CredentialStatus {
  providerId: ProviderId;
  configured: boolean;
  source?: "credential-vault" | "demo-session" | "none";
}

export interface CredentialPromptResult extends CredentialStatus {
  cancelled: boolean;
}

export type ChatStreamKind = "started" | "delta" | "completed" | "cancelled" | "error";

export interface ChatStreamEvent {
  requestId: string;
  conversationId: string;
  providerId: ProviderId;
  modelId: ModelId;
  sequence: number;
  kind: ChatStreamKind;
  delta?: string;
  message?: ChatMessage;
  error?: string;
  errorCode?: string;
  retryable?: boolean;
}

export interface SendMessageInput {
  conversationId: string;
  content: string;
  responseProfile: ResponseProfile;
  regenerateFromMessageId?: string;
}

export interface SendMessageResult {
  requestId: string;
}

export interface ExportResult {
  cancelled: boolean;
  fileName?: string;
}

export type StreamUnsubscribe = () => void;

export interface AssistantAdapter {
  readonly runtime: "tauri" | "browser-demo";
  appStatus(): Promise<AppStatus>;
  modelCatalog(): Promise<ModelCatalog>;
  providerStatuses(): Promise<ProviderStatus[]>;
  listConversations(): Promise<ConversationSummary[]>;
  getConversation(id: string): Promise<Conversation>;
  createConversation(
    title?: string,
    selection?: Readonly<{ providerId: ProviderId; modelId: ModelId }>,
  ): Promise<Conversation>;
  updateConversationSelection(
    conversationId: string,
    providerId: ProviderId,
    modelId: ModelId,
  ): Promise<Conversation>;
  renameConversation(id: string, title: string): Promise<ConversationSummary>;
  deleteConversation(id: string): Promise<void>;
  sendMessage(input: SendMessageInput): Promise<SendMessageResult>;
  cancelGeneration(requestId: string): Promise<void>;
  onChatStream(
    listener: (event: ChatStreamEvent) => void,
    onUnscopedProtocolError?: () => void,
  ): Promise<StreamUnsubscribe>;
  acknowledgeExternalProcessing(providerId: ProviderId): Promise<void>;
  usageSummary(providerId: ProviderId, modelId?: ModelId): Promise<UsageSummary>;
  setUsageBudget(providerId: ProviderId, tokenBudget: number | null): Promise<UsageSummary>;
  deepSeekBalanceStatus(): Promise<DeepSeekBalance | null>;
  refreshDeepSeekBalance(): Promise<DeepSeekBalance>;
  openExternalUrl(url: string): Promise<void>;
  exportConversation(id: string): Promise<string | ExportResult | undefined>;
  importConversations(serialized?: string): Promise<ConversationSummary[]>;
}

export interface DesktopAssistantAdapter extends AssistantAdapter {
  readonly runtime: "tauri";
  promptStoreApiKey(providerId: ProviderId): Promise<CredentialPromptResult>;
  deleteApiKey(providerId: ProviderId): Promise<CredentialStatus>;
  openProviderAccount(providerId: ProviderId, action: ProviderAccountAction): Promise<void>;
}
