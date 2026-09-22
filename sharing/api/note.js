import { renderNote } from '../lib/notes.js';
import { validID, read } from '../lib/storage.js';

export default async function handler(req,res) {
  res.setHeader('Cache-Control','private, no-store, max-age=0');
  res.setHeader('X-Robots-Tag','noindex, nofollow, noarchive');
  res.setHeader('Referrer-Policy','no-referrer');
  res.setHeader('X-Content-Type-Options','nosniff');
  res.setHeader('Content-Security-Policy',"default-src 'none'; style-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'");
  if (req.method!=='GET' && req.method!=='HEAD') return res.status(405).end();
  try {
    const data=validID(req.query.id) ? await read(req.query.id) : null;
    if (!data) return res.status(404).send('This shared note is unavailable. Its owner may have stopped sharing it.');
    return res.status(200).setHeader('Content-Type','text/html; charset=utf-8').send(renderNote(data));
  } catch { return res.status(503).send('This note could not be loaded. Please try again shortly.'); }
}
