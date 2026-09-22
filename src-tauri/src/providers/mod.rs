//! Accounts, the Keychain, and the network services Minutes talks to.
pub mod clients;
pub mod keychain;
pub mod loopback;
pub mod oauth_service;
pub mod share_client;

use crate::core::{Error, Result};

/// Turns transport failures into a message the person can act on.
pub(crate) trait NetResult<T> {
    fn net(self) -> Result<T>;
}

impl<T> NetResult<T> for std::result::Result<T, reqwest::Error> {
    fn net(self) -> Result<T> {
        self.map_err(|error| {
            let reason = if error.is_timeout() { "The request timed out." } else { "Check your internet connection." };
            Error::message(format!("Minutes could not reach the service. {reason} ({error})"))
        })
    }
}
