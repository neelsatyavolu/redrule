// A read-only MCP server over one shared folder, for cloud agents such as Grok Bot that can't reach the Mac.
// Stateless Streamable HTTP: each POST carries one JSON-RPC message and gets a JSON reply.
import { formatDate } from './folder-render.js';
import { oneLine } from './markdown.js';

export const PROTOCOL_VERSION = '2025-06-18';
const READ_ONLY = {readOnlyHint:true, openWorldHint:false};

const TOOLS = [
  {name:'list_meetings', description:'Lists the meetings in this shared Redrule folder, newest first: id, date, title and who recorded it.',
    inputSchema:{type:'object', properties:{}}, annotations:READ_ONLY},
  {name:'get_meeting', description:'Returns one meeting as Markdown: summary, sections, decisions, action items and transcript.',
    inputSchema:{type:'object', required:['id'], properties:{id:{type:'string', description:'Meeting id from list_meetings.'}}}, annotations:READ_ONLY},
];

const text = (value, isError = false) => ({content:[{type:'text', text:value}], ...(isError ? {isError:true} : {})});
const newestFirst = meetings => [...meetings].sort((a, b) => Date.parse(b.startedAt) - Date.parse(a.startedAt));

// folder: {name, listMeetings: () => [{id, title, startedAt, recordedBy}], meetingMarkdown: id => string | null}
async function callTool(params, folder) {
  const name = params?.name, args = params?.arguments ?? {};
  if (name === 'list_meetings') {
    const meetings = newestFirst(await folder.listMeetings());
    const lines = meetings.map(m => `- ${m.id} · ${formatDate(m.startedAt)} · ${oneLine(m.title || 'Untitled meeting')} · Recorded by ${oneLine(m.recordedBy)}`);
    return text(lines.length ? `# ${oneLine(folder.name)}\n\n${lines.join('\n')}` : 'This folder has no meetings yet.');
  }
  if (name === 'get_meeting') {
    const id = typeof args.id === 'string' ? args.id.trim().toUpperCase() : '';
    const markdown = id ? await folder.meetingMarkdown(id) : null;
    return markdown ? text(markdown) : text(`No meeting in this folder has the id ${id || '(none)'}. Use list_meetings to find one.`, true);
  }
  return text(`Unknown tool: ${name}`, true);
}

// Returns the JSON-RPC reply, or null for a notification.
export async function handleMessage(message, folder) {
  if (message === null || typeof message !== 'object' || Array.isArray(message)) return {jsonrpc:'2.0', id:null, error:{code:-32600, message:'Invalid request'}};
  if (message.id === undefined) return null;
  const reply = result => ({jsonrpc:'2.0', id:message.id, result});
  switch (message.method) {
    case 'initialize': return reply({
      protocolVersion: typeof message.params?.protocolVersion === 'string' ? message.params.protocolVersion : PROTOCOL_VERSION,
      capabilities:{tools:{}},
      serverInfo:{name:'redrule', title:`Redrule · ${oneLine(folder.name)}`, version:'1'},
      instructions:'Meeting notes and transcripts from a shared Redrule folder. Use list_meetings to find a meeting id, then get_meeting to read it.',
    });
    case 'ping': return reply({});
    case 'tools/list': return reply({tools:TOOLS});
    case 'tools/call': return reply(await callTool(message.params, folder));
    default: return {jsonrpc:'2.0', id:message.id, error:{code:-32601, message:`Unknown method: ${message.method}`}};
  }
}
