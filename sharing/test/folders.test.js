import { test, mock } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';

// In-memory private Blob store. list pages hold two blobs, so every listing exercises the cursor.
// hooks lets a test watch reads, run code after a read (to simulate a concurrent request) or make deletes fail.
const store = new Map(), hooks = {reads:[], afterGet:null, afterHead:null, failDel:null};
let clock = Date.parse('2026-09-22T10:00:00.000Z'), version = 0;
class BlobNotFoundError extends Error {}
class BlobPreconditionFailedError extends Error {}
const url = p => `https://store.example/${p}`, pathOf = u => u.replace('https://store.example/', '');
const save = (p, body, o) => {
  if (store.has(p) && !o.allowOverwrite) throw new Error('Blob exists');
  if (o.ifMatch !== undefined && store.get(p)?.etag !== o.ifMatch) throw new BlobPreconditionFailedError();
  store.set(p, {body, uploadedAt:new Date(clock += 1000), etag:`"v${++version}"`});
};
mock.module('@vercel/blob', {namedExports:{
  BlobNotFoundError, BlobPreconditionFailedError,
  get: async p => {
    hooks.reads.push(p);
    const result = store.has(p) ? {statusCode:200, stream:new Blob([store.get(p).body]).stream(), blob:{pathname:p, uploadedAt:new Date(Math.floor(store.get(p).uploadedAt / 1000) * 1000)}} : null;
    hooks.afterGet?.(p);
    return result;
  },
  head: async p => { if (!store.has(pathOf(p))) throw new BlobNotFoundError(); const b = store.get(pathOf(p)); hooks.afterHead?.(pathOf(p)); return {pathname:pathOf(p), uploadedAt:b.uploadedAt, etag:b.etag}; },
  put: async (p, body, o) => { save(p, body, o); return {pathname:p, url:url(p)}; },
  copy: async (from, to, o) => { save(to, store.get(pathOf(from)).body, o); return {pathname:to, url:url(to)}; },
  del: async p => { for (const x of [p].flat()) { if (hooks.failDel?.(pathOf(x))) throw new Error('Storage unavailable'); store.delete(pathOf(x)); } },
  list: async ({prefix, cursor}) => {
    const all = [...store.keys()].filter(k => k.startsWith(prefix)).sort(), start = Number(cursor ?? 0);
    const blobs = all.slice(start, start + 2).map(k => ({pathname:k, url:url(k), uploadedAt:store.get(k).uploadedAt}));
    return {blobs, hasMore:start + 2 < all.length, cursor:start + 2 < all.length ? String(start + 2) : undefined};
  }
}});
const {default:folderAPI} = await import('../api/folder.js');
const {default:meetingAPI} = await import('../api/folder-meeting.js');
const {default:resetAPI} = await import('../api/folder-reset.js');
const {default:pageAPI} = await import('../api/folder-page.js');
const {validateMeeting} = await import('../lib/folders.js');
const {formatDate, formatTime} = await import('../lib/folder-render.js');

process.env.MINUTES_SHARE_KEY = 'test-only-credential';
const SHARE = 'Bearer test-only-credential';
const sha = s => createHash('sha256').update(s).digest('hex');
const editA = sha('device-a:1'), editB = sha('device-b:1');
const M1 = '11111111-2222-3333-4444-555555555555', M2 = 'AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE';
const meeting = (over = {}) => ({title:'Planning', app:'zoom', startedAt:'2026-09-22T09:00:00Z', endedAt:'2026-09-22T09:30:00.123456789Z', recordedBy:'Dana',
  note:{title:'Planning notes', tldr:'We planned.', sections:[{heading:'Scope', bullets:['Ship folders']}], decisions:['Go'], actionItems:[{owner:'Sam', task:'Write spec', done:true}, {task:'Review'}]},
  transcript:[{speaker:'me', start:0, end:2, text:'Hello'}, {speaker:'them', start:75.4, end:80, text:'Hi', speakerName:'Lee'}], ...over});
