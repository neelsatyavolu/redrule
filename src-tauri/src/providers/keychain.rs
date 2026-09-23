//! Generic passwords in the login keychain, under the service "Redrule".
//! Items saved while the app was called Minutes are read from the old service and copied across.
use security_framework::passwords::{delete_generic_password, get_generic_password, set_generic_password};

use crate::core::oauth::{ProviderId, TokenBundle};
use crate::core::{Error, Result};

const SERVICE: &str = "Redrule";
const LEGACY_SERVICE: &str = "Minutes";
const SHARING_ACCOUNT: &str = "sharing";
const ITEM_NOT_FOUND: i32 = -25300;

fn describe(error: security_framework::base::Error) -> Error {
    let message = error.message().unwrap_or_else(|| format!("code {}", error.code()));
    Error::message(format!("Keychain error: {message}"))
}

fn read(service: &str, account: &str) -> Result<Option<Vec<u8>>> {
    match get_generic_password(service, account) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.code() == ITEM_NOT_FOUND => Ok(None),
        Err(error) => Err(describe(error)),
    }
}

fn load(account: &str) -> Result<Option<Vec<u8>>> {
    if let Some(bytes) = read(SERVICE, account)? {
        return Ok(Some(bytes));
    }
    let Some(bytes) = read(LEGACY_SERVICE, account)? else { return Ok(None) };
    // Copying is best effort: the old item keeps working if the new one cannot be written.
    let _ = set_generic_password(SERVICE, account, &bytes);
    Ok(Some(bytes))
}

fn remove(service: &str, account: &str) -> Result<()> {
    match delete_generic_password(service, account) {
        Err(error) if error.code() != ITEM_NOT_FOUND => Err(describe(error)),
        _ => Ok(()),
    }
}

pub fn load_tokens(provider: ProviderId) -> Result<Option<TokenBundle>> {
    // An unreadable item is treated as signed out, as the Swift app did.
    Ok(load(provider.raw())?.and_then(|bytes| serde_json::from_slice(&bytes).ok()))
}

pub fn save_tokens(bundle: &TokenBundle, provider: ProviderId) -> Result<()> {
    set_generic_password(SERVICE, provider.raw(), &serde_json::to_vec(bundle)?).map_err(describe)
}

/// Removes the old copy too, so disconnecting cannot be undone by the migration above.
pub fn delete_tokens(provider: ProviderId) -> Result<()> {
    remove(SERVICE, provider.raw())?;
    remove(LEGACY_SERVICE, provider.raw())
}

/// The upload key for the sharing service, provisioned by `scripts/setup-sharing.swift`.
pub fn sharing_key() -> Result<String> {
    let bytes = load(SHARING_ACCOUNT)?.ok_or_else(|| {
        Error::message("Sharing is not configured on this Mac. Run the sharing setup script from the Redrule project.")
    })?;
    String::from_utf8(bytes).map_err(|_| Error::message("The sharing key in the Keychain is not readable."))
}
