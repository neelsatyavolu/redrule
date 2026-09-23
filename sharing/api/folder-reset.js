import { validID, newSecret, hash, bearer, matches, readFolder, createFolder, copyContents, retireFolder } from '../lib/folders.js';
import { limited, tooMany } from '../lib/limit.js';

// Moves the folder to a new ID and member key, so the old link stops working. The owner key stays the same.
// The new folder.json is written only after every meeting is copied. Once it exists the reset has succeeded and the
// app must learn the new ID, so retiring the old folder never turns the answer into an error.
export default async function handler(req, res) {
  res.setHeader('Cache-Control', 'no-store');
  if (req.method !== 'POST') { res.setHeader('Allow', 'POST'); return res.status(405).end(); }
  // A reset copies every blob in the folder, so it counts as creating one.
  const wait = limited(req, 'create');
  if (wait) return tooMany(res, wait);
  const id = req.query.id;
  if (!validID(id)) return res.status(400).json({error:'Invalid folder ID.'});
  try {
    const folder = await readFolder(id);
    if (!folder) return res.status(404).json({error:'This folder no longer exists. Its owner may have reset its link or deleted it.'});
    if (!matches(bearer(req.headers.authorization), folder.ownerKeyHash)) return res.status(403).json({error:'Only the folder’s owner can do this.'});
    const newID = newSecret(), memberKey = newSecret();
    await copyContents(id, newID);
    await createFolder(newID, {...folder, memberKeyHash:hash(memberKey)});
    const oldLinkClosed = await retireFolder(id);
    return res.status(200).json(oldLinkClosed ? {id:newID, memberKey} : {id:newID, memberKey, oldLinkStillActive:true});
  } catch { return res.status(503).json({error:'Shared folders are temporarily unavailable. Please try again.'}); }
}