function response() { return {code:200, headers:{}, status(c) { this.code = c; return this; }, setHeader(k, v) { this.headers[k] = v; return this; }, json(b) { this.body = b; return this; }, send(b) { this.body = b; return this; }, end() { return this; }}; }
async function call(handler, method, query, {auth, edit, body} = {}) {
  const res = response();
  await handler({method, query, body, headers:{...(auth === undefined ? {} : {authorization:auth}), ...(edit === undefined ? {} : {'x-edit-key':edit})}}, res);
  return res;
}
async function newFolder(name = 'Team') { return (await call(folderAPI, 'POST', {}, {auth:SHARE, body:{name}})).body; }
const put = (f, mid, edit, body = meeting(), auth = `Bearer ${f.memberKey}`) => call(meetingAPI, 'PUT', {id:f.id, meeting:mid}, {auth, edit, body});

test('creating a folder needs the share key and stores only key hashes', async () => {
  assert.equal((await call(folderAPI, 'POST', {}, {body:{name:'Team'}})).code, 401);
  assert.equal((await call(folderAPI, 'POST', {}, {auth:'Bearer wrong', body:{name:'Team'}})).code, 401);
  const res = await call(folderAPI, 'POST', {}, {auth:SHARE, body:{name:'  Team  '}});
  assert.equal(res.code, 201);
  for (const k of ['id', 'memberKey', 'ownerKey']) assert.match(res.body[k], /^[a-f0-9]{64}$/);
  const stored = JSON.parse(store.get(`folders/${res.body.id}/folder.json`).body);
  assert.deepEqual(Object.keys(stored).sort(), ['createdAt', 'memberKeyHash', 'name', 'ownerKeyHash']);
  assert.equal(stored.name, 'Team');
  assert.equal(stored.memberKeyHash, sha(res.body.memberKey));
  assert.ok(![...store.values()].some(v => v.body.includes(res.body.memberKey) || v.body.includes(res.body.ownerKey)));
});

test('reading a folder with and without the member key', async () => {
  const f = await newFolder();
  let res = await call(folderAPI, 'GET', {id:f.id});
  assert.deepEqual(res.body, {name:'Team', meetings:[]});
  assert.equal(res.headers['Cache-Control'], 'no-store');
  assert.equal((await call(folderAPI, 'GET', {id:f.id}, {auth:`Bearer ${f.memberKey}`})).body.member, true);
  assert.equal((await call(folderAPI, 'GET', {id:f.id}, {auth:`Bearer ${f.ownerKey}`})).body.member, false);
  assert.equal((await call(folderAPI, 'GET', {id:f.id}, {auth:'Bearer nonsense'})).body.member, false);
  assert.equal((await call(folderAPI, 'GET', {id:'b'.repeat(64)})).code, 404);
  assert.equal((await call(folderAPI, 'GET', {id:'../x'})).code, 400);
});

