//! Answering questions about a saved meeting with the model chosen for questions.
use minutes_engine::LocalNoteWriter;

use super::state::App;
use crate::core::ask::{self, ACCOUNT_BUDGET, Exchange, MAX_QUESTION};
use crate::core::{Error, Result};

impl App {
    pub async fn ask_meeting(&self, id: &str, question: &str, history: &[Exchange]) -> Result<String> {
        let question = question.trim();
        if question.is_empty() {
            return Err(Error::message("Type a question first."));
        }
        if question.chars().count() > MAX_QUESTION {
            return Err(Error::message("That question is too long. Keep it under 2,000 characters."));
        }
        if self.read(|state| state.recording_id.as_deref() == Some(id)) {
            return Err(Error::message("You can ask about this meeting once the recording ends."));
        }
        let meeting = self.meeting(id).ok_or_else(|| Error::message("This meeting no longer exists."))?;
        let store = self.store()?;
        let (note, segments) = (store.note(id)?, store.transcript(id)?);

        match self.read(|state| state.settings.local_ask_model()) {
            Some(option) => {
                let prompt = ask::prompt(&meeting, note.as_ref(), &segments, history, question, option.chunk_chars)?;
                let _one_at_a_time = self.local_notes.lock().await;
                let dir = self.models_dir.clone();
                let writer = tauri::async_runtime::spawn_blocking(move || LocalNoteWriter::load(option, &dir))
                    .await
                    .map_err(|error| Error::message(error.to_string()))??;
                ask::answer(&writer, &prompt).await
            }
            None => {
                let prompt = ask::prompt(&meeting, note.as_ref(), &segments, history, question, ACCOUNT_BUDGET)?;
                let preferred = self.read(|state| state.settings.ask_model_choice());
                ask::answer(&self.account_client(preferred)?, &prompt).await
            }
        }
    }
}
