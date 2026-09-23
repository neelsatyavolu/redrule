import { createHash, randomBytes, timingSafeEqual } from 'node:crypto';
import { get, put, del, list, copy, head, BlobNotFoundError, BlobPreconditionFailedError } from '@vercel/blob';
import { validateNote } from './notes.js';
import { validID } from './storage.js';

export const MAX_MEETINGS = 500, MAX_BODY = 2000000;
export { validID };
export const validMeetingID = id => typeof id === 'string' && /^[0-9A-F]{8}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{12}$/.test(id);
export const newSecret = () => randomBytes(32).toString('hex');
export const hash = key => createHash('sha256').update(key).digest('hex');
export const tooLarge = body => Buffer.byteLength(JSON.stringify(body ?? null)) > MAX_BODY;
export const bearer = header => typeof header === 'string' && header.startsWith('Bearer ') && validID(header.slice(7)) ? header.slice(7) : null;
export function matches(key, keyHash) {
  if (!validID(key) || !validID(keyHash)) return false;
  return timingSafeEqual(Buffer.from(hash(key), 'hex'), Buffer.from(keyHash, 'hex'));
}

// Validation. Every function throws on bad input and returns a fresh object holding only known fields.
const length = s => [...s].length;
const text = (v, min, max) => { if (typeof v !== 'string' || length(v.trim()) < min || length(v) > max) throw new Error('Invalid text'); return v; };
// RFC 3339 dates that exist, as chrono's parse_from_rfc3339 in the app reads them: no Feb 30, hour 24 or leap second.
const ISO = /^(\d{4})-(\d\d)-(\d\d)T(\d\d):(\d\d):(\d\d)(\.\d{1,9})?(Z|[+-](\d\d):(\d\d))$/;
const monthDays = (y, m) => [31, y % 4 === 0 && (y % 100 !== 0 || y % 400 === 0) ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][m - 1];
const iso = v => {
  const p = typeof v === 'string' && ISO.exec(v);
  const [y, mo, d, h, mi, sec, oh, om] = p ? [1, 2, 3, 4, 5, 6, 9, 10].map(i => Number(p[i] ?? 0)) : [];
  if (!p || mo < 1 || mo > 12 || d < 1 || d > monthDays(y, mo) || h > 23 || mi > 59 || sec > 59 || oh > 23 || om > 59) throw new Error('Invalid date');
  return v;
};
const finite = v => { if (typeof v !== 'number' || !Number.isFinite(v)) throw new Error('Invalid number'); return v; };
const optional = (v, fn) => v === undefined || v === null ? {} : fn(v);
export const validateName = name => text(name, 1, 80).trim();
function segment(s) {
  if (s?.speaker !== 'me' && s?.speaker !== 'them') throw new Error('Invalid speaker');
  return {speaker:s.speaker, start:finite(s.start), end:finite(s.end), text:text(s.text, 0, 100000),
    ...optional(s.speakerID, v => ({speakerID:text(v, 0, 200)})), ...optional(s.speakerName, v => ({speakerName:text(v, 0, 200)}))};
}
export function validateMeeting(m) {
  if (!['zoom','googleMeet','manual'].includes(m?.app)) throw new Error('Invalid app');
  const {note} = validateNote({note:m.note});
  const actionItems = note.actionItems.map((a, i) => {
    const done = m.note.actionItems[i].done;
    if (done !== undefined && typeof done !== 'boolean') throw new Error('Invalid done');
    return done === undefined ? a : {...a, done};
  });
  if (!Array.isArray(m.transcript) || m.transcript.length > 20000) throw new Error('Invalid transcript');
  return {title:text(m.title, 0, 300), app:m.app, startedAt:iso(m.startedAt), endedAt:m.endedAt === null ? null : iso(m.endedAt),
    recordedBy:text(m.recordedBy, 1, 60), note:{...note, actionItems}, transcript:m.transcript.map(segment)};
}

