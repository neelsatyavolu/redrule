// Best-effort rate limits held in each function instance's memory: fixed windows per client IP and bucket. Vercel runs
// several instances, so this stops a burst from one client, not a spread-out flood; the WAF rules in the README do that.
// [requests, seconds] per client.
export const LIMITS = {create:[20, 3600], share:[60, 600], write:[600, 600], page:[30, 600], read:[1200, 600]};
const MAX_CLIENTS = 10000;
const windows = new Map();

// Vercel sets both headers to the caller's address. Without one every caller would share a bucket, so there is no limit.
export const clientIP = req => String(req.headers?.['x-real-ip'] ?? req.headers?.['x-forwarded-for'] ?? '').split(',')[0].trim();

// One IPv6 subscriber usually holds a whole /64, so those addresses count as one client.
export function clientKey(ip) {
  if (!ip.includes(':') || ip.includes('.')) return ip;
  const [head, tail = ''] = ip.toLowerCase().split('::');
  const left = head ? head.split(':') : [], right = tail ? tail.split(':') : [];
  const groups = ip.includes('::') ? [...left, ...Array(Math.max(0, 8 - left.length - right.length)).fill('0'), ...right] : left;
  return groups.slice(0, 4).map(g => g.replace(/^0+(?=.)/, '')).join(':') + '::/64';
}

// Counts the request. Returns 0 when it may go ahead, or the seconds until the client's window ends.
export function limited(req, bucket, now = Date.now()) {
  const ip = clientIP(req);
  if (!ip) return 0;
  const [max, seconds] = LIMITS[bucket], key = `${bucket} ${clientKey(ip)}`;
  if (windows.size >= MAX_CLIENTS) for (const [k, w] of windows) if (w.reset <= now) windows.delete(k);
  // Still full: drop the oldest windows rather than all of them, so a flood can't reset everyone's count.
  for (const k of windows.keys()) { if (windows.size < MAX_CLIENTS) break; windows.delete(k); }
  const w = windows.get(key);
  const next = w && w.reset > now ? {count:w.count + 1, reset:w.reset} : {count:1, reset:now + seconds * 1000};
  windows.set(key, next);
  return next.count > max ? Math.ceil((next.reset - now) / 1000) : 0;
}

export const TOO_MANY = 'Too many requests from this network. Please wait a few minutes and try again.';
export function tooMany(res, seconds, json = true) {
  res.setHeader('Retry-After', String(seconds));
  return json ? res.status(429).json({error:TOO_MANY}) : res.status(429).send(TOO_MANY);
}
