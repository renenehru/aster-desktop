# ADR-0008: Capture Provider Credentials Outside the Webview

**Status:** Accepted

**Date:** 2026-07-12

**Decision owners:** `FR-006`, `SEC-001`-`SEC-005`, `SEC-017`, `SEC-018`, `SEC-020`, `AC-005`, `AC-025`

## Context

The React webview is an untrusted presentation layer. Even a password input whose value is never placed in React state still creates the provider key in the DOM, renderer memory, and a secret-bearing IPC payload. That contradicts the governing trust boundary. Windows does not expose a suitable safe Rust API for generic password-only capture, and a second webview, console prompt, shell helper, or bundled GUI toolkit would either retain the same trust problem or add a much larger capability and supply-chain surface.

## Decision

Settings invokes an allowlisted, no-argument `prompt_store_api_key` Rust command. On Windows, Rust opens a modal operating-system credential dialog with fixed English application text and a password-only field by calling `CredUIPromptForCredentialsW` with generic-credential, password-only, always-show, exclude-certificate, and do-not-persist flags. The dialog's target is dedicated to capture and is not caller-controlled. A confirmed, validated key is explicitly stored by the existing application-specific Windows credential-store adapter; CredUI itself is instructed not to persist it. Cancellation leaves the existing entry unchanged. Only configured/cancelled status returns to React.

Microsoft [recommends the modern `CredUIPromptForWindowsCredentialsW` surface for Vista and later](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-creduipromptforcredentialsw) for current authentication UI, extensibility, and Common Criteria alignment. The older generic CredUI entry point is nevertheless deliberately selected for this non-Windows-authentication use: the modern provider surface exposes account-oriented fields and authentication packages that are irrelevant to an opaque API key, while the supported desktop API provides the narrowly required password-only generic mode. This caveat and the Windows 11 appearance/security behavior require packaged review; no credential validation call is made to the provider during setup.

The webview contains no API-key field, variable, adapter argument, command payload, or demo substitute. The old secret-bearing `store_api_key` command and permission are removed.

## Necessary unsafe boundary

Calling the Win32 function requires one isolated Windows-only FFI boundary. The main application crate continues to forbid unsafe code. A tiny local credential-prompt crate denies unsafe code globally and permits it only for the reviewed FFI function. These invariants are mandatory:

1. The FFI call receives only initialized, fixed-capacity UTF-16 buffers and NUL-terminated, application-owned constant strings. The password buffer has 257 UTF-16 units. The application accepts at most 255 characters and rejects a full 256-character result as potentially truncated, reserving one detectable unit before the terminating NUL. Lengths are passed in UTF-16 code units and cannot exceed their actual allocations; input is never silently truncated or persisted at the sentinel length.
2. `CREDUI_INFOW.cbSize` is the exact structure size, `hwndParent` is the validated Aster main-window handle, the banner handle is null, and every pointer remains live and immovable for the call duration.
3. CredUI receives `CREDUI_FLAGS_DO_NOT_PERSIST`; the application credential adapter remains the only persistence path.
4. Return codes are handled as success, cancellation, or a stable safe error. Buffer contents and raw Win32/provider errors are never formatted, logged, or returned.
5. Username and password buffers are wrapped in zeroizing guards before the call. The decoded password is immediately wrapped in a zeroizing Rust value, validated to the documented key bound, passed once to the credential adapter, and erased on every success/error/unwind path.
6. No unsafe pointer is stored, returned, dereferenced by application code, or shared across requests. The modal call runs in a bounded blocking task; only the numeric owner handle crosses into that task.
7. Static security tests fail if another application source file contains unsafe code or if the credential command regains a secret-bearing IPC argument.
8. Rust permits at most one credential prompt in flight. An RAII guard releases the prompt slot on confirmation, cancellation, error, or unwind; a concurrent invoke receives a stable busy error and cannot race credential persistence.

## Consequences

- A compromised renderer cannot read a key from a credential field or IPC capture.
- Setup is a native Windows-only workflow; browser demo mode cannot configure a key.
- The application contains one auditable FFI site and therefore requires focused static, unit, and packaged Windows review.
- Native prompt automation, cancellation, replacement, modal ownership across the blocking thread, app-exit behavior, keyboard accessibility, and scaling remain `NOT RUN` until exercised on the packaged candidate with an unmistakably fake isolated credential.
- The legacy CredUI presentation is a residual UX/platform risk despite its narrower API-key capture surface; an incompatible or misleading Windows 11 rendering blocks distribution until the decision is replaced or the native workflow is redesigned.

## Rejected alternatives

- Transient webview password input or an untracked React ref.
- A secret-bearing Tauri command argument.
- PowerShell, console, or another external helper process.
- A second Tauri webview presented as a trusted dialog.
- A broad GUI toolkit solely for one password field.
- Weakening the repository trust boundary to match an implementation shortcut.

## Verification

Unit tests cover result classification, input length handling, status-only serialization, and zeroizing buffer helpers without displaying a dialog or using a real key. Static assertions inventory the command/capability surface, forbid credential inputs in React, and restrict unsafe code to the reviewed module. `AC-005` retains the packaged native-dialog and isolated Windows credential-store test as required evidence.
