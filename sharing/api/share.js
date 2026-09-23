import { authorized, validateNote } from '../lib/notes.js';
import { validID, write, remove } from '../lib/storage.js';
import { hash, bearer } from '../lib/folders.js';
import { limited, tooMany } from '../lib/limit.js';

const NOT_OWNER = 'Only the Mac that shared this link can change or remove it.';
// A link's id is the SHA-256 of its owner key, so only the key's holder can create, change or remove it, and the service
// keeps no key. Links from before owner keys have random ids and answer only to the legacy MINUTES_SHARE_KEY.
// Returns null without a usable credential, else whether it owns the link.
function owns(header, id) {
  if (authorized(header, process.env.MINUTES_SHARE_KEY)) return true;
  const key = bearer(header);
  return key === null ? null : hash(key) === id;
}

export default async function handler(req,res) {
  res.setHeader('Cache-Control','no-store');
  if (req.method!=='PUT' && req.method!=='DELETE') { res.setHeader('Allow','PUT, DELETE'); return res.status(405).end(); }
  const wait=limited(req,'share');
  if (wait) return tooMany(res,wait);
  const id=req.query.id;
  if (!validID(id)) return res.status(400).json({error:'Invalid share ID.'});
  const owner=owns(req.headers.authorization,id);
  if (owner!==true) return res.status(owner===null ? 401 : 403).json({error:NOT_OWNER});
  try {
    if (req.method==='DELETE') { await remove(id); return res.status(204).end(); }
    if (Buffer.byteLength(JSON.stringify(req.body ?? null)) > 2000000) return res.status(413).json({error:'This meeting is too large to share.'});
    let data;
    try { data=validateNote(req.body); } catch { return res.status(400).json({error:'Invalid meeting content.'}); }
    await write(id,data);
    return res.status(200).json({path:`/s/${id}`});
  } catch { return res.status(503).json({error:'Sharing is temporarily unavailable. Please try again.'}); }
}
