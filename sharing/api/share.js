import { authorized, validateNote } from '../lib/notes.js';
import { validID, write, remove } from '../lib/storage.js';

export default async function handler(req,res) {
  res.setHeader('Cache-Control','no-store');
  if (!authorized(req.headers.authorization,process.env.MINUTES_SHARE_KEY)) return res.status(401).json({error:'Sharing is not authorized on this Mac.'});
  const id=req.query.id;
  if (!validID(id)) return res.status(400).json({error:'Invalid share ID.'});
  try {
    if (req.method==='DELETE') { await remove(id); return res.status(204).end(); }
    if (req.method!=='PUT') { res.setHeader('Allow','PUT, DELETE'); return res.status(405).end(); }
    if (Buffer.byteLength(JSON.stringify(req.body ?? null)) > 2000000) return res.status(413).json({error:'This meeting is too large to share.'});
    let data;
    try { data=validateNote(req.body); } catch { return res.status(400).json({error:'Invalid meeting content.'}); }
    await write(id,data);
    return res.status(200).json({path:`/s/${id}`});
  } catch { return res.status(503).json({error:'Sharing is temporarily unavailable. Please try again.'}); }
}