// Storage in the private Blob store: folders/<id>/folder.json, folders/<id>/meetings/<meetingId>.json and, for the
// folder page, folders/<id>/summaries/<meetingId>.json holding {title, startedAt, recordedBy}.
const folderPrefix = id => `folders/${id}/`;
const folderPath = id => `${folderPrefix(id)}folder.json`;
const meetingsPrefix = id => `${folderPrefix(id)}meetings/`;
const meetingPath = (id, meeting) => `${meetingsPrefix(id)}${meeting}.json`;
const summariesPrefix = id => `${folderPrefix(id)}summaries/`;
const summaryPath = (id, meeting) => `${summariesPrefix(id)}${meeting}.json`;
const summaryOf = m => ({title:m.title, startedAt:m.startedAt, recordedBy:m.recordedBy});
const options = overwrite => ({access:'private', addRandomSuffix:false, allowOverwrite:overwrite, contentType:'application/json'});
async function readJSON(pathname) {
  const result = await get(pathname, {access:'private', useCache:false});
  if (!result) return null;
  if (result.statusCode !== 200) throw new Error('Storage unavailable');
  return JSON.parse(await new Response(result.stream).text());
}
async function listAll(prefix) {
  const blobs = [];
  let cursor;
  do { const page = await list({prefix, cursor}); blobs.push(...page.blobs); cursor = page.hasMore ? page.cursor : undefined; } while (cursor);
  return blobs;
}
async function inBatches(items, fn, size = 25) {
  const out = [];
  for (let i = 0; i < items.length; i += size) out.push(...await Promise.all(items.slice(i, i + size).map(fn)));
  return out;
}
async function delAll(urls) { for (let i = 0; i < urls.length; i += 500) await del(urls.slice(i, i + 500)); }
export const readFolder = id => readJSON(folderPath(id));
export const createFolder = (id, folder) => put(folderPath(id), JSON.stringify(folder), options(false));
// Writes only over the folder.json that is there now (by ETag), so a rename racing a reset or delete fails instead of
// bringing the old folder back. Returns 'gone' or 'changed' when it did not write.
export async function updateFolder(id, folder) {
  let etag;
  try { ({etag} = await head(folderPath(id))); } catch (e) { if (e instanceof BlobNotFoundError) return 'gone'; throw e; }
  try { await put(folderPath(id), JSON.stringify(folder), {...options(true), ifMatch:etag}); return 'ok'; }
  catch (e) { if (e instanceof BlobPreconditionFailedError) return await readFolder(id) ? 'changed' : 'gone'; throw e; }
}
// A meeting's public updatedAt is its blob's upload time as reported by head and list (get only has whole seconds),
// so one list call tells the app exactly which meetings changed. Head runs before get: a write in between makes the
// time older than the content, which the next sync corrects, rather than hiding the change.
export async function readMeeting(id, meeting) {
  const pathname = meetingPath(id, meeting);
  let info;
  try { info = await head(pathname); } catch (e) { if (e instanceof BlobNotFoundError) return null; throw e; }
  const data = await readJSON(pathname);
  if (!data) return null;
  const {editKeyHash, ...stored} = data;
  return {meeting:{...stored, id:meeting, updatedAt:info.uploadedAt.toISOString()}, editKeyHash};
}
export async function writeMeeting(id, meeting, data, overwrite) {
  const pathname = meetingPath(id, meeting);
  await put(pathname, JSON.stringify({...data, updatedAt:new Date().toISOString()}), options(overwrite));
  const [, info] = await Promise.all([put(summaryPath(id, meeting), JSON.stringify(summaryOf(data)), options(true)), head(pathname)]);
  return info.uploadedAt.toISOString();
}
export const removeMeeting = (id, meeting) => del([meetingPath(id, meeting), summaryPath(id, meeting)]);
export async function meetingList(id) {
  const prefix = meetingsPrefix(id);
  return (await listAll(prefix)).map(b => ({id:b.pathname.slice(prefix.length, -5), updatedAt:new Date(b.uploadedAt).toISOString()})).filter(m => validMeetingID(m.id));
}
// Copies meetings and summaries, everything but folder.json, for a reset.
export async function copyContents(from, to) {
  const prefix = folderPrefix(from);
  const blobs = (await listAll(prefix)).filter(b => b.pathname !== folderPath(from));
  await inBatches(blobs, b => copy(b.url, `${folderPrefix(to)}${b.pathname.slice(prefix.length)}`, options(true)));
}
// What the folder page lists, read from the small summaries. A meeting without one falls back to the full meeting,
// and a summary whose meeting is gone is skipped.
export async function meetingSummaries(id) {
  const prefix = summariesPrefix(id);
  const [meetings, summaries] = await Promise.all([meetingList(id), listAll(prefix)]);
  const summarized = new Set(summaries.map(b => b.pathname.slice(prefix.length, -5)));
  const read = async m => { const d = await readJSON(summarized.has(m.id) ? summaryPath(id, m.id) : meetingPath(id, m.id)); return d && {id:m.id, ...summaryOf(d)}; };
  return (await inBatches(meetings, read)).filter(Boolean);
}
// Deleting: contents first and folder.json last, so a failed delete can be retried with the owner key.
export async function removeFolder(id) {
  await delAll((await listAll(folderPrefix(id))).filter(b => b.pathname !== folderPath(id)).map(b => b.url));
  await del(folderPath(id));
}
// After a reset: folder.json first, so the old link dies at once, then the contents on a best-effort basis. Never
// throws. Returns whether the old link is closed; while its folder.json exists, the old link still works.
const RETIRE_ATTEMPTS = 3;
export async function retireFolder(id) {
  let closed = false;
  for (let attempt = 0; attempt < RETIRE_ATTEMPTS && !closed; attempt++) {
    try { await del(folderPath(id)); closed = true; } catch { /* retried; the new folder already exists */ }
  }
  try { await delAll((await listAll(folderPrefix(id))).map(b => b.url)); } catch { /* leftover blobs are harmless */ }
  return closed;
}
