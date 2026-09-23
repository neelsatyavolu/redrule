import { timingSafeEqual } from 'node:crypto';

export function authorized(header, secret) {
  if (!secret || typeof header !== 'string') return false;
  const actual = Buffer.from(header), expected = Buffer.from(`Bearer ${secret}`);
  return actual.length === expected.length && timingSafeEqual(actual, expected);
}

export function validateNote(input) {
  const text = (v, max = 100000) => {
    if (typeof v !== 'string' || v.length > max) throw new Error('Invalid text');
    return v;
  };
  const array = (v, fn, max = 5000) => {
    if (!Array.isArray(v) || v.length > max) throw new Error('Invalid list');
    return v.map(fn);
  };
  const n = input?.note;
  if (!n) throw new Error('Missing note');
  const note = {
    title: text(n.title, 1000), tldr: text(n.tldr),
    sections: array(n.sections, s => ({heading:text(s.heading,1000), bullets:array(s.bullets,b=>text(b))}),100),
    decisions: array(n.decisions,d=>text(d)),
    actionItems: array(n.actionItems,a=>({owner:a.owner == null ? null : text(a.owner,200),task:text(a.task)}))
  };
  const result = {note};
  if (input.transcript != null) result.transcript = array(input.transcript,s=>({speaker:text(s.speaker,200),time:text(s.time,30),text:text(s.text)}),20000);
  return result;
}

const escape = value => String(value).replace(/[&<>"']/g, c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
export function renderNote({note:n,transcript}) {
  const list = values => `<ul>${values.map(v=>`<li>${escape(v)}</li>`).join('')}</ul>`;
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="robots" content="noindex,nofollow,noarchive"><title>${escape(n.title)} · Redrule</title><link rel="stylesheet" href="/style.css"></head><body><main><header><a href="/">Redrule</a><span>Shared meeting notes</span></header><article><p class="eyebrow">ON THE RECORD</p><h1>${escape(n.title)}</h1><p class="summary">${escape(n.tldr)}</p>${n.sections.map(s=>`<section><h2>${escape(s.heading)}</h2>${list(s.bullets)}</section>`).join('')}${n.decisions.length?`<section><h2>Decisions</h2>${list(n.decisions)}</section>`:''}${n.actionItems.length?`<section><h2>Action items</h2><ul class="actions">${n.actionItems.map(a=>`<li>${a.owner?`<span class="owner">${escape(a.owner)}</span>`:''}${escape(a.task)}</li>`).join('')}</ul></section>`:''}${transcript?`<details><summary>Transcript <span>${transcript.length} passages</span></summary>${transcript.map(s=>`<div class="passage"><div class="speaker">${escape(s.speaker)}<time>${escape(s.time)}</time></div><p>${escape(s.text)}</p></div>`).join('')}</details>`:''}</article><footer>Shared with Redrule · Anyone with this link can read this page.</footer></main></body></html>`;
}