test('members add meetings; only the uploading Mac or the owner can change them', async () => {
  const f = await newFolder();
  let res = await put(f, M1, editA);
  assert.equal(res.code, 200);
  const first = res.body.updatedAt;
  let folder = (await call(folderAPI, 'GET', {id:f.id})).body;
  assert.deepEqual(folder.meetings, [{id:M1, updatedAt:first}]);
  res = await call(meetingAPI, 'GET', {id:f.id, meeting:M1});
  assert.equal(res.code, 200);
  assert.equal(res.body.id, M1);
  assert.equal(res.body.updatedAt, first);
  assert.equal(res.body.editKeyHash, undefined);
  assert.deepEqual(res.body.note.actionItems, [{owner:'Sam', task:'Write spec', done:true}, {owner:null, task:'Review'}]);
  assert.equal(res.body.transcript[1].speakerName, 'Lee');

  assert.equal((await put(f, M1, editB, meeting({title:'Hijacked'}))).code, 403);
  assert.equal((await put(f, M1, editA, meeting(), `Bearer ${'c'.repeat(64)}`)).code, 401);
  assert.equal((await put(f, M1, editA, meeting(), `Bearer ${f.ownerKey}`)).code, 401);
  res = await put(f, M1, editA, meeting({title:'Planning v2'}));
  assert.equal(res.code, 200);
  assert.notEqual(res.body.updatedAt, first);
  assert.deepEqual((await call(folderAPI, 'GET', {id:f.id})).body.meetings, [{id:M1, updatedAt:res.body.updatedAt}]);
  assert.equal((await call(meetingAPI, 'GET', {id:f.id, meeting:M1})).body.title, 'Planning v2');

  // Deleting: wrong edit key is refused, the author and the owner succeed, and deleting again is fine.
  const del = (mid, auth, edit) => call(meetingAPI, 'DELETE', {id:f.id, meeting:mid}, {auth, edit});
  assert.equal((await del(M1, `Bearer ${f.memberKey}`, editB)).code, 403);
  assert.equal((await del(M1, `Bearer ${f.memberKey}`)).code, 403);
  assert.equal((await del(M1, undefined, editA)).code, 401);
  assert.equal((await del(M1, `Bearer ${f.memberKey}`, editA)).code, 204);
  assert.equal((await call(meetingAPI, 'GET', {id:f.id, meeting:M1})).code, 404);
  assert.equal((await del(M1, `Bearer ${f.memberKey}`, editA)).code, 204);
  await put(f, M2, editB);
  assert.equal((await del(M2, `Bearer ${f.ownerKey}`)).code, 204);
  assert.deepEqual((await call(folderAPI, 'GET', {id:f.id})).body.meetings, []);
});

test('a folder holds at most 500 meetings, counted across every list page', async () => {
  const f = await newFolder();
  const hex = n => n.toString(16).toUpperCase().padStart(12, '0');
  for (let i = 0; i < 500; i++) store.set(`folders/${f.id}/meetings/00000000-0000-0000-0000-${hex(i)}.json`, {body:JSON.stringify({...meeting(), editKeyHash:sha(editA)}), uploadedAt:new Date(clock)});
  assert.equal((await call(folderAPI, 'GET', {id:f.id})).body.meetings.length, 500);
  const res = await put(f, M1, editA);
  assert.equal(res.code, 409);
  assert.match(res.body.error, /500/);
  assert.equal((await put(f, `00000000-0000-0000-0000-${hex(7)}`, editA)).code, 200);
});

test('the owner renames, resets and deletes the folder', async () => {
  const f = await newFolder();
  const rename = (auth, name) => call(folderAPI, 'PATCH', {id:f.id}, {auth, body:{name}});
  assert.equal((await rename(`Bearer ${f.memberKey}`, 'Nope')).code, 403);
  assert.equal((await rename(`Bearer ${f.ownerKey}`, '')).code, 400);
  assert.equal((await rename(`Bearer ${f.ownerKey}`, 'x'.repeat(81))).code, 400);
  let res = await rename(`Bearer ${f.ownerKey}`, 'Design');
  assert.deepEqual([res.code, res.body], [200, {name:'Design'}]);
  assert.equal((await call(folderAPI, 'GET', {id:f.id})).body.name, 'Design');

  await put(f, M1, editA); await put(f, M2, editB);
  assert.equal((await call(resetAPI, 'POST', {id:f.id}, {auth:`Bearer ${f.memberKey}`})).code, 403);
  assert.equal((await call(resetAPI, 'GET', {id:f.id}, {auth:`Bearer ${f.ownerKey}`})).code, 405);
  res = await call(resetAPI, 'POST', {id:f.id}, {auth:`Bearer ${f.ownerKey}`});
  assert.equal(res.code, 200);
  const moved = {id:res.body.id, memberKey:res.body.memberKey, ownerKey:f.ownerKey};
  assert.match(moved.id, /^[a-f0-9]{64}$/);
  assert.notEqual(moved.memberKey, f.memberKey);
  assert.equal((await call(folderAPI, 'GET', {id:f.id})).code, 404);
  assert.equal((await call(meetingAPI, 'GET', {id:f.id, meeting:M1})).code, 404);
  assert.equal((await pageAPI({method:'GET', query:{id:f.id}}, res = response()), res.code), 404);
  assert.ok(![...store.keys()].some(k => k.startsWith(`folders/${f.id}/`)));
  const after = (await call(folderAPI, 'GET', {id:moved.id}, {auth:`Bearer ${moved.memberKey}`})).body;
  assert.deepEqual([after.name, after.member, after.meetings.map(m => m.id).sort()], ['Design', true, [M1, M2]]);
  assert.equal((await call(folderAPI, 'GET', {id:moved.id}, {auth:`Bearer ${f.memberKey}`})).body.member, false);
  assert.equal((await put(moved, M1, editA, meeting({title:'Still mine'}))).code, 200);
  assert.equal((await put(moved, M2, editA)).code, 403);

  assert.equal((await call(folderAPI, 'DELETE', {id:moved.id}, {auth:`Bearer ${moved.memberKey}`})).code, 403);
  assert.equal((await call(folderAPI, 'DELETE', {id:moved.id}, {auth:`Bearer ${moved.ownerKey}`})).code, 204);
  assert.ok(![...store.keys()].some(k => k.startsWith(`folders/${moved.id}/`)));
  assert.equal((await call(folderAPI, 'GET', {id:moved.id})).code, 404);
  assert.equal((await put(moved, M1, editA)).code, 404);
});

