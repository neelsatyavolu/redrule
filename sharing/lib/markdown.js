// Markdown versions of the shared pages, for AI agents and other tools that read text rather than HTML. A page's
// Markdown is at its URL plus .md, and at the URL itself when the request's Accept header names text/markdown.
export const MARKDOWN = 'text/markdown; charset=utf-8';

// Splits a trailing .md off a path parameter: 'abc.md' gives {value:'abc', markdown:true}.
export function markdownPath(value) {
  const markdown = typeof value === 'string' && value.endsWith('.md');
  return {value:markdown ? value.slice(0, -3) : value, markdown};
}
export const wantsMarkdown = (req, suffix) => suffix || /\btext\/markdown\b/i.test(req.headers?.accept ?? '');
// The https origin the request came to, so links in the Markdown work outside a browser. Empty without a Host header.
export const origin = req => req.headers?.host ? `https://${req.headers.host}` : '';

export const oneLine = value => String(value).replace(/\s+/g, ' ').trim();
export const linkText = value => oneLine(value).replace(/[[\]]/g, '\\$&');
// Indents continuation lines so text with line breaks stays inside its list item.
const indent = value => value.replace(/\r?\n/g, '\n  ');
const list = values => values.map(v => `- ${indent(v)}`).join('\n');

// Mirrors renderArticle. byline is Markdown; callers escape what goes into it.
export function noteMarkdown({note:n, transcript}, byline = '') {
  const blocks = [`# ${oneLine(n.title) || 'Untitled meeting'}`, byline, n.tldr && `## Summary\n\n${n.tldr}`,
    ...n.sections.map(s => [`## ${oneLine(s.heading)}`, list(s.bullets)].filter(Boolean).join('\n\n'))];
  if (n.decisions.length) blocks.push(`## Decisions\n\n${list(n.decisions)}`);
  if (n.actionItems.length) blocks.push(`## Action items\n\n${n.actionItems.map(a => `- [${a.done ? 'x' : ' '}] ${a.owner ? `**${oneLine(a.owner)}:** ` : ''}${indent(a.task)}`).join('\n')}`);
  if (transcript) blocks.push(`## Transcript\n\n${transcript.map(s => `**${oneLine(s.speaker)}** [${s.time}]: ${s.text}`).join('\n\n')}`);
  return `${blocks.filter(Boolean).join('\n\n')}\n`;
}
