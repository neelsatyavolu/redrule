import { escape, renderPage, renderArticle } from './notes.js';

const MONTHS = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec'];
export function formatDate(value) {
  const d = new Date(value);
  return Number.isNaN(d.getTime()) ? '' : `${d.getUTCDate()} ${MONTHS[d.getUTCMonth()]} ${d.getUTCFullYear()}`;
}
export function formatTime(seconds) {
  const s = Math.max(0, Math.floor(seconds));
  return `${String(Math.floor(s / 60)).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`;
}
const speakerLabel = (s, recordedBy) => s.speakerName || (s.speakerID ? `Speaker ${s.speakerID}` : s.speaker === 'me' ? recordedBy : 'Them');

// meetings: [{id, title, startedAt, recordedBy}] as returned by meetingSummaries.
export function renderFolder(id, folder, meetings) {
  const sorted = [...meetings].sort((a, b) => Date.parse(b.startedAt) - Date.parse(a.startedAt));
  const items = sorted.map(m => `<li><a href="/f/${id}/m/${m.id}">${escape(m.title || 'Untitled meeting')}</a><span class="owner">${escape(formatDate(m.startedAt))} · Recorded by ${escape(m.recordedBy)}</span></li>`).join('');
  const count = `${meetings.length} ${meetings.length === 1 ? 'meeting' : 'meetings'}`;
  return renderPage(folder.name, `<article><p class="eyebrow">SHARED FOLDER</p><h1>${escape(folder.name)}</h1><p class="summary">${count}</p>${meetings.length ? `<ul>${items}</ul>` : ''}</article>`);
}

export function renderFolderMeeting(id, folder, m) {
  const transcript = m.transcript.map(s => ({speaker:speakerLabel(s, m.recordedBy), time:formatTime(s.start), text:s.text}));
  const article = renderArticle({note:m.note, transcript:transcript.length ? transcript : undefined}, {
    eyebrow:`<a href="/f/${id}">← ${escape(folder.name)}</a>`,
    byline:`<p class="owner">Recorded by ${escape(m.recordedBy)} · ${escape(formatDate(m.startedAt))}</p>`
  });
  return renderPage(m.note.title, article);
}