test('folder and meeting pages render escaped, dated and linked', async () => {
  const f = await newFolder('<b>Team</b>');
  await put(f, M1, editA, meeting({title:'Older <script>', startedAt:'2026-09-01T12:00:00Z'}));
  await put(f, M2, editB, meeting({title:'Newer', recordedBy:'<i>Eve</i>', startedAt:'2026-09-22T23:30:00-05:00',
    note:{...meeting().note, title:'<script>alert(1)</script>'}, transcript:[{speaker:'me', start:75.9, end:80, text:'<img src=x>'}, {speaker:'them', start:3600, end:3601, text:'Late', speakerID:'2'}]}));
  let res = response();
  await pageAPI({method:'GET', query:{id:f.id}}, res);
  assert.equal(res.code, 200);
  assert.match(res.headers['Content-Security-Policy'], /default-src 'none'/);
  assert.equal(res.headers['Referrer-Policy'], 'no-referrer');
  assert.match(res.headers['Cache-Control'], /no-store/);
  assert.ok(res.body.includes('&lt;b&gt;Team&lt;/b&gt;') && !res.body.includes('<b>Team'));
  assert.ok(res.body.includes('Older &lt;script&gt;') && !res.body.includes('<script>'));
  assert.ok(res.body.indexOf('Newer') < res.body.indexOf('Older'));
  assert.ok(res.body.includes('23 Sep 2026 · Recorded by &lt;i&gt;Eve&lt;/i&gt;'));
  assert.ok(res.body.includes('1 Sep 2026 · Recorded by Dana'));
  assert.ok(res.body.includes(`href="/f/${f.id}/m/${M2}"`));

  res = response();
  await pageAPI({method:'GET', query:{id:f.id, meeting:M2}}, res);
  assert.equal(res.code, 200);
  assert.ok(res.body.includes(`<a href="/f/${f.id}">← &lt;b&gt;Team&lt;/b&gt;</a>`));
  assert.ok(res.body.includes('Recorded by &lt;i&gt;Eve&lt;/i&gt; · 23 Sep 2026'));
  assert.ok(res.body.includes('&lt;script&gt;alert(1)&lt;/script&gt;') && !res.body.includes('<script>') && !res.body.includes('<img'));
  assert.ok(res.body.includes('&lt;i&gt;Eve&lt;/i&gt;<time>01:15</time>'));
  assert.ok(res.body.includes('Speaker 2<time>60:00</time>'));

  for (const query of [{id:f.id, meeting:M2.toLowerCase()}, {id:f.id, meeting:'11111111-0000-0000-0000-000000000000'}, {id:'d'.repeat(64)}, {id:'nope'}]) {
    res = response(); await pageAPI({method:'GET', query}, res);
    assert.equal(res.code, 404);
    assert.match(res.body, /no longer|unavailable/);
  }
  res = response(); await pageAPI({method:'POST', query:{id:f.id}}, res); assert.equal(res.code, 405);
});

