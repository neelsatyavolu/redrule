//! Generic passwords in the login keychain, under the service "Minutes" (shared with the Swift app).
use security_framework::passwords::{delete_generic_password, get_generic_password, set_generic_password};

use crate::core::oauth::{ProviderId, TokenBundle};
use crate::core::{Error, Result};

const SERVICE: &str = "Minutes";
const SHARING_ACCOUNT: &str = "sharing";
const ITEM_NOT_FOUND: i32 = -25300;

fn describe(error: security_framework::base::Error) -> Error {
    let message = error.message().unwrap_or_else(|| format!("code {}", error.code()));
    Error::message(format!("Keychain error: {message}"))
}

fn load(account: &str) -> Result<Option<Vec<u8>>> {
    match get_generic_password(SERVICE, account) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.code() == ITEM_NOT_FOUND => Ok(None),
        Err(error) => Err(describe(error)),
    }
}

pub fn load_tokens(provider: ProviderId) -> Result<Option<TokenBundle>> {
    // An unreadable item is treated as signed out, as the Swift app did.
    Ok(load(provider.raw())?.and_then(|bytes| serde_json::from_slice(&bytes).ok()))
}

pub fn save_tokens(bundle: &TokenBundle, provider: ProviderId) -> Result<()> {
    set_generic_password(SERVICE, provider.raw(), &serde_json::to_vec(bundle)?).map_err(describe)
}

pub fn delete_tokens(provider: ProviderId) -> Result<()> {
    match delete_generic_password(SERVICE, provider.raw()) {
        Err(error) if error.code() != ITEM_NOT_FOUND => Err(describe(error)),
        _ => Ok(()),
    }
}

/// The upload key for the sharing service, provisioned by `scripts/setup-sharing.swift`.
pub fn sharing_key() -> Result<String> {
    let bytes = load(SHARING_ACCOUNT)?.ok_or_else(|| {
        Error::message("Sharing is not configured on this Mac. Run the sharing setup script from the Minutes project.")
    })?;
    String::from_utf8(bytes).map_err(|_| Error::message("The sharing key in the Keychain is not readable."))
}
