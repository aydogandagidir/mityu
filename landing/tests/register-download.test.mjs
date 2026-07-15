import assert from 'node:assert/strict';
import { createHash, createHmac } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const endpointUrl = 'https://mityu.bluedev.dev/api/register-download';

function request(body, headers = { 'content-type': 'application/json' }, method = 'POST') {
  const init = { method, headers };
  if (method !== 'GET' && method !== 'HEAD') {
    init.body = typeof body === 'string' ? body : JSON.stringify(body);
  }
  return new Request(endpointUrl, init);
}

function clearStorageEnvironment() {
  delete process.env.KV_REST_API_URL;
  delete process.env.KV_REST_API_TOKEN;
  delete process.env.UPSTASH_REDIS_REST_URL;
  delete process.env.UPSTASH_REDIS_REST_TOKEN;
  delete process.env.DOWNLOAD_RATE_LIMIT_HMAC_SECRET;
}

test('strict validation and privacy-minimised anonymous path', async () => {
  clearStorageEnvironment();
  const { default: handler } = await import('../api/register-download.js?validation-test');

  const cases = [
    [request(null, {}, 'GET'), 405],
    [request({}, { 'content-type': 'text/plain' }), 415],
    [request({ unexpected: true }), 422],
    [request({ marketingConsent: true, email: 'person@example.com' }), 422],
    [request({ email: 'person@example.com' }), 422],
    [request(`"${'x'.repeat(9000)}"`), 413],
  ];
  for (const [input, expected] of cases) {
    const response = await handler(input);
    assert.equal(response.status, expected);
  }

  const anonymous = await handler(request({ website: '' }));
  assert.equal(anonymous.status, 200);
  assert.equal((await anonymous.json()).stored, false);

  const honeypot = await handler(request({ website: 'spam' }));
  assert.equal(honeypot.status, 200);
  assert.equal((await honeypot.json()).stored, false);
});

test('configured KV fails closed before any write when configuration is incomplete or unsafe', async () => {
  let fetchCalls = 0;
  const originalFetch = globalThis.fetch;
  globalThis.fetch = async () => {
    fetchCalls += 1;
    throw new Error('KV must not be called with unsafe configuration');
  };

  try {
    for (const [moduleId, token, secret] of [
      ['missing-hmac-secret', 'test-only', undefined],
      ['short-hmac-secret', 'test-only', 'too-short'],
      ['partial-kv-credentials', undefined, 'test-only-secret-with-at-least-32-bytes-0001'],
    ]) {
      clearStorageEnvironment();
      process.env.KV_REST_API_URL = 'https://kv.invalid';
      if (token !== undefined) process.env.KV_REST_API_TOKEN = token;
      if (secret === undefined) delete process.env.DOWNLOAD_RATE_LIMIT_HMAC_SECRET;
      else process.env.DOWNLOAD_RATE_LIMIT_HMAC_SECRET = secret;

      const { default: handler } = await import(`../api/register-download.js?${moduleId}`);
      const response = await handler(request({ website: '' }));
      assert.equal(response.status, 503);
      assert.equal(response.headers.get('retry-after'), '60');
      assert.deepEqual(await response.json(), {
        error: 'Download registration is temporarily unavailable.',
        stored: false,
        counted: false,
      });
    }
    assert.equal(fetchCalls, 0, 'unsafe configuration must not write any KV key');
  } finally {
    globalThis.fetch = originalFetch;
    clearStorageEnvironment();
  }
});