test('dates and times', () => {
  assert.equal(formatDate('2026-09-22T09:00:00Z'), '22 Sep 2026');
  assert.equal(formatDate('2026-12-31T23:30:00-01:00'), '1 Jan 2027');
  assert.equal(formatTime(0), '00:00');
  assert.equal(formatTime(65.9), '01:05');
});

test('invalid meeting content is rejected and unknown fields are dropped', async () => {
  const valid = validateMeeting({...meeting(), secret:'x', note:{...meeting().note, secret:'x'}, transcript:[{speaker:'me', start:1, end:2, text:'a', secret:'x'}]});
  assert.ok(!JSON.stringify(valid).includes('secret'));
  assert.equal(validateMeeting(meeting({endedAt:null})).endedAt, null);
  const bad = [
    {app:'teams'}, {title:'x'.repeat(301)}, {title:3}, {recordedBy:''}, {recordedBy:'   '}, {recordedBy:'x'.repeat(61)},
    {startedAt:'yesterday'}, {startedAt:'2026-13-45T00:00:00Z'}, {endedAt:undefined}, {note:undefined}, {note:{...meeting().note, title:5}},
    {note:{...meeting().note, actionItems:[{task:'x', done:'yes'}]}}, {transcript:undefined}, {transcript:'x'},
    {transcript:[{speaker:'Alice', start:0, end:1, text:'a'}]}, {transcript:[{speaker:'me', start:Infinity, end:1, text:'a'}]},
    {transcript:[{speaker:'me', start:'0', end:1, text:'a'}]}, {transcript:[{speaker:'me', start:0, end:1, text:'a', speakerName:4}]}
  ];
  for (const over of bad) assert.throws(() => validateMeeting(meeting(over)), undefined, JSON.stringify(over));

  const f = await newFolder();
  assert.equal((await put(f, M1, editA, meeting({app:'teams'}))).code, 400);
  assert.equal((await put(f, M1, editA, {...meeting(), padding:'x'.repeat(2000001)})).code, 413);
  assert.equal((await put(f, M1, undefined)).code, 400);
  assert.equal((await put(f, M1, 'short')).code, 400);
  assert.equal((await put(f, M2.toLowerCase(), editA)).code, 400);
  assert.equal((await call(meetingAPI, 'PUT', {id:'x', meeting:M1}, {auth:`Bearer ${f.memberKey}`, edit:editA, body:meeting()})).code, 400);
  assert.equal((await call(meetingAPI, 'POST', {id:f.id, meeting:M1})).code, 405);
  assert.equal((await call(folderAPI, 'PUT', {id:f.id})).code, 405);
  assert.equal((await call(folderAPI, 'POST', {}, {auth:SHARE, body:{name:42}})).code, 400);
  assert.equal((await call(folderAPI, 'POST', {}, {auth:SHARE, body:{name:'x'.repeat(2000001)}})).code, 413);
  assert.deepEqual((await call(folderAPI, 'GET', {id:f.id})).body.meetings, []);
});

