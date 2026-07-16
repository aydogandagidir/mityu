import assert from 'node:assert/strict';
import test from 'node:test';

const MANIFEST_URL = 'https://github.com/aydogandagidir/mityu/releases/latest/download/latest.json';
const ASSET_104 =
  'https://github.com/aydogandagidir/mityu/releases/download/v1.0.4/Mityu_1.0.4_x64-setup.exe';

const realFetch = globalThis.fetch;

function manifestFor(url) {
  return new Response(JSON.stringify({ version: '1.0.4', platforms: { 'windows-x86_64': { url } } }), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}

function installerBody() {
  return new Response('MZ-installer-bytes', { status: 200, headers: { 'content-length': '18' } });
}

/** Route fetches by URL; anything unrouted 404s. Returns the call log. */
function stubFetch(routes) {
  const calls = [];
  globalThis.fetch = async (input) => {
    const url = typeof input === 'string' ? input : input.url;
    calls.push(url);
    const route = routes[url];
    if (!route) return new Response('unrouted', { status: 404 });
    return typeof route === 'function' ? route() : route();
  };
  return calls;
}

async function loadHandler(tag) {
  const { default: handler } = await import(`../api/download.js?${tag}`);
  return handler;
}

test.afterEach(() => {
  globalThis.fetch = realFetch;
});

test('serves the installer under its real versioned name, resolved from latest.json', async () => {
  const calls = stubFetch({
    [MANIFEST_URL]: () => manifestFor(ASSET_104),
    [ASSET_104]: installerBody,
  });
  const handler = await loadHandler('happy');
  const response = await handler();

  assert.equal(response.status, 200);
  assert.equal(
    response.headers.get('content-disposition'),
    'attachment; filename="Mityu_1.0.4_x64-setup.exe"',
    'the visitor must see the version in the downloaded file name',
  );
  assert.equal(response.headers.get('content-type'), 'application/octet-stream');
  assert.equal(response.headers.get('content-length'), '18');
  assert.equal(await response.text(), 'MZ-installer-bytes');
  // Resolved from the manifest, then streamed the versioned asset — no hand-maintained alias.
  assert.deepEqual(calls, [MANIFEST_URL, ASSET_104]);
});

test('a new release is picked up with no code change', async () => {
  const asset105 =
    'https://github.com/aydogandagidir/mityu/releases/download/v1.0.5/Mityu_1.0.5_x64-setup.exe';
  stubFetch({ [MANIFEST_URL]: () => manifestFor(asset105), [asset105]: installerBody });
  const handler = await loadHandler('next-version');
  const response = await handler();

  assert.equal(response.status, 200);
  assert.equal(
    response.headers.get('content-disposition'),
    'attachment; filename="Mityu_1.0.5_x64-setup.exe"',
  );
});

test('refuses to proxy an asset outside this repository (no open proxy)', async () => {
  stubFetch({
    [MANIFEST_URL]: () =>
      manifestFor('https://evil.example.com/releases/download/v1.0.4/Mityu_1.0.4_x64-setup.exe'),
  });
  const handler = await loadHandler('foreign-origin');
  const response = await handler();
  assert.equal(response.status, 502);
});

test('refuses a manifest url whose file name breaks the release naming architecture', async () => {
  const odd = 'https://github.com/aydogandagidir/mityu/releases/download/v1.0.4/Mityu-Setup.exe';
  stubFetch({ [MANIFEST_URL]: () => manifestFor(odd), [odd]: installerBody });
  const handler = await loadHandler('bad-name');
  const response = await handler();
  assert.equal(response.status, 502, 'an unversioned name must never be served');
});

test('fails closed when the manifest is missing, malformed or unreachable', async () => {
  for (const [tag, routes] of [
    ['manifest-404', { [MANIFEST_URL]: () => new Response('nope', { status: 404 }) }],
    ['manifest-garbage', { [MANIFEST_URL]: () => new Response('not json', { status: 200 }) }],
    ['manifest-no-platform', { [MANIFEST_URL]: () => new Response('{}', { status: 200 }) }],
    [
      'manifest-throws',
      {
        [MANIFEST_URL]: () => {
          throw new Error('network down');
        },
      },
    ],
  ]) {
    stubFetch(routes);
    const handler = await loadHandler(tag);
    const response = await handler();
    assert.equal(response.status, 502, `${tag} must fail closed`);
    assert.equal(response.headers.get('cache-control'), 'no-store');
  }
});

test('fails closed when the installer asset itself is unavailable', async () => {
  stubFetch({
    [MANIFEST_URL]: () => manifestFor(ASSET_104),
    [ASSET_104]: () => new Response('gone', { status: 404 }),
  });
  const handler = await loadHandler('asset-404');
  const response = await handler();
  assert.equal(response.status, 502);
});
