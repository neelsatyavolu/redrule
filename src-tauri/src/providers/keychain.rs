//! Generic passwords in the login keychain, under the service "Redrule".
//! Items saved under the previous service name are read and copied across.
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

fn text(bytes: Option<Vec<u8>>) -> Result<Option<String>> {
    bytes.map(|b| String::from_utf8(b).map_err(|_| Error::message("A sharing key in the Keychain is not readable."))).transpose()
}

/// The legacy service key from `scripts/setup-sharing.swift`, if this Mac has one. Links made before owner keys need it.
pub fn sharing_key() -> Result<Option<String>> {
    text(load(SHARING_ACCOUNT)?)
}

fn share_account(share_id: &str) -> String {
    format!("share:{share_id}")
}

/// The owner key of one shared link, or None for a link made before owner keys.
pub fn share_key(share_id: &str) -> Result<Option<String>> {
    text(read(SERVICE, &share_account(share_id))?)
}

pub fn save_share_key(share_id: &str, key: &str) -> Result<()> {
    set_generic_password(SERVICE, &share_account(share_id), key.as_bytes()).map_err(describe)
}

pub fn delete_share_key(share_id: &str) -> Result<()> {
    remove(SERVICE, &share_account(share_id))
}
