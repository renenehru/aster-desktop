#![deny(unsafe_code)]

use zeroize::Zeroizing;

pub const MIN_API_KEY_BYTES: usize = 8;
pub const MAX_API_KEY_BYTES: usize = 255;

pub enum PromptOutcome {
    Submitted(Zeroizing<String>),
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptError {
    InvalidOwner,
    InvalidSecret,
    Unavailable,
}

pub fn prompt_api_key(owner_window: isize) -> Result<PromptOutcome, PromptError> {
    if owner_window == 0 {
        return Err(PromptError::InvalidOwner);
    }
    windows_prompt::prompt(owner_window)
}

fn decode_password(buffer: &[u16]) -> Result<Zeroizing<String>, PromptError> {
    let terminator = buffer
        .iter()
        .position(|unit| *unit == 0)
        .ok_or(PromptError::InvalidSecret)?;
    let value = Zeroizing::new(
        String::from_utf16(&buffer[..terminator]).map_err(|_| PromptError::InvalidSecret)?,
    );
    if value.len() < MIN_API_KEY_BYTES
        || value.len() > MAX_API_KEY_BYTES
        || !value.chars().all(|character| character.is_ascii_graphic())
    {
        return Err(PromptError::InvalidSecret);
    }
    Ok(value)
}

mod windows_prompt {
    use std::ffi::c_void;
    use std::mem::size_of;

    use windows::Win32::Foundation::{ERROR_CANCELLED, HWND, NO_ERROR};
    use windows::Win32::Graphics::Gdi::HBITMAP;
    use windows::Win32::Security::Credentials::{
        CREDUI_FLAGS, CREDUI_FLAGS_ALWAYS_SHOW_UI, CREDUI_FLAGS_DO_NOT_PERSIST,
        CREDUI_FLAGS_EXCLUDE_CERTIFICATES, CREDUI_FLAGS_GENERIC_CREDENTIALS,
        CREDUI_FLAGS_KEEP_USERNAME, CREDUI_FLAGS_PASSWORD_ONLY_OK, CREDUI_INFOW,
        CredUIPromptForCredentialsW,
    };
    use windows::core::w;
    use zeroize::Zeroizing;

    use super::{PromptError, PromptOutcome, decode_password};

    const USERNAME_CAPACITY: usize = 514;
    const PASSWORD_CAPACITY: usize = 257;
    const USERNAME: [u16; 12] = [
        b'z' as u16,
        b'a' as u16,
        b'i' as u16,
        b'-' as u16,
        b'a' as u16,
        b'p' as u16,
        b'i' as u16,
        b'-' as u16,
        b'k' as u16,
        b'e' as u16,
        b'y' as u16,
        0,
    ];

    pub(super) fn prompt(owner_window: isize) -> Result<PromptOutcome, PromptError> {
        let mut username = Zeroizing::new([0_u16; USERNAME_CAPACITY]);
        username[..USERNAME.len()].copy_from_slice(&USERNAME);
        let mut password = Zeroizing::new([0_u16; PASSWORD_CAPACITY]);
        let information = CREDUI_INFOW {
            cbSize: u32::try_from(size_of::<CREDUI_INFOW>())
                .map_err(|_| PromptError::Unavailable)?,
            hwndParent: HWND(owner_window as *mut c_void),
            pszMessageText: w!(
                "Enter your Z.AI API key. Aster stores it in Windows Credential Manager."
            ),
            pszCaptionText: w!("Aster credential setup"),
            hbmBanner: HBITMAP::default(),
        };
        let flags = prompt_flags();

        let return_code = invoke_credui(&information, &mut username, &mut password, flags);

        if return_code == NO_ERROR {
            decode_password(password.as_slice()).map(PromptOutcome::Submitted)
        } else if return_code == ERROR_CANCELLED {
            Ok(PromptOutcome::Cancelled)
        } else {
            Err(PromptError::Unavailable)
        }
    }

    #[allow(unsafe_code)]
    fn invoke_credui(
        information: &CREDUI_INFOW,
        username: &mut [u16; USERNAME_CAPACITY],
        password: &mut [u16; PASSWORD_CAPACITY],
        flags: CREDUI_FLAGS,
    ) -> windows::Win32::Foundation::WIN32_ERROR {
        // SAFETY: ADR-0008 owns this single FFI site. `information` contains the
        // validated nonzero main-window HWND, fixed NUL-terminated static text,
        // an exact structure size, and a null banner. Both mutable slices are
        // initialized, fixed-capacity, live and immovable for this synchronous
        // call; their lengths are their actual UTF-16 allocations. CredUI is
        // explicitly forbidden from persisting the returned credential. The
        // caller's zeroizing guards erase both buffers on success, cancellation,
        // error, or unwind, and no pointer escapes this call.
        unsafe {
            CredUIPromptForCredentialsW(
                Some(information),
                w!("Aster:Z.AI API key capture"),
                None,
                0,
                username,
                password,
                None,
                flags,
            )
        }
    }

