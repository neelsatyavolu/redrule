//! Reading and editing saved meetings.
use std::sync::Arc;

use chrono::Utc;
use serde::Serialize;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;

use super::state::App;
use crate::core::models::{ActionItem, MeetingNote, MeetingShare, NoteSection, TranscriptSegment};
use crate::core::export::{self, ExportFormat};
use crate::core::transcript;
use crate::core::{Error, Result};

const MAX_TITLE: usize = 300;
const MAX_TAG: usize = 40;
const MAX_TAGS: usize = 20;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingDetail {
    /// Merged for display.
    pub transcript: Vec<TranscriptSegment>,
    pub note: Option<MeetingNote>,
    pub share: Option<MeetingShare>,
}

impl App {
    pub fn meeting_detail(&self, id: &str) -> Result<MeetingDetail> {
        if self.meeting(id).is_none()
            && let Some(remote) = self.folder_meeting(id)
        {
            return Ok(MeetingDetail { transcript: remote.meeting.transcript, note: Some(remote.meeting.note), share: None });
        }
        let store = self.store()?;
        Ok(MeetingDetail {
            transcript: transcript::merge(&store.transcript(id)?),
            note: store.note(id)?,
            share: store.share(id)?,
        })
    }

    pub fn rename_meeting(&self, id: &str, title: &str) -> Result<()> {
        let meeting = self.editable(id)?;
        let title: String = title.trim().chars().take(MAX_TITLE).collect();
        self.store()?.rename(&meeting, &title).map_err(|e| e.context("The meeting could not be renamed."))?;
        self.reload_meetings();
        Ok(())
    }

    pub fn set_archived(&self, id: &str, archived: bool) -> Result<()> {
        let meeting = self.editable(id)?;
        self.persist(&meeting.archived(archived, Utc::now()));
        Ok(())
    }

    pub fn set_tags(&self, id: &str, tags: Vec<String>) -> Result<()> {
        let meeting = self.editable(id)?;
        self.persist(&meeting.tagged(clean_tags(tags)));
        Ok(())
    }

    pub async fn delete_meeting(self: &Arc<Self>, id: &str) -> Result<()> {
        self.editable(id)?;
        if self.read(|state| state.sharing_busy) {
            return Err(Error::message("Wait for sharing to finish, then delete the meeting."));
        }
        // A shared link must not outlive the meeting it came from.
        self.revoke_share(id).await?;
        self.store()?.delete(id).map_err(|e| e.context("The meeting could not be deleted."))?;
        self.reload_meetings();
        Ok(())
    }

    pub fn save_note(&self, id: &str, note: MeetingNote) -> Result<()> {
        let meeting = self.editable(id)?;
        let note = clean_note(note)?;
        let store = self.store()?;
        store.save_note(&note, id).map_err(|e| e.context("The notes could not be saved."))?;
        store.save(&meeting.titled(note.title.clone())).map_err(|e| e.context("The notes could not be saved."))?;
        self.reload_meetings();
        Ok(())
    }

    pub fn copy_markdown(&self, id: &str) -> Result<()> {
        let note = match self.folder_meeting(id).filter(|_| self.meeting(id).is_none()) {
            Some(remote) => remote.meeting.note,
            None => self.store()?.note(id)?.ok_or_else(|| Error::message("This meeting has no notes yet."))?,
        };
        self.handle.clipboard().write_text(note.markdown()).map_err(|e| Error::message(format!("The notes could not be copied. {e}")))
    }

    /// Asks where to save, then writes the file. Blocks on the save panel. Returns false when cancelled.
    pub fn export_meeting(&self, id: &str, format: ExportFormat) -> Result<bool> {
        let meeting = self
            .meeting(id)
            .or_else(|| self.read(|state| state.folder_meetings.iter().find(|m| m.meeting.id == id).map(|m| m.meeting.clone())))
            .ok_or_else(|| Error::message("That meeting does not exist."))?;
        let detail = self.meeting_detail(id)?;
        let text = export::render(format, &meeting, detail.note.as_ref(), &detail.transcript);
        let chosen = self
            .handle
            .dialog()
            .file()
            .set_file_name(export::file_name(&meeting, format))
            .add_filter(format.filter_name(), &[format.extension()])
            .blocking_save_file();
        let Some(path) = chosen.and_then(|file| file.into_path().ok()) else { return Ok(false) };
        std::fs::write(&path, text).map_err(|e| Error::message(format!("The file could not be saved. {e}")))?;
        Ok(true)
    }

