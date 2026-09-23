//! Publishes and revokes read-only links on the Redrule sharing service.
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{NetResult, keychain};
use crate::core::models::{MeetingNote, MeetingShare, TranscriptSegment};
use crate::core::transcript::timestamp;
use crate::core::{Error, Result};

pub const BASE_URL: &str = "https://redrule.vercel.app";
const MAX_PAYLOAD: usize = 2_000_000;
const TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Serialize)]
struct Payload<'a> {
    note: &'a MeetingNote,
    #[serde(skip_serializing_if = "Option::is_none")]
    transcript: Option<Vec<Segment>>,
}

#[derive(Serialize)]
struct Segment {
    speaker: String,
    time: String,
    text: String,
}

#[derive(Deserialize)]
struct ServiceError {
    error: String,
}

pub fn share_url(id: &str) -> String {
    format!("{BASE_URL}/s/{id}")
}

/// Share ids are 64 lowercase hex characters: unguessable, and safe in a URL.
pub fn new_share_id() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

pub async fn publish(http: &reqwest::Client, share: &MeetingShare, note: &MeetingNote, transcript: &[TranscriptSegment]) -> Result<()> {
    let transcript = share.includes_transcript.then(|| {
        transcript
            .iter()
            .map(|s| Segment { speaker: s.speaker_label(), time: timestamp(s.start), text: s.text.clone() })
            .collect()
    });
    let body = serde_json::to_vec(&Payload { note, transcript })?;
    if body.len() > MAX_PAYLOAD {
        return Err(Error::message("This meeting is too large to share."));
    }
    request(http, share, reqwest::Method::PUT, Some(body)).await
}

pub async fn revoke(http: &reqwest::Client, share: &MeetingShare) -> Result<()> {
    request(http, share, reqwest::Method::DELETE, None).await
}

async fn request(http: &reqwest::Client, share: &MeetingShare, method: reqwest::Method, body: Option<Vec<u8>>) -> Result<()> {
    // Always use our configured host; stored links cannot redirect upload credentials.
    let mut request = http
        .request(method, format!("{BASE_URL}/api/share"))
        .query(&[("id", &share.id)])
        .timeout(TIMEOUT)
        .bearer_auth(keychain::sharing_key()?)
        .header("Content-Type", "application/json");
    if let Some(body) = body {
        request = request.body(body);
    }
    let response = request.send().await.net()?;
    if response.status().is_success() {
        return Ok(());
    }
    let message = response.json::<ServiceError>().await.map(|e| e.error);
    Err(Error::message(message.unwrap_or_else(|_| "Sharing could not complete. Check your connection and try again.".into())))
}
