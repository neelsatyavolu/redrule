import { escape, renderPage, renderArticle } from './notes.js';
import { oneLine, linkText, noteMarkdown } from './markdown.js';

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
const newestFirst = meetings => [...meetings].sort((a, b) => Date.parse(b.startedAt) - Date.parse(a.startedAt));
const meetingCount = n => `${n} ${n === 1 ? 'meeting' : 'meetings'}`;
const transcriptOf = m => m.transcript.map(s => ({speaker:speakerLabel(s, m.recordedBy), time:formatTime(s.start), text:s.text}));

// meetings: [{id, title, startedAt, recordedBy}] as returned by meetingSummaries.
export function renderFolder(id, folder, meetings) {
  const items = newestFirst(meetings).map(m => `<li><a href="/f/${id}/m/${m.id}">${escape(m.title || 'Untitled meeting')}</a><span class="owner">${escape(formatDate(m.startedAt))} · Recorded by ${escape(m.recordedBy)}</span></li>`).join('');
  return renderPage(folder.name, `<article><p class="eyebrow">SHARED FOLDER</p><h1>${escape(folder.name)}</h1><p class="summary">${meetingCount(meetings.length)}</p>${meetings.length ? `<ul>${items}</ul>` : ''}</article>`, `/f/${id}.md`);
}

export function renderFolderMeeting(id, folder, m) {
  const transcript = transcriptOf(m);
  const article = renderArticle({note:m.note, transcript:transcript.length ? transcript : undefined}, {
    eyebrow:`<a href="/f/${id}">← ${escape(folder.name)}</a>`,
    byline:`<p class="owner">Recorded by ${escape(m.recordedBy)} · ${escape(formatDate(m.startedAt))}</p>`
  });
  return renderPage(m.note.title, article, `/f/${id}/m/${m.id}.md`);
}

// origin makes the links absolute, since an agent reading Markdown has no page URL to resolve them against.
export function folderMarkdown(id, folder, meetings, origin) {
  const items = newestFirst(meetings).map(m => `- [${linkText(m.title || 'Untitled meeting')}](${origin}/f/${id}/m/${m.id}.md) · ${formatDate(m.startedAt)} · Recorded by ${oneLine(m.recordedBy)}`);
  return [`# ${oneLine(folder.name)}`, `Shared folder with ${meetingCount(meetings.length)}.${meetings.length ? ' Each link below has one meeting’s notes and transcript.' : ''}`, items.join('\n')].filter(Boolean).join('\n\n') + '\n';
}

export function folderMeetingMarkdown(id, folder, m, origin) {
  const transcript = transcriptOf(m);
  return noteMarkdown({note:m.note, transcript:transcript.length ? transcript : undefined},
    `Recorded by ${oneLine(m.recordedBy)} · ${formatDate(m.startedAt)} · From the shared folder [${linkText(folder.name)}](${origin}/f/${id}.md)`);
}
