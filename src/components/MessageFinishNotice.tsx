import type { FinishReason } from "../types/chat";
import { Icon } from "./Icon";

interface MessageFinishNoticeProps {
  finishReason?: FinishReason;
}

export function MessageFinishNotice({ finishReason }: MessageFinishNoticeProps) {
  if (finishReason !== "outputLimit") return null;

  return (
    <aside
      aria-label="Incomplete response notice"
      className="message-finish-notice"
      role="note"
      tabIndex={0}
    >
      <Icon name="warning" size={17} />
      <span>
        The provider reached Aster&apos;s output limit. This response may be incomplete. This is not
        a provider error.
      </span>
    </aside>
  );
}
