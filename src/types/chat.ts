export type ReasoningMode = "fast" | "standard" | "deep";

export type MessageRole = "user" | "assistant";

export type MessageStatus = "complete" | "streaming" | "cancelled" | "error";

export interface ChatMessage {
  id: string;
  conversationId: string;
  role: MessageRole;
  content: string;
  createdAt: string;
  status: MessageStatus;
}

export interface ConversationSummary {
  id: string;
  title: string;
  model?: string;
  createdAt: string;
  updatedAt: string;
  messageCount?: number;
  reasoningMode?: ReasoningMode;
}

export interface Conversation extends ConversationSummary {
  messages: ChatMessage[];
}

export interface AppStatus {
  mode: "desktop" | "demo";
  version?: string;
  online: boolean;
  providerReachability?: "unknown" | "reachable" | "unreachable";
  externalProcessingAcknowledged?: boolean;
  databaseReady?: boolean;
}

export interface CredentialStatus {
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
  reasoningMode: ReasoningMode;
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
  credentialStatus(): Promise<CredentialStatus>;
  promptStoreApiKey(): Promise<CredentialPromptResult>;
  deleteApiKey(): Promise<CredentialStatus>;
  listConversations(): Promise<ConversationSummary[]>;
  getConversation(id: string): Promise<Conversation>;
  createConversation(title?: string): Promise<Conversation>;
  renameConversation(id: string, title: string): Promise<ConversationSummary>;
  deleteConversation(id: string): Promise<void>;
  sendMessage(input: SendMessageInput): Promise<SendMessageResult>;
  cancelGeneration(requestId: string): Promise<void>;
  onChatStream(
    listener: (event: ChatStreamEvent) => void,
    onUnscopedProtocolError?: () => void,
  ): Promise<StreamUnsubscribe>;
  acknowledgeExternalProcessing(): Promise<void>;
  openExternalUrl(url: string): Promise<void>;
  exportConversation(id: string): Promise<string | ExportResult | undefined>;
  importConversations(serialized?: string): Promise<ConversationSummary[]>;
}
