import { validID, newSecret, hash, bearer, matches, tooLarge, validateName, readFolder, createFolder, updateFolder, meetingList, removeFolder } from '../lib/folders.js';
import { limited, tooMany } from '../lib/limit.js';

const GONE = 'This folder no longer exists. Its owner may have reset its link or deleted it.';
const NOT_OWNER = 'Only the folder’s owner can do this.';
function name(req, res) {
  if (tooLarge(req.body)) { res.status(413).json({error:'This request is too large.'}); return null; }
  try { return validateName(req.body?.name); } catch { res.status(400).json({error:'Folder names need 1 to 80 characters.'}); return null; }
}

// Anyone can create a folder; the keys it answers with are the only way to use it. Old apps still send the legacy
// MINUTES_SHARE_KEY, which is ignored.
async function create(req, res) {
  const folderName = name(req, res);
  if (folderName === null) return;
  const id = newSecret(), memberKey = newSecret(), ownerKey = newSecret();
  await createFolder(id, {name:folderName, memberKeyHash:hash(memberKey), ownerKeyHash:hash(ownerKey), createdAt:new Date().toISOString()});
  return res.status(201).json({id, memberKey, ownerKey});
}

export default async function handler(req, res) {
  res.setHeader('Cache-Control', 'no-store');
  if (!['GET','POST','PATCH','DELETE'].includes(req.method)) { res.setHeader('Allow', 'GET, POST, PATCH, DELETE'); return res.status(405).end(); }
  const wait = limited(req, {GET:'read', POST:'create'}[req.method] ?? 'write');
  if (wait) return tooMany(res, wait);
  try {
    if (req.method === 'POST') return await create(req, res);
    const id = req.query.id;
    if (!validID(id)) return res.status(400).json({error:'Invalid folder ID.'});
    const folder = await readFolder(id);
    if (!folder) return res.status(404).json({error:GONE});
    if (req.method === 'GET') {
      const member = req.headers.authorization === undefined ? {} : {member:matches(bearer(req.headers.authorization), folder.memberKeyHash)};
      return res.status(200).json({name:folder.name, meetings:await meetingList(id), ...member});
    }
    if (!matches(bearer(req.headers.authorization), folder.ownerKeyHash)) return res.status(403).json({error:NOT_OWNER});
    if (req.method === 'DELETE') { await removeFolder(id); return res.status(204).end(); }
    const folderName = name(req, res);
    if (folderName === null) return;
    const result = await updateFolder(id, {...folder, name:folderName});
    if (result === 'gone') return res.status(404).json({error:GONE});
    if (result === 'changed') return res.status(409).json({error:'The folder changed while it was being renamed. Please try again.'});
    return res.status(200).json({name:folderName});
  } catch { return res.status(503).json({error:'Shared folders are temporarily unavailable. Please try again.'}); }
}
