import { get, put, del } from '@vercel/blob';

export const validID = id => typeof id === 'string' && /^[a-f0-9]{64}$/.test(id);
const pathname = id => `notes/${id}.json`;
export async function read(id) {
  const result = await get(pathname(id), {access:'private', useCache:false});
  if (!result) return null;
  if (result.statusCode !== 200) throw new Error('Storage unavailable');
  return JSON.parse(await new Response(result.stream).text());
}
export async function write(id, data) {
  await put(pathname(id), JSON.stringify(data), {access:'private',addRandomSuffix:false,allowOverwrite:true,contentType:'application/json'});
}
export async function remove(id) { await del(pathname(id)); }