test('KV path uses a keyed IP token, fixed TTL, and never writes lead data', async () => {
  const hmacSecret = 'test-only-secret-with-at-least-32-bytes-0001';
  const sourceAddress = '203.0.113.10';
  process.env.KV_REST_API_URL = 'https://kv.invalid';
  process.env.KV_REST_API_TOKEN = 'test-only';
  process.env.DOWNLOAD_RATE_LIMIT_HMAC_SECRET = hmacSecret;
  let rateCount = 0;
  let expiryApplications = 0;
  const pipelines = [];
  const originalFetch = globalThis.fetch;
  globalThis.fetch = async (_url, init) => {
    const commands = JSON.parse(init.body);
    pipelines.push(commands);
    if (commands[0][0] === 'EVAL') {
      const [command, script, keyCount, key, windowSeconds] = commands[0];
      assert.equal(command, 'EVAL');
      assert.equal(keyCount, 1);
      assert.match(script, /if count == 1 then redis\.call\('EXPIRE'/);
      assert.equal(key.startsWith('downloads:rate:v2:'), true);
      assert.equal(windowSeconds, '900');
      rateCount += 1;
      if (rateCount === 1) expiryApplications += 1;
      const simulatedTtl = 901 - rateCount;
      return new Response(JSON.stringify([{ result: [rateCount, simulatedTtl] }]), { status: 200 });
    }
    return new Response(JSON.stringify(commands.map(() => ({ result: 'OK' }))), { status: 200 });
  };

  try {
    const { default: handler } = await import('../api/register-download.js?kv-test');
    const withIp = (body) => request(body, {
      'content-type': 'application/json',
      'x-vercel-forwarded-for': sourceAddress,
      'user-agent': 'private-agent',
    });

    for (let index = 0; index < 5; index += 1) {
      const response = await handler(withIp({ website: '' }));
      assert.equal(response.status, 200);
    }
    const limited = await handler(withIp({ website: '' }));
    assert.equal(limited.status, 429);
    assert.equal(limited.headers.get('retry-after'), '895');

    const ratePipelines = pipelines.filter((commands) => commands[0][0] === 'EVAL');
    assert.equal(ratePipelines.length, 6);
    assert.equal(expiryApplications, 1, 'the fixed-window TTL is applied only at count 1');
    const rateKeys = ratePipelines.map((commands) => commands[0][3]);
    assert.equal(new Set(rateKeys).size, 1, 'the same address must use one fixed-window key');
    const expectedToken = createHmac('sha256', hmacSecret)
      .update(`register-download:v2:${sourceAddress}`)
      .digest('hex');
    assert.equal(rateKeys[0], `downloads:rate:v2:${expectedToken}`);
    const unkeyedToken = createHash('sha256').update(sourceAddress).digest('hex');
    assert.notEqual(rateKeys[0], `downloads:rate:v2:${unkeyedToken}`);

    assert.equal(
      pipelines.some((commands) => commands.some((command) =>
        ['SET', 'LPUSH', 'RPUSH', 'SADD', 'HSET'].includes(command[0]))),
      false,
      'v1.0.4 endpoint must never store a lead record',
    );
    const serialized = JSON.stringify(pipelines);
    assert.equal(serialized.includes(sourceAddress), false);
    assert.equal(serialized.includes('private-agent'), false);
    assert.equal(serialized.includes(hmacSecret), false);
  } finally {
    globalThis.fetch = originalFetch;
    clearStorageEnvironment();
  }
});

test('Vercel configuration declares the landing security header baseline', async () => {
  const config = JSON.parse(await readFile(new URL('../vercel.json', import.meta.url), 'utf8'));
  const globalHeaders = config.headers.find((entry) => entry.source === '/(.*)')?.headers ?? [];
  const names = new Set(globalHeaders.map((entry) => entry.key.toLowerCase()));
  for (const required of [
    'content-security-policy',
    'referrer-policy',
    'permissions-policy',
    'x-content-type-options',
    'x-frame-options',
    'strict-transport-security',
  ]) {
    assert.equal(names.has(required), true, `missing ${required}`);
  }

  const deploymentGuide = await readFile(new URL('../README.md', import.meta.url), 'utf8');
  assert.match(deploymentGuide, /DOWNLOAD_RATE_LIMIT_HMAC_SECRET/);
  assert.match(deploymentGuide, /at least 32 bytes/i);
  assert.match(deploymentGuide, /returns `503` before any KV write/i);
});

test('public privacy notices disclose rate limiting and disabled contact collection', async () => {
  const websiteNotice = await readFile(new URL('../privacy.html', import.meta.url), 'utf8');
  const downloadNotice = await readFile(new URL('../index.html', import.meta.url), 'utf8');
  const repositoryNotice = await readFile(new URL('../../PRIVACY_POLICY.md', import.meta.url), 'utf8');

  assert.doesNotMatch(downloadNotice, /id="dl(?:Email|Name|Company|Consent)"/i);
  assert.match(downloadNotice, /JSON\.stringify\(\{ website: websiteEl\.value \}\)/);

  for (const notice of [websiteNotice, downloadNotice, repositoryNotice]) {
    assert.match(notice, /keyed HMAC/i);
    assert.match(notice, /server-only secret/i);
    assert.match(notice, /15 minutes/i);
    assert.match(notice, /does not extend|do not extend/i);
    assert.match(notice, /does not (offer or )?store a product-update signup/i);
  }
});
