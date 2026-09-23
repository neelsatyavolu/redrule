//! Searching every meeting's notes and transcript, with each meeting's text cached until its files change.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::state::App;
use crate::core::Result;
use crate::core::models::Meeting;
use crate::core::search::{SearchDocument, SearchHit, terms};

/// Documents by meeting id, with the stamp they were built from.
#[derive(Default)]
pub struct SearchCache(Mutex<HashMap<String, (String, Arc<SearchDocument>)>>);

impl App {
    /// Meetings on this Mac and in joined folders whose title, tags, notes or transcript hold every word of `query`.
    pub fn search_meetings(&self, query: &str) -> Vec<SearchHit> {
        let terms = terms(query);
        if terms.is_empty() {
            return Vec::new();
        }
        let (local, remote) = self.read(|state| {
            let remote: Vec<(Meeting, String)> =
                state.folder_meetings.iter().map(|m| (m.meeting.clone(), m.updated_at.clone())).collect();
            (state.meetings.clone(), remote)
        });
        let documents: Vec<Arc<SearchDocument>> = local
            .iter()
            .filter_map(|meeting| self.local_document(meeting).ok())
            .chain(remote.iter().filter_map(|(meeting, updated_at)| self.remote_document(meeting, updated_at)))
            .collect();
        let live: Vec<&str> = local.iter().map(|m| m.id.as_str()).chain(remote.iter().map(|(m, _)| m.id.as_str())).collect();
        self.search_cache.0.lock().unwrap().retain(|id, _| live.contains(&id.as_str()));
        documents.iter().filter_map(|document| document.search(&terms)).collect()
    }

    fn local_document(&self, meeting: &Meeting) -> Result<Arc<SearchDocument>> {
        let store = self.store()?;
        let folder = store.folder(&meeting.id)?;
        let modified = |name: &str| std::fs::metadata(folder.join(name)).and_then(|m| m.modified()).ok();
        let stamp = format!("{:?}", [modified("meeting.json"), modified("note.json"), modified("transcript.json")]);
        self.cached(&meeting.id, stamp, || {
            let note = store.note(&meeting.id).ok().flatten();
            let transcript = store.transcript(&meeting.id).unwrap_or_default();
            Some(SearchDocument::new(meeting, note.as_ref(), &transcript))
        })
        .ok_or_else(|| crate::core::Error::message("The meeting could not be read."))
    }

    fn remote_document(&self, meeting: &Meeting, updated_at: &str) -> Option<Arc<SearchDocument>> {
        let folder_id = meeting.folder_id.as_deref()?;
        self.cached(&meeting.id, updated_at.to_string(), || {
            let remote = self.folders.cache.meeting(folder_id, &meeting.id).ok().flatten()?;
            Some(SearchDocument::new(meeting, Some(&remote.meeting.note), &remote.meeting.transcript))
        })
    }

    fn cached(&self, id: &str, stamp: String, build: impl FnOnce() -> Option<SearchDocument>) -> Option<Arc<SearchDocument>> {
        if let Some((kept, document)) = self.search_cache.0.lock().unwrap().get(id)
            && *kept == stamp
        {
            return Some(Arc::clone(document));
        }
        // Built outside the lock: the first search reads every meeting from disk.
        let document = Arc::new(build()?);
        self.search_cache.0.lock().unwrap().insert(id.to_string(), (stamp, Arc::clone(&document)));
        Some(document)
    }
}
