# ADR-0007: Use a Narrow Custom Windows Title Bar

**Status:** Accepted

**Date:** 2026-07-12

**Decision owners:** `FR-001`, `UX-001`, `UX-005`, `SEC-017`, `AC-001`, `AC-025`, `AC-035`

## Context

The original Aster shell includes a compact application title bar modeled on the supplied Windows 11 layout. Leaving normal operating-system decorations enabled at the same time would produce two title bars. Making the custom controls decorative would leave a borderless window without an accessible minimize, maximize, close, or drag path.

## Decision

The main Windows window disables standard decorations and renders one local custom title bar. Tauri grants only four core window permissions to the main window: minimize, toggle maximize, close, and start dragging. It grants no window creation, arbitrary resize/position, fullscreen, visibility, title mutation, or multi-window permission.

The renderer uses semantic buttons with English accessible names for minimize, maximize/restore, and close. The title-bar background is the only drag region. Keyboard users retain the operating system's normal application-switching and close shortcuts. Browser-demo controls are visibly representative but perform no native window action and cannot count as desktop evidence.

## Consequences

- The packaged desktop has one title bar and remains visually aligned with the reference layout.
- Four narrowly exercised core permissions join the capability inventory.
- Window-control behavior, Windows scaling, resizable edges, and keyboard behavior require packaged Windows E2E evidence.
- If custom controls fail in a future Tauri/Windows release, decorations must be restored before distribution rather than shipping an unclosable window.

## Rejected alternatives

- Shipping both native and custom title bars.
- A borderless window with decorative or pointer-only controls.
- Granting `core:window:default` or other broad window permission sets.
- Creating additional webview windows for settings or links.

## Verification

`AC-001` covers layout and accessible control names. `AC-025` inventories exactly the four core permissions and rejects window creation or unrelated window authority. `AC-035` exercises drag, minimize, maximize/restore, close, resize edges, keyboard switching/close, and Windows scaling in the packaged engineering candidate. Until that run exists, packaged custom-title-bar behavior is `NOT RUN`.
