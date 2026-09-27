import { renderFolder, renderFolderMeeting, folderMarkdown, folderMeetingMarkdown } from '../lib/folder-render.js';
import { validID, validMeetingID, readFolder, readMeeting, meetingSummaries } from '../lib/folders.js';
import { limited, tooMany } from '../lib/limit.js';
import { MARKDOWN, markdownPath, wantsMarkdown, origin, sendError } from '../lib/markdown.js';

const html = (res, body) => res.status(200).setHeader('Content-Type', 'text/html; charset=utf-8').send(body);
const markdown = (res, body) => res.status(200).setHeader('Content-Type', MARKDOWN).send(body);

export default async function handler(req, res) {
  res.setHeader('Cache-Control','private, no-store, max-age=0');
  res.setHeader('X-Robots-Tag','noindex, nofollow, noarchive');
  res.setHeader('Referrer-Policy','no-referrer');
  res.setHeader('X-Content-Type-Options','nosniff');
  res.setHeader('Vary','Accept');
  res.setHeader('Content-Security-Policy',"default-src 'none'; style-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'");
  if (req.method!=='GET' && req.method!=='HEAD') return res.status(405).end();
  // Either the folder's id or the meeting's id may end in .md; see lib/markdown.js.
  const folderRef = markdownPath(req.query.id), meetingRef = req.query.meeting === undefined ? undefined : markdownPath(req.query.meeting);
  const id = folderRef.value, meeting = meetingRef?.value, asMarkdown = wantsMarkdown(req, (meetingRef ?? folderRef).markdown);
  // The folder view lists every meeting and reads each summary, so it has a tighter limit than one meeting's page.
  const wait = limited(req, meeting === undefined ? 'page' : 'read');
  if (wait) return tooMany(res, wait, false);
  try {
    const folder = validID(id) ? await readFolder(id) : null;
    if (!folder) return sendError(req, res, 404, 'This shared folder is unavailable. Its owner may have reset its link or deleted it.', asMarkdown);
    if (meeting === undefined) {
      const meetings = await meetingSummaries(id);
      return asMarkdown ? markdown(res, folderMarkdown(id, folder, meetings, origin(req))) : html(res, renderFolder(id, folder, meetings));
    }
    const found = validMeetingID(meeting) ? await readMeeting(id, meeting) : null;
    if (!found) return sendError(req, res, 404, 'This meeting is no longer in the shared folder.', asMarkdown);
    return asMarkdown ? markdown(res, folderMeetingMarkdown(id, folder, found.meeting, origin(req))) : html(res, renderFolderMeeting(id, folder, found.meeting));
  } catch { return sendError(req, res, 503, 'This folder could not be loaded. Please try again shortly.', asMarkdown); }
}