test('dates must exist, as the app parses them', async () => {
  for (const startedAt of ['2026-02-30T00:00:00Z', '2025-02-29T00:00:00Z', '2026-04-31T00:00:00Z', '2026-00-10T00:00:00Z', '2026-13-01T00:00:00Z',
    '2026-01-01T24:00:00Z', '2026-01-01T12:60:00Z', '2026-01-01T12:00:60Z', '2026-01-01T12:00:00+24:00', '2026-01-01T12:00:00+05:60',
    '2026-01-01 12:00:00Z', '2026-01-01T12:00Z', '2026-01-01T12:00:00', '2026-01-01T12:00:00.Z', '2026-01-01T12:00:00.1234567890Z'])
    assert.throws(() => validateMeeting(meeting({startedAt})), undefined, startedAt);
  for (const startedAt of ['2028-02-29T00:00:00Z', '2000-02-29T23:59:59.999999999+05:30', '2026-12-31T00:00:00-12:00'])
    assert.equal(validateMeeting(meeting({startedAt})).startedAt, startedAt);
  assert.throws(() => validateMeeting(meeting({endedAt:'1900-02-29T00:00:00Z'})));
  const f = await newFolder();
  assert.equal((await put(f, M1, editA, meeting({startedAt:'2026-02-30T00:00:00Z'}))).code, 400);
});

test('each meeting has a summary blob that the folder page reads instead of the meeting', async () => {
  const f = await newFolder();
  const summary = mid => store.get(`folders/${f.id}/summaries/${mid}.json`);
  await put(f, M1, editA, meeting({title:'First'}));
  await put(f, M2, editB, meeting({title:'Second', recordedBy:'Eve'}));
  assert.deepEqual(JSON.parse(summary(M2).body), {title:'Second', startedAt:'2026-09-22T09:00:00Z', recordedBy:'Eve'});
  await put(f, M2, editB, meeting({title:'Second v2', recordedBy:'Eve'}));
  assert.equal(JSON.parse(summary(M2).body).title, 'Second v2');
  assert.deepEqual((await call(folderAPI, 'GET', {id:f.id})).body.meetings.map(m => m.id).sort(), [M1, M2]);

  hooks.reads = [];
  let res = response(); await pageAPI({method:'GET', query:{id:f.id}}, res);
  assert.ok(res.body.includes('First') && res.body.includes('Second v2'));
  assert.ok(!hooks.reads.some(p => p.includes('/meetings/')), 'the page must not download full meetings');
  assert.equal(hooks.reads.filter(p => p.includes('/summaries/')).length, 2);

  // A meeting without a summary falls back to the full meeting; a summary without a meeting is skipped.
  store.delete(`folders/${f.id}/summaries/${M1}.json`);
  store.set(`folders/${f.id}/summaries/00000000-0000-0000-0000-000000000009.json`, {body:JSON.stringify({title:'Orphan', startedAt:'2026-09-22T09:00:00Z', recordedBy:'X'}), uploadedAt:new Date(clock)});
  res = response(); await pageAPI({method:'GET', query:{id:f.id}}, res);
  assert.ok(res.body.includes('First') && !res.body.includes('Orphan'));
  store.delete(`folders/${f.id}/summaries/00000000-0000-0000-0000-000000000009.json`);
  await put(f, M1, editA, meeting({title:'First'}));

  assert.equal((await call(meetingAPI, 'DELETE', {id:f.id, meeting:M1}, {auth:`Bearer ${f.memberKey}`, edit:editA})).code, 204);
  assert.equal(summary(M1), undefined);
  res = await call(resetAPI, 'POST', {id:f.id}, {auth:`Bearer ${f.ownerKey}`});
  const moved = res.body.id;
  assert.deepEqual(JSON.parse(store.get(`folders/${moved}/summaries/${M2}.json`).body).title, 'Second v2');
  assert.equal((await call(meetingAPI, 'DELETE', {id:moved, meeting:M2}, {auth:`Bearer ${f.ownerKey}`})).code, 204);
  assert.equal(store.get(`folders/${moved}/summaries/${M2}.json`), undefined);
  await put({id:moved, memberKey:res.body.memberKey}, M1, editA);
  assert.equal((await call(folderAPI, 'DELETE', {id:moved}, {auth:`Bearer ${f.ownerKey}`})).code, 204);
  assert.ok(![...store.keys()].some(k => k.startsWith(`folders/${moved}/`)));
});