    pub fn rename_speaker(&self, id: &str, key: &str, name: &str) -> Result<()> {
        self.store()?.rename_speaker(key, name, id).map_err(|e| e.context("The speaker name could not be saved."))?;
        self.reload_meetings();
        Ok(())
    }
}

/// Trims tags, drops a leading "#", and keeps the first spelling of each, ignoring case.
fn clean_tags(tags: Vec<String>) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for tag in tags {
        let tag: String = tag.trim().trim_start_matches('#').split_whitespace().collect::<Vec<_>>().join(" ");
        let tag: String = tag.chars().take(MAX_TAG).collect();
        if !tag.is_empty() && !kept.iter().any(|k| k.to_lowercase() == tag.to_lowercase()) {
            kept.push(tag);
        }
    }
    kept.truncate(MAX_TAGS);
    kept
}

/// Trims an edited note and drops empty entries, as the Swift editor did.
fn clean_note(note: MeetingNote) -> Result<MeetingNote> {
    let title: String = note.title.trim().chars().take(MAX_TITLE).collect();
    if title.is_empty() {
        return Err(Error::message("Give the meeting a title."));
    }
    let lines = |items: Vec<String>| -> Vec<String> {
        items.into_iter().map(|item| item.trim().to_string()).filter(|item| !item.is_empty()).collect()
    };
    Ok(MeetingNote {
        title,
        tldr: note.tldr.trim().to_string(),
        sections: note
            .sections
            .into_iter()
            .map(|section| NoteSection { heading: section.heading.trim().to_string(), bullets: lines(section.bullets) })
            .filter(|section| !section.heading.is_empty() || !section.bullets.is_empty())
            .collect(),
        decisions: lines(note.decisions),
        action_items: note
            .action_items
            .into_iter()
            .filter_map(|item| {
                let task = item.task.trim().to_string();
                let owner = item.owner.map(|o| o.trim().to_string()).filter(|o| !o.is_empty());
                (!task.is_empty()).then_some(ActionItem { owner, task, done: item.done })
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_edited_notes() {
        let note = MeetingNote {
            title: "  Plan ".into(),
            tldr: " Summary ".into(),
            sections: vec![
                NoteSection { heading: " A ".into(), bullets: vec![" one ".into(), "  ".into()] },
                NoteSection { heading: " ".into(), bullets: vec![] },
            ],
            decisions: vec!["".into(), "Ship".into()],
            action_items: vec![
                ActionItem { owner: Some(" ".into()), task: " Do it ".into(), done: true },
                ActionItem { owner: Some("Ada".into()), task: " ".into(), done: false },
            ],
        };
        let cleaned = clean_note(note).unwrap();
        assert_eq!(cleaned.title, "Plan");
        assert_eq!(cleaned.sections, vec![NoteSection { heading: "A".into(), bullets: vec!["one".into()] }]);
        assert_eq!(cleaned.decisions, vec!["Ship".to_string()]);
        assert_eq!(cleaned.action_items, vec![ActionItem { owner: None, task: "Do it".into(), done: true }]);
    }

    #[test]
    fn cleans_tags() {
        let tags = vec![" #Acme ".into(), "acme".into(), "  ".into(), "Hiring   loop".into(), "x".repeat(50)];
        assert_eq!(clean_tags(tags), vec!["Acme".to_string(), "Hiring loop".into(), "x".repeat(MAX_TAG)]);
        assert_eq!(clean_tags((0..30).map(|n| n.to_string()).collect()).len(), MAX_TAGS);
    }

    #[test]
    fn requires_a_title() {
        let note = MeetingNote { title: " ".into(), tldr: "".into(), sections: vec![], decisions: vec![], action_items: vec![] };
        assert!(clean_note(note).is_err());
    }
}
