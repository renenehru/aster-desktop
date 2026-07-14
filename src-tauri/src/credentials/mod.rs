use aster_credential_prompt::{MAX_API_KEY_BYTES, MIN_API_KEY_BYTES};
use keyring::v1::{Entry, Error as KeyringError};
use zeroize::Zeroizing;

use crate::error::{AppError, AppResult};
use crate::models::{CredentialStatus, ProviderId};

const SERVICE: &str = "com.aster.desktop";

pub(crate) const fn credential_target(provider: ProviderId) -> &'static str {
    match provider {
        // This exact legacy target is intentionally preserved for existing MVP v1 users.
        ProviderId::Zai => "zai-api-key",
        ProviderId::DeepSeek => "deepseek-api-key",
        ProviderId::AlibabaUs => "alibaba-us-api-key",
        ProviderId::Google => "google-api-key",
        ProviderId::Nvidia => "nvidia-api-key",
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CredentialStore;

impl CredentialStore {
    fn entry(self, provider: ProviderId) -> AppResult<Entry> {
        Entry::new(SERVICE, credential_target(provider)).map_err(|_| AppError::CredentialVault)
    }

    pub fn status(self, provider: ProviderId) -> AppResult<CredentialStatus> {
        match self.entry(provider)?.get_password() {
            Ok(secret) => {
                let secret = Zeroizing::new(secret);
                validate_api_key(secret.as_str())?;
                Ok(CredentialStatus {
                    provider_id: provider,
                    configured: true,
                    source: "credential-vault",
                })
            }
            Err(KeyringError::NoEntry) => Ok(CredentialStatus {
                provider_id: provider,
                configured: false,
                source: "none",
            }),
            Err(_) => Err(AppError::CredentialVault),
        }
    }

    pub fn store(
        self,
        provider: ProviderId,
        api_key: Zeroizing<String>,
    ) -> AppResult<CredentialStatus> {
        validate_api_key(api_key.as_str())?;
        self.entry(provider)?
            .set_password(api_key.as_str())
            .map_err(|_| AppError::CredentialVault)?;
        Ok(CredentialStatus {
            provider_id: provider,
            configured: true,
            source: "credential-vault",
        })
    }

    pub fn delete(self, provider: ProviderId) -> AppResult<CredentialStatus> {
        match self.entry(provider)?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(CredentialStatus {
                provider_id: provider,
                configured: false,
                source: "none",
            }),
            Err(_) => Err(AppError::CredentialVault),
        }
    }

    pub fn load(self, provider: ProviderId) -> AppResult<Zeroizing<String>> {
        match self.entry(provider)?.get_password() {
            Ok(secret) => {
                let secret = Zeroizing::new(secret);
                validate_api_key(secret.as_str())?;
                Ok(secret)
            }
            Err(KeyringError::NoEntry) => Err(AppError::CredentialNotConfigured),
            Err(_) => Err(AppError::CredentialVault),
        }
    }
}

fn validate_api_key(value: &str) -> AppResult<()> {
    if value.len() < MIN_API_KEY_BYTES || value.len() > MAX_API_KEY_BYTES {
        return Err(AppError::Validation(
            "The API key must contain 8 to 255 printable ASCII characters.",
        ));
    }
    if !value.chars().all(|character| character.is_ascii_graphic()) {
        return Err(AppError::Validation(
            "The API key must contain only printable ASCII characters without whitespace.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_targets_are_exact_and_preserve_legacy_zai_target() {
        assert_eq!(credential_target(ProviderId::Zai), "zai-api-key");
        assert_eq!(credential_target(ProviderId::DeepSeek), "deepseek-api-key");
        assert_eq!(
            credential_target(ProviderId::AlibabaUs),
            "alibaba-us-api-key"
        );
        assert_eq!(credential_target(ProviderId::Google), "google-api-key");
        assert_eq!(credential_target(ProviderId::Nvidia), "nvidia-api-key");
    }

    #[test]
    fn api_key_validation_enforces_the_native_prompt_bound() {
        assert!(validate_api_key("short").is_err());
        assert!(validate_api_key(" valid-key").is_err());
        assert!(validate_api_key("valid\nkey").is_err());
        assert!(validate_api_key("valid-key-\u{00e9}").is_err());
        assert!(validate_api_key("valid-key-value").is_ok());
        assert!(validate_api_key(&"x".repeat(MAX_API_KEY_BYTES)).is_ok());
        assert!(validate_api_key(&"x".repeat(MAX_API_KEY_BYTES + 1)).is_err());
    }
}
