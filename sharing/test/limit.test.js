import test from 'node:test';
import assert from 'node:assert/strict';
import { LIMITS, clientIP, limited } from '../lib/limit.js';

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