test('a reset succeeds once the new folder exists, even if removing the old one fails', async () => {
  const f = await newFolder();
  await put(f, M1, editA); await put(f, M2, editB);
  const old = p => p.startsWith(`folders/${f.id}/`);
  hooks.failDel = p => old(p) && !p.endsWith('/folder.json');
  let res = await call(resetAPI, 'POST', {id:f.id}, {auth:`Bearer ${f.ownerKey}`});
  hooks.failDel = null;
  assert.equal(res.code, 200);
  assert.match(res.body.id, /^[a-f0-9]{64}$/);
  assert.equal(res.body.oldLinkStillActive, undefined);
  assert.equal((await call(folderAPI, 'GET', {id:f.id})).code, 404, 'the old link dies even when its meetings could not be removed');
  assert.equal((await call(folderAPI, 'GET', {id:res.body.id})).body.meetings.length, 2);

  const g = await newFolder();
  hooks.failDel = p => p.startsWith(`folders/${g.id}/`);
  res = await call(resetAPI, 'POST', {id:g.id}, {auth:`Bearer ${g.ownerKey}`});
  hooks.failDel = null;
  assert.equal(res.code, 200);
  assert.equal(res.body.oldLinkStillActive, true, 'the owner is told when the old link could not be closed');
  assert.equal((await call(folderAPI, 'GET', {id:res.body.id}, {auth:`Bearer ${res.body.memberKey}`})).body.member, true);
});

test('a rename racing a reset or another rename does not bring the old folder back', async () => {
  const f = await newFolder();
  const folderPath = `folders/${f.id}/folder.json`;
  const rename = name => call(folderAPI, 'PATCH', {id:f.id}, {auth:`Bearer ${f.ownerKey}`, body:{name}});
  // The reset deletes folder.json after the rename read it...
  hooks.afterGet = p => { if (p === folderPath) { hooks.afterGet = null; store.delete(folderPath); } };
  let res = await rename('Late');
  assert.equal(res.code, 404);
  assert.equal(typeof res.body.error, 'string');
  assert.equal(store.has(folderPath), false);
  // ...or between the rename's head and its write.
  const g = await newFolder();
  const gPath = `folders/${g.id}/folder.json`;
  hooks.afterHead = p => { if (p === gPath) { hooks.afterHead = null; store.delete(gPath); } };
  res = await call(folderAPI, 'PATCH', {id:g.id}, {auth:`Bearer ${g.ownerKey}`, body:{name:'Late'}});
  assert.equal(res.code, 404);
  assert.equal(store.has(gPath), false);
  // Another rename in between gives 409 and keeps its result.
  const h = await newFolder();
  const hPath = `folders/${h.id}/folder.json`;
  hooks.afterHead = p => { if (p === hPath) { hooks.afterHead = null; store.set(hPath, {...store.get(hPath), body:JSON.stringify({...JSON.parse(store.get(hPath).body), name:'Other'}), etag:'"other"'}); } };
  res = await call(folderAPI, 'PATCH', {id:h.id}, {auth:`Bearer ${h.ownerKey}`, body:{name:'Mine'}});
  assert.equal(res.code, 409);
  assert.equal((await call(folderAPI, 'GET', {id:h.id})).body.name, 'Other');
});

test('every API 404 carries a JSON error', async () => {
  const f = await newFolder();
  const gone = {id:'e'.repeat(64)}, auth = `Bearer ${f.ownerKey}`;
  const responses = [
    await call(folderAPI, 'GET', gone), await call(folderAPI, 'PATCH', gone, {auth, body:{name:'x'}}), await call(folderAPI, 'DELETE', gone, {auth}),
    await call(resetAPI, 'POST', gone, {auth}), await call(meetingAPI, 'GET', {...gone, meeting:M1}),
    await call(meetingAPI, 'PUT', {...gone, meeting:M1}, {auth, edit:editA, body:meeting()}), await call(meetingAPI, 'DELETE', {...gone, meeting:M1}, {auth}),
    await call(meetingAPI, 'GET', {id:f.id, meeting:M1})
  ];
  for (const res of responses) { assert.equal(res.code, 404); assert.equal(typeof res.body?.error, 'string'); assert.ok(res.body.error.length > 0); }
});
