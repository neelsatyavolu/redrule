import test from 'node:test';
import assert from 'node:assert/strict';
import { validateNote, authorized, renderNote } from '../lib/notes.js';
const note = {title:'Planning',tldr:'Summary',sections:[],decisions:[],actionItems:[]};
test('validates and projects only public note fields', () => {
 const result = validateNote({note, transcript:[{speaker:'Alice',time:'00:01',text:'Hello',secret:'hidden'}],secret:'hidden'});
 assert.equal(result.note.title,'Planning');
 assert.equal(JSON.stringify(result).includes('hidden'),false);
 assert.throws(()=>validateNote({note:{...note,title:42}}));
 assert.throws(()=>validateNote({note,transcript:'wrong'}));
});
test('write authorization requires configured credential',()=>{
 assert.equal(authorized(undefined,undefined),false);
 assert.equal(authorized('Bearer secret','secret'),true);
 assert.equal(authorized('Bearer wrong','secret'),false);
});
test('renders escaped text and optional transcript',()=>{
 const html=renderNote(validateNote({note:{...note,title:'<script>alert(1)</script>'},transcript:[{speaker:'<img>',time:'00:01',text:'<script>bad</script>'}]}));
 assert.ok(!html.includes('<script>alert'));
 assert.ok(html.includes('&lt;script&gt;'));
 assert.ok(html.includes('Transcript'));
 assert.ok(!renderNote(validateNote({note})).includes('<summary>Transcript'));
});
