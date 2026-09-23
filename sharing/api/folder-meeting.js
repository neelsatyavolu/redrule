import { MAX_MEETINGS, MAX_BYTES, validID, validMeetingID, hash, bearer, matches, tooLarge, validateMeeting, readFolder, readMeeting, writeMeeting, removeMeeting, meetingBlobs, meetingList } from '../lib/folders.js';
import { limited, tooMany } from '../lib/limit.js';

const NOT_MEMBER = 'This link does not let you add meetings to the folder.';
const NOT_YOURS = 'Only the Mac that shared this meeting, or the folder’s owner, can change it.';
const FULL = `This folder is full. It can hold ${MAX_MEETINGS} meetings.`;

async function save(req, res, id, meeting, folder) {
  if (!matches(bearer(req.headers.authorization), folder.memberKeyHash)) return res.status(401).json({error:NOT_MEMBER});
  const editKey = req.headers['x-edit-key'];
  if (!validID(editKey)) return res.status(400).json({error:'Missing or invalid edit key.'});
  if (tooLarge(req.body)) return res.status(413).json({error:'This meeting is too large to share.'});
  let data;
  try { data = validateMeeting(req.body); } catch { return res.status(400).json({error:'Invalid meeting content.'}); }
  const existing = await readMeeting(id, meeting);
  if (existing && !matches(editKey, existing.editKeyHash)) return res.status(403).json({error:NOT_YOURS});
  const others = (await meetingBlobs(id)).filter(m => m.id !== meeting), stored = {...data, editKeyHash:hash(editKey)};
  if (!existing && others.length >= MAX_MEETINGS) return res.status(409).json({error:FULL});
  if (others.reduce((sum, m) => sum + m.size, 0) + Buffer.byteLength(JSON.stringify(stored)) > MAX_BYTES) return res.status(413).json({error:`This folder is full. It can hold ${MAX_BYTES / 1e6} MB of meetings.`});
  const updatedAt = await writeMeeting(id, meeting, stored, Boolean(existing));
  // Uploads running side by side can all pass the count above. A new meeting that then finds the folder over the limit
  // takes itself back out, so the folder never stays above it.
  if (!existing && (await meetingList(id)).length > MAX_MEETINGS) { await removeMeeting(id, meeting); return res.status(409).json({error:FULL}); }
  return res.status(200).json({updatedAt});
}

async function remove(req, res, id, meeting, folder) {
  const key = bearer(req.headers.authorization);
  if (matches(key, folder.ownerKeyHash)) { await removeMeeting(id, meeting); return res.status(204).end(); }
  if (!matches(key, folder.memberKeyHash)) return res.status(401).json({error:NOT_MEMBER});
  const existing = await readMeeting(id, meeting);
  if (!existing) return res.status(204).end();
  if (!matches(req.headers['x-edit-key'], existing.editKeyHash)) return res.status(403).json({error:NOT_YOURS});
  await removeMeeting(id, meeting);
  return res.status(204).end();
}

export default async function handler(req, res) {
  res.setHeader('Cache-Control', 'no-store');
  if (!['GET','PUT','DELETE'].includes(req.method)) { res.setHeader('Allow', 'GET, PUT, DELETE'); return res.status(405).end(); }
  const {id, meeting} = req.query;
  if (!validID(id)) return res.status(400).json({error:'Invalid folder ID.'});
  if (!validMeetingID(meeting)) return res.status(400).json({error:'Invalid meeting ID.'});
  const wait = limited(req, req.method === 'GET' ? 'read' : 'write');
  if (wait) return tooMany(res, wait);
  try {
    const folder = await readFolder(id);
    if (!folder) return res.status(404).json({error:'This folder no longer exists. Its owner may have reset its link or deleted it.'});
    if (req.method === 'PUT') return await save(req, res, id, meeting, folder);
    if (req.method === 'DELETE') return await remove(req, res, id, meeting, folder);
    const found = await readMeeting(id, meeting);
    return found ? res.status(200).json(found.meeting) : res.status(404).json({error:'This meeting is no longer in the folder.'});
  } catch { return res.status(503).json({error:'Shared folders are temporarily unavailable. Please try again.'}); }
}