    fn prompt_flags() -> CREDUI_FLAGS {
        CREDUI_FLAGS_GENERIC_CREDENTIALS
            | CREDUI_FLAGS_PASSWORD_ONLY_OK
            | CREDUI_FLAGS_KEEP_USERNAME
            | CREDUI_FLAGS_ALWAYS_SHOW_UI
            | CREDUI_FLAGS_EXCLUDE_CERTIFICATES
            | CREDUI_FLAGS_DO_NOT_PERSIST
    }

    #[cfg(test)]
    pub(super) fn expected_flags() -> CREDUI_FLAGS {
        prompt_flags()
    }
}

#[cfg(test)]
mod tests {
    use windows::Win32::Security::Credentials::{
        CREDUI_FLAGS_ALWAYS_SHOW_UI, CREDUI_FLAGS_DO_NOT_PERSIST,
        CREDUI_FLAGS_EXCLUDE_CERTIFICATES, CREDUI_FLAGS_GENERIC_CREDENTIALS,
        CREDUI_FLAGS_KEEP_USERNAME, CREDUI_FLAGS_PASSWORD_ONLY_OK,
    };
    use zeroize::{Zeroize, Zeroizing};

    use super::*;

    #[test]
    fn decoded_keys_are_bounded_printable_ascii_and_never_truncated() {
        for length in [MIN_API_KEY_BYTES, MAX_API_KEY_BYTES] {
            let mut valid = [0_u16; 257];
            valid[..length].fill(b'x' as u16);
            assert_eq!(
                decode_password(&valid).expect("valid boundary").len(),
                length
            );
        }

        let mut too_long = [b'x' as u16; 257];
        too_long[256] = 0;
        assert!(matches!(
            decode_password(&too_long),
            Err(PromptError::InvalidSecret)
        ));
        assert!(matches!(
            decode_password(&[
                b'x' as u16,
                b'x' as u16,
                b'x' as u16,
                b'x' as u16,
                b'x' as u16,
                b'x' as u16,
                b'x' as u16,
                0,
            ]),
            Err(PromptError::InvalidSecret)
        ));
        assert!(matches!(
            decode_password(&[0x00e9, 0]),
            Err(PromptError::InvalidSecret)
        ));
        assert!(matches!(
            decode_password(&[b'x' as u16, b'\n' as u16, 0]),
            Err(PromptError::InvalidSecret)
        ));
        assert!(matches!(
            decode_password(&[b'x' as u16; 257]),
            Err(PromptError::InvalidSecret)
        ));
    }

    #[test]
    fn prompt_flags_are_exact_and_disable_credui_persistence() {
        let expected = CREDUI_FLAGS_GENERIC_CREDENTIALS
            | CREDUI_FLAGS_PASSWORD_ONLY_OK
            | CREDUI_FLAGS_KEEP_USERNAME
            | CREDUI_FLAGS_ALWAYS_SHOW_UI
            | CREDUI_FLAGS_EXCLUDE_CERTIFICATES
            | CREDUI_FLAGS_DO_NOT_PERSIST;
        assert_eq!(windows_prompt::expected_flags(), expected);
    }

    #[test]
    fn wide_secret_storage_can_be_zeroized_in_place() {
        let mut buffer = Zeroizing::new([42_u16; 257]);
        buffer.zeroize();
        assert!(buffer.iter().all(|unit| *unit == 0));
    }

    #[test]
    fn unsafe_code_is_confined_to_one_reviewed_ffi_site() {
        let source = include_str!("lib.rs");
        let unsafe_block = concat!("unsafe", " {");
        let unsafe_allow = concat!("allow", "(unsafe_code)");
        let unsafe_function = concat!("unsafe", " fn");
        let unsafe_implementation = concat!("unsafe", " impl");
        let unsafe_external = concat!("unsafe", " extern");
        assert_eq!(source.matches(unsafe_block).count(), 1);
        assert_eq!(source.matches(unsafe_allow).count(), 1);
        assert_eq!(source.matches(unsafe_function).count(), 0);
        assert_eq!(source.matches(unsafe_implementation).count(), 0);
        assert_eq!(source.matches(unsafe_external).count(), 0);
    }
}
