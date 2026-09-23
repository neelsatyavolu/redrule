//! Calls to the shared-folder service on the Redrule site.
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::share_client::BASE_URL;
use super::{NetResult, keychain};
use crate::core::folders::{RemoteMeeting, SharedMeeting};
use crate::core::{Error, Result};

const TIMEOUT: Duration = Duration::from_secs(45);
const MAX_PAYLOAD: usize = 2_000_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Created {
    pub id: String,
    pub member_key: String,
    pub owner_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub name: String,
    pub meetings: Vec<Entry>,
    #[serde(default)]
    pub member: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reset {
    pub id: String,
    pub member_key: String,
}

#[derive(Serialize)]
struct Named<'a> {
    name: &'a str,
}

#[derive(Deserialize)]
struct ServiceError {
    error: String,
}

/// How a request proves who is asking.
pub enum Auth<'a> {
    None,
    Bearer(&'a str),
    /// A member uploading or removing a meeting it added.
    Editor { member_key: &'a str, edit_key: &'a str },
}

pub struct FolderClient<'a> {
    pub http: &'a reqwest::Client,
}

impl FolderClient<'_> {
    pub async fn create(&self, name: &str) -> Result<Created> {
        let key = keychain::sharing_key()
            .map_err(|_| Error::message("Folders can only be created on the Mac that runs the Redrule sharing service."))?;
        self.send(reqwest::Method::POST, "folder", &[], Auth::Bearer(&key), Some(&Named { name })).await?.ok_or_else(gone)
    }

    /// The folder's name and meetings, or None when its link no longer works.
    pub async fn listing(&self, id: &str, member_key: &str) -> Result<Option<Listing>> {
        self.send::<_, ()>(reqwest::Method::GET, "folder", &[("id", id)], Auth::Bearer(member_key), None).await
    }

    pub async fn rename(&self, id: &str, owner_key: &str, name: &str) -> Result<()> {
        self.send::<serde_json::Value, _>(reqwest::Method::PATCH, "folder", &[("id", id)], Auth::Bearer(owner_key), Some(&Named { name }))
            .await?
            .map(drop)
            .ok_or_else(gone)
    }

    pub async fn delete(&self, id: &str, owner_key: &str) -> Result<()> {
        self.send::<serde_json::Value, ()>(reqwest::Method::DELETE, "folder", &[("id", id)], Auth::Bearer(owner_key), None).await.map(drop)
    }

    pub async fn reset(&self, id: &str, owner_key: &str) -> Result<Reset> {
        self.send::<_, ()>(reqwest::Method::POST, "folder-reset", &[("id", id)], Auth::Bearer(owner_key), None).await?.ok_or_else(gone)
    }

    pub async fn meeting(&self, id: &str, meeting_id: &str) -> Result<Option<RemoteMeeting>> {
        self.send::<_, ()>(reqwest::Method::GET, "folder-meeting", &[("id", id), ("meeting", meeting_id)], Auth::None, None).await
    }

    pub async fn upload(&self, id: &str, meeting_id: &str, auth: Auth<'_>, meeting: &SharedMeeting) -> Result<()> {
        if serde_json::to_vec(meeting)?.len() > MAX_PAYLOAD {
            return Err(Error::message(format!("“{}” is too large to add to a shared folder.", meeting.title)));
        }
        self.send::<serde_json::Value, _>(reqwest::Method::PUT, "folder-meeting", &[("id", id), ("meeting", meeting_id)], auth, Some(meeting))
            .await?
            .map(drop)
            .ok_or_else(gone)
    }

    /// Removing a meeting that is already gone succeeds.
    pub async fn remove(&self, id: &str, meeting_id: &str, auth: Auth<'_>) -> Result<()> {
        self.send::<serde_json::Value, ()>(reqwest::Method::DELETE, "folder-meeting", &[("id", id), ("meeting", meeting_id)], auth, None)
            .await
            .map(drop)
    }

    /// Sends one request. A 404 is None; an empty success body reads as JSON null.
    async fn send<T: DeserializeOwned, B: Serialize>(
        &self,
        method: reqwest::Method,
        endpoint: &str,
        query: &[(&str, &str)],
        auth: Auth<'_>,
        body: Option<&B>,
    ) -> Result<Option<T>> {
        // Always our configured host; a pasted link never chooses where keys are sent.
        let mut request = self.http.request(method, format!("{BASE_URL}/api/{endpoint}")).query(query).timeout(TIMEOUT);
        request = match auth {
            Auth::None => request,
            Auth::Bearer(key) => request.bearer_auth(key),
            Auth::Editor { member_key, edit_key } => request.bearer_auth(member_key).header("X-Edit-Key", edit_key),
        };
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request.send().await.net()?;
        let status = response.status();
        // Our handlers answer a missing folder or meeting with 404; Vercel's own 404 (a missing route) is an error.
        if status == reqwest::StatusCode::NOT_FOUND && !response.headers().contains_key("x-vercel-error") {
            return Ok(None);
        }
        if status.is_success() {
            let bytes = response.bytes().await.net()?;
            let text = if bytes.is_empty() { &b"null"[..] } else { &bytes[..] };
            return Ok(Some(serde_json::from_slice(text)?));
        }
        let message = response.json::<ServiceError>().await.map(|e| e.error);
        Err(Error::message(message.unwrap_or_else(|_| format!("The shared folder service could not complete the request ({status}). Try again."))))
    }
}

fn gone() -> Error {
    Error::message("This folder's link no longer works. It may have been reset or deleted.")
}
