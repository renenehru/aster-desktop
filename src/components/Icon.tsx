import type { SVGProps } from "react";

export type IconName =
  | "archive"
  | "arrow-up"
  | "check"
  | "chevron-down"
  | "code"
  | "copy"
  | "download"
  | "edit"
  | "file"
  | "folder"
  | "history"
  | "key"
  | "menu"
  | "message"
  | "more"
  | "paperclip"
  | "plus"
  | "refresh"
  | "search"
  | "settings"
  | "shield"
  | "sidebar"
  | "spark"
  | "stop"
  | "trash"
  | "upload"
  | "x";

interface IconProps extends SVGProps<SVGSVGElement> {
  name: IconName;
  size?: number;
}

export function Icon({ name, size = 18, ...props }: IconProps) {
  const common = {
    width: size,
    height: size,
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.8,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    "aria-hidden": true,
    focusable: false,
    ...props,
  };

  switch (name) {
    case "archive":
      return (
        <svg {...common}>
          <path d="M4 8h16M5 8v11h14V8M3 4h18v4H3zM9 12h6" />
        </svg>
      );
    case "arrow-up":
      return (
        <svg {...common}>
          <path d="m6 11 6-6 6 6M12 5v14" />
        </svg>
      );
    case "check":
      return (
        <svg {...common}>
          <path d="m5 12 4 4L19 6" />
        </svg>
      );
    case "chevron-down":
      return (
        <svg {...common}>
          <path d="m7 9 5 5 5-5" />
        </svg>
      );
    case "code":
      return (
        <svg {...common}>
          <path d="m8 9-3 3 3 3M16 9l3 3-3 3M14 5l-4 14" />
        </svg>
      );
    case "copy":
      return (
        <svg {...common}>
          <rect x="8" y="8" width="11" height="11" rx="2" />
          <path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2" />
        </svg>
      );
    case "download":
      return (
        <svg {...common}>
          <path d="M12 3v12m0 0 4-4m-4 4-4-4M5 20h14" />
        </svg>
      );
    case "edit":
      return (
        <svg {...common}>
          <path d="m14 5 5 5M4 20l3.5-.8L19 7.7 16.3 5 4.8 16.5z" />
        </svg>
      );
    case "file":
      return (
        <svg {...common}>
          <path d="M6 3h8l4 4v14H6zM14 3v5h5" />
        </svg>
      );
    case "folder":
      return (
        <svg {...common}>
          <path d="M3 7.5h7l2-2h9v13H3z" />
        </svg>
      );
    case "history":
      return (
        <svg {...common}>
          <path d="M4 4v5h5M5.2 17.8A9 9 0 1 0 4 9M12 7v5l3 2" />
        </svg>
      );
    case "key":
      return (
        <svg {...common}>
          <circle cx="8" cy="15" r="4" />
          <path d="m11 12 8-8m-3 3 2 2m-5 1 2 2" />
        </svg>
      );
    case "menu":
      return (
        <svg {...common}>
          <path d="M4 7h16M4 12h16M4 17h16" />
        </svg>
      );
    case "message":
      return (
        <svg {...common}>
          <path d="M4 5h16v12H8l-4 3z" />
        </svg>
      );
    case "more":
      return (
        <svg {...common}>
          <circle cx="5" cy="12" r="1" fill="currentColor" stroke="none" />
          <circle cx="12" cy="12" r="1" fill="currentColor" stroke="none" />
          <circle cx="19" cy="12" r="1" fill="currentColor" stroke="none" />
        </svg>
      );
    case "paperclip":
      return (
        <svg {...common}>
          <path d="m8 12 6.5-6.5a3.2 3.2 0 0 1 4.5 4.5l-8.5 8.5a5 5 0 0 1-7-7L12 3" />
        </svg>
      );
    case "plus":
      return (
        <svg {...common}>
          <path d="M12 5v14M5 12h14" />
        </svg>
      );
    case "refresh":
      return (
        <svg {...common}>
          <path d="M20 7v5h-5M4 17v-5h5M18.5 9A7 7 0 0 0 6 7M5.5 15A7 7 0 0 0 18 17" />
        </svg>
      );
    case "search":
      return (
        <svg {...common}>
          <circle cx="10.5" cy="10.5" r="6.5" />
          <path d="m16 16 4 4" />
        </svg>
      );
    case "settings":
      return (
        <svg {...common}>
          <circle cx="12" cy="12" r="3" />
          <path
            d="M19 13.5v-3l-2-.7-.6-1.4.9-1.9-2.1-2.1-1.9.9-1.4-.6L10.5 3h-3l-.7 2-1.4.6-1.9-.9-2.1 2.1.9 1.9-.6 1.4L0 10.5v3l2 .7.6 1.4-.9 1.9 2.1 2.1 1.9-.9 1.4.6.7 2h3l.7-2 1.4-.6 1.9.9 2.1-2.1-.9-1.9.6-1.4z"
            transform="translate(2 0) scale(.83 1)"
          />
        </svg>
      );
    case "shield":
      return (
        <svg {...common}>
          <path d="M12 3 5 6v5c0 4.6 2.7 8 7 10 4.3-2 7-5.4 7-10V6z" />
          <path d="m9 12 2 2 4-4" />
        </svg>
      );
    case "sidebar":
      return (
        <svg {...common}>
          <rect x="3" y="4" width="18" height="16" rx="2" />
          <path d="M9 4v16" />
        </svg>
      );
    case "spark":
      return (
        <svg {...common}>
          <path d="M12 2c.4 5.6 2.4 8.1 8 8.5-5.6.4-7.6 2.9-8 8.5-.4-5.6-2.4-8.1-8-8.5 5.6-.4 7.6-2.9 8-8.5Z" />
          <path d="M19 3v4M17 5h4" />
        </svg>
      );
    case "stop":
      return (
        <svg {...common}>
          <rect x="7" y="7" width="10" height="10" rx="2" fill="currentColor" stroke="none" />
        </svg>
      );
    case "trash":
      return (
        <svg {...common}>
          <path d="M4 7h16M9 7V4h6v3M6 7l1 14h10l1-14M10 11v6M14 11v6" />
        </svg>
      );
    case "upload":
      return (
        <svg {...common}>
          <path d="M12 16V4m0 0L8 8m4-4 4 4M5 20h14" />
        </svg>
      );
    case "x":
      return (
        <svg {...common}>
          <path d="m6 6 12 12M18 6 6 18" />
        </svg>
      );
  }
}
