# ADR-0006: Open Validated HTTPS Links Through a Rust-Scoped System Opener

**Status:** Accepted

**Date:** 2026-07-12

**Decision owners:** `FR-009`, `SEC-015`, `SEC-017`, `AC-024`, `AC-025`

## Context

Assistant Markdown may contain links. Letting an untrusted anchor navigate the webview can replace the application UI, reach custom or local schemes, or create an unmanaged window. Granting the renderer a general shell or opener capability would exceed the MVP trust boundary. Rendering every destination as inert text would not satisfy the specified external-link behavior.

## Decision

MVP v1 exposes one typed application command, `open_external_url`, only to the main window. The Rust command accepts a bounded URL string, parses it again outside the renderer, and permits only an absolute `https` URL with a host and without embedded username or password data. It rejects loopback, local/private destinations, control characters, non-default schemes, paths supplied as files, and all malformed input.

After validation, Rust delegates only that URL to the official Tauri opener plugin's Rust API so the operating system opens it in the user's default external browser. The renderer receives no opener-plugin, shell, process, file, or wildcard URL permission. The renderer also performs a usability-only HTTPS check, displays the destination hostname, prevents ordinary in-webview navigation, and invokes only the typed application command. Browser-demo behavior is isolated and cannot count as desktop security evidence.

## Consequences

- Model and imported content cannot navigate the Aster webview directly.
- A single narrowly validated OS integration and runtime dependency is added to satisfy `FR-009` and `SEC-015`.
- The command and its permission must appear in IPC/capability inventories and protocol-policy tests.
- Opening a valid public HTTPS destination is an explicit user click and may disclose the user's IP address and browser metadata to that destination.

## Rejected alternatives

- Raw `<a target="_blank">` navigation from untrusted model output.
- The opener plugin's default permission set, which also permits HTTP, mail, telephone, or file-related behavior.
- A renderer-accessible unrestricted opener or shell command.
- Accepting a caller-selected application for the URL.

## Verification

`AC-024` exercises an encoding and protocol corpus plus a valid public HTTPS destination. `AC-025` asserts that the renderer has only the custom command permission and no raw opener, shell, process, file, or wildcard URL capability. Unit tests cover length, credentials, local/private hosts, IP literals, controls, and unsupported schemes. Packaged Windows E2E remains `NOT RUN` until the external browser boundary is observed in the engineering build.
