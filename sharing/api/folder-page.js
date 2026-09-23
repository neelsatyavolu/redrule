import { renderFolder, renderFolderMeeting } from '../lib/folder-render.js';
import { validID, validMeetingID, readFolder, readMeeting, meetingSummaries } from '../lib/folders.js';

const html = (res, body) => res.status(200).setHeader('Content-Type', 'text/html; charset=utf-8').send(body);

export default async function handler(req, res) {
  res.setHeader('Cache-Control','private, no-store, max-age=0');
  res.setHeader('X-Robots-Tag','noindex, nofollow, noarchive');
  res.setHeader('Referrer-Policy','no-referrer');
  res.setHeader('X-Content-Type-Options','nosniff');
  res.setHeader('Content-Security-Policy',"default-src 'none'; style-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'");
  if (req.method!=='GET' && req.method!=='HEAD') return res.status(405).end();
  const {id, meeting} = req.query;
  try {
    const folder = validID(id) ? await readFolder(id) : null;
    if (!folder) return res.status(404).send('This shared folder is unavailable. Its owner may have reset its link or deleted it.');
    if (meeting === undefined) return html(res, renderFolder(id, folder, await meetingSummaries(id)));
    const found = validMeetingID(meeting) ? await readMeeting(id, meeting) : null;
    if (!found) return res.status(404).send('This meeting is no longer in the shared folder.');
    return html(res, renderFolderMeeting(id, folder, found.meeting));
  } catch { return res.status(503).send('This folder could not be loaded. Please try again shortly.'); }
}
