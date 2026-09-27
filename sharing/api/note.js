import { renderNote } from '../lib/notes.js';
import { validID, read } from '../lib/storage.js';
import { MARKDOWN, markdownPath, wantsMarkdown, noteMarkdown, sendError } from '../lib/markdown.js';

export default async function handler(req,res) {
  res.setHeader('Cache-Control','private, no-store, max-age=0');
  res.setHeader('X-Robots-Tag','noindex, nofollow, noarchive');
  res.setHeader('Referrer-Policy','no-referrer');
  res.setHeader('X-Content-Type-Options','nosniff');
  res.setHeader('Vary','Accept');
  res.setHeader('Content-Security-Policy',"default-src 'none'; style-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'");
  if (req.method!=='GET' && req.method!=='HEAD') return res.status(405).end();
  const {value:id, markdown}=markdownPath(req.query.id);
  try {
    const data=validID(id) ? await read(id) : null;
    if (!data) return sendError(req,res,404,'This shared note is unavailable. Its owner may have stopped sharing it.',markdown);
    if (wantsMarkdown(req,markdown)) return res.status(200).setHeader('Content-Type',MARKDOWN).send(noteMarkdown(data));
    return res.status(200).setHeader('Content-Type','text/html; charset=utf-8').send(renderNote(data,`/s/${id}.md`));
  } catch { return sendError(req,res,503,'This note could not be loaded. Please try again shortly.',markdown); }
}
