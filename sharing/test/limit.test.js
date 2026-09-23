import test from 'node:test';
import assert from 'node:assert/strict';
import { LIMITS, clientIP, clientKey, limited } from '../lib/limit.js';

const from = ip => ({headers:ip === undefined ? {} : {'x-real-ip':ip}});

test('counts requests per client and bucket in fixed windows', () => {
  const [max, seconds] = LIMITS.create, start = 1_000_000;
  for (let i = 0; i < max; i++) assert.equal(limited(from('192.0.2.1'), 'create', start), 0);
  assert.equal(limited(from('192.0.2.1'), 'create', start + 1000), seconds - 1);
  assert.equal(limited(from('192.0.2.2'), 'create', start + 1000), 0, 'another client has its own window');
  assert.equal(limited(from('192.0.2.1'), 'share', start + 1000), 0, 'another bucket has its own window');
  assert.equal(limited(from('192.0.2.1'), 'create', start + seconds * 1000), 0, 'a new window starts afresh');
});

test('reads the client from Vercel headers and never limits an unknown client', () => {
  assert.equal(clientIP({headers:{'x-forwarded-for':'198.51.100.7, 10.0.0.1'}}), '198.51.100.7');
  assert.equal(clientIP({headers:{'x-real-ip':'198.51.100.8', 'x-forwarded-for':'198.51.100.7'}}), '198.51.100.8');
  for (let i = 0; i < LIMITS.create[0] + 5; i++) assert.equal(limited(from(undefined), 'create'), 0);
});

test('an IPv6 /64 counts as one client', () => {
  assert.equal(clientKey('2001:db8:0:1:aaaa::1'), '2001:db8:0:1::/64');
  assert.equal(clientKey('2001:db8::1'), '2001:db8:0:0::/64');
  assert.equal(clientKey('2001:DB8:0:1:ffff:ffff:ffff:ffff'), '2001:db8:0:1::/64');
  assert.equal(clientKey('192.0.2.7'), '192.0.2.7');
  assert.equal(clientKey('::ffff:192.0.2.7'), '::ffff:192.0.2.7');
  const [max] = LIMITS.create, start = 9e12;
  for (let i = 0; i < max; i++) assert.equal(limited(from(`2001:db8:0:9::${i + 1}`), 'create', start), 0);
  assert.ok(limited(from('2001:db8:0:9::ffff'), 'create', start) > 0, 'a fresh address in the same /64 is still limited');
});
