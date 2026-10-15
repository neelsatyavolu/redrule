import { folderMeetingMarkdown } from '../lib/folder-render.js';
import { validID, validMeetingID, readFolder, readMeeting, meetingSummaries } from '../lib/folders.js';
import { limited, tooMany } from '../lib/limit.js';
import { origin } from '../lib/markdown.js';
import { handleMessage } from '../lib/mcp.js';

const rpcError = (res, status, code, message) => res.status(status).json({jsonrpc:'2.0', id:null, error:{code, message}});

// /f/<id>/mcp: the folder as a read-only MCP server. Like the folder page, the id alone grants read access.
export default async function handler(req, res) {
  res.setHeader('Cache-Control','private, no-store, max-age=0');
  res.setHeader('X-Robots-Tag','noindex, nofollow, noarchive');
  res.setHeader('Referrer-Policy','no-referrer');
  res.setHeader('X-Content-Type-Options','nosniff');
  // No server-sent stream: every reply comes back on its POST.
  if (req.method !== 'POST') return res.status(405).setHeader('Allow', 'POST').end();
  const id = req.query.id;
  const message = typeof req.body === 'string' ? safeParse(req.body) : req.body;
  if (message === undefined) return rpcError(res, 400, -32700, 'Parse error');
  // Listing reads every summary, like the folder page, so it shares that tighter limit.
  const wait = limited(req, message?.params?.name === 'list_meetings' ? 'page' : 'read');
  if (wait) return tooMany(res, wait);
  try {
    const folder = validID(id) ? await readFolder(id) : null;
    if (!folder) return rpcError(res, 404, -32001, 'This shared folder is unavailable. Its owner may have reset its link or deleted it.');
    const reply = await handleMessage(message, {
      name: folder.name,
      listMeetings: () => meetingSummaries(id),
      meetingMarkdown: async meeting => {
        const found = validMeetingID(meeting) ? await readMeeting(id, meeting) : null;
        return found && folderMeetingMarkdown(id, folder, found.meeting, origin(req));
      },
    });
    return reply ? res.status(200).json(reply) : res.status(202).end();
  } catch { return rpcError(res, 503, -32002, 'This folder could not be loaded. Please try again shortly.'); }
}

function safeParse(body) {
  try { return JSON.parse(body); } catch { return undefined; }
}
