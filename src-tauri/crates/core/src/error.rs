use serde::{Serialize, Serializer};

/// Every failure the app reports. Display strings are written for the person using Redrule.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // OAuth
    #[error("Paste the code or the full redirect URL.")]
    EmptyInput,
    #[error("Sign-in failed: {0}")]
    OAuthProvider(String),
    #[error("No authorization code was found in that text.")]
    MissingCode,
    #[error("The sign-in response did not match this request. Try connecting again.")]
    StateMismatch,
    #[error("The provider returned an unexpected token response.")]
    BadTokenResponse,

    // Summaries
    #[error("The model did not return notes in the expected format.")]
    NotJson,
    #[error("Nothing was transcribed, so there is nothing to summarise.")]
    EmptyTranscript,
    #[error("Connect ChatGPT or Grok, or choose to write notes on this Mac, in Settings.")]
    NoProviderConnected,

    /// A complete, user-facing message.
    #[error("{0}")]
    Message(String),

    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
}

impl Error {
    pub fn message(text: impl Into<String>) -> Self {
        Self::Message(text.into())
    }

    /// Prefixes the error with what the app was trying to do.
    pub fn context(self, what: &str) -> Self {
        Self::Message(format!("{what} {self}"))
    }
}

/// Commands return errors to the webview as their display string.
impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
