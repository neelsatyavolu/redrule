import { test, mock } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
const notes = new Map();
mock.module('../lib/storage.js', {namedExports:{
 validID: id=> typeof id==='string' && /^[a-f0-9]{64}$/.test(id),
 write: async(id,data)=>notes.set(id,data), read: async id=>notes.get(id)??null,
 remove: async id=>notes.delete(id)
}});
const {default:share}=await import('../api/share.js');
const {default:page}=await import('../api/note.js');
process.env.MINUTES_SHARE_KEY='test-only-credential';
const id='a'.repeat(64);
const body={note:{title:'Test meeting',tldr:'Summary',sections:[],decisions:[],actionItems:[]}};
function response(){return {code:200,headers:{},status(code){this.code=code;return this},setHeader(k,v){this.headers[k]=v;return this},json(body){this.body=body;return this},send(body){this.body=body;return this},end(){return this}}}
function request(method,body,auth='Bearer test-only-credential'){return {method,body,query:{id},headers:{authorization:auth}}}
test('publish, read without login, update and revoke',async()=>{
 let res=response(); await share(request('PUT',body),res);assert.equal(res.code,200);
 res=response();await page(request('GET',undefined,''),res);assert.equal(res.code,200);assert.match(res.body,/Test meeting/);assert.match(res.headers['Cache-Control'],/no-store/);
 res=response();await share(request('PUT',{...body,transcript:[{speaker:'Alice',time:'00:01',text:'Hello'}]}),res);assert.equal(res.code,200);
 res=response();await page(request('GET'),res);assert.match(res.body,/Alice/);
 res=response();await share(request('PUT',body),res);assert.equal(res.code,200);
 res=response();await page(request('GET'),res);assert.ok(!res.body.includes('Alice'));
 res=response();await share(request('DELETE'),res);assert.equal(res.code,204);
 res=response();await page(request('GET'),res);assert.equal(res.code,404);
});
test('unauthorized writes, invalid content and oversize payloads cannot publish',async()=>{
 for(const method of ['PUT','DELETE']){const res=response();await share(request(method,body,'Bearer invalid'),res);assert.equal(res.code,401)}
 let res=response();await share(request('PUT',{note:{}}),res);assert.equal(res.code,400);
 res=response();await share(request('PUT',{...body,extra:'x'.repeat(2000001)}),res);assert.equal(res.code,413);
 res=response();await share({...request('PUT',body),query:{id:'../secret'}},res);assert.equal(res.code,400);
});
const sha=s=>createHash('sha256').update(s).digest('hex');
const ownerKey='c'.repeat(64), otherKey='d'.repeat(64), ownID=sha(ownerKey);
const call=async(method,{id:shareID=ownID,auth,ip,payload}={})=>{const res=response();await share({method,body:payload,query:{id:shareID},headers:{...(auth===undefined?{}:{authorization:auth}),...(ip===undefined?{}:{'x-real-ip':ip})}},res);return res};
test('anyone publishes with a new owner key; only that key, or the legacy key, changes or removes the link',async()=>{
 assert.equal((await call('PUT',{auth:`Bearer ${ownerKey}`,payload:body})).code,200);
 assert.ok(!JSON.stringify(notes.get(ownID)).includes(ownerKey));
 for(const method of ['PUT','DELETE']){
  assert.equal((await call(method,{auth:`Bearer ${otherKey}`,payload:body})).code,403);
  assert.equal((await call(method,{payload:body})).code,401);
 }
 assert.equal((await call('PUT',{auth:`Bearer ${ownerKey}`,payload:{...body,note:{...body.note,title:'Renamed'}}})).code,200);
 assert.equal(notes.get(ownID).note.title,'Renamed');
 assert.equal((await call('PUT',{auth:'Bearer test-only-credential',payload:body})).code,200);
 assert.equal((await call('DELETE',{auth:`Bearer ${ownerKey}`})).code,204);
 assert.equal(notes.has(ownID),false);
});
test('links from before owner keys answer only to the legacy key',async()=>{
 const legacy='e'.repeat(64);
 assert.equal((await call('PUT',{id:legacy,auth:'Bearer test-only-credential',payload:body})).code,200);
 assert.equal((await call('PUT',{id:legacy,auth:`Bearer ${ownerKey}`,payload:body})).code,403);
 assert.equal((await call('DELETE',{id:legacy,auth:`Bearer ${ownerKey}`})).code,403);
 assert.equal(notes.has(legacy),true);
 assert.equal((await call('DELETE',{id:legacy,auth:'Bearer test-only-credential'})).code,204);
});
test('share writes are rate limited per client',async()=>{
 const {LIMITS}=await import('../lib/limit.js');
 for(let i=0;i<LIMITS.share[0];i++) assert.equal((await call('PUT',{auth:`Bearer ${ownerKey}`,ip:'203.0.113.9',payload:body})).code,200);
 notes.clear();
 const res=await call('PUT',{auth:`Bearer ${ownerKey}`,ip:'203.0.113.9',payload:body});
 assert.equal(res.code,429);assert.equal(typeof res.body.error,'string');assert.equal(notes.size,0);
 assert.equal((await call('PUT',{auth:`Bearer ${ownerKey}`,ip:'203.0.113.10',payload:body})).code,200);
});
