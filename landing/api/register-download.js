// Edge function for a privacy-minimized anonymous download counter.
// v1.0.4 deliberately accepts no contact or marketing data: an email-ownership
// verification flow and its legal/operational approvals do not exist yet.
export const config = { runtime: 'edge' };

const KV_URL = process.env.KV_REST_API_URL || process.env.UPSTASH_REDIS_REST_URL || '';
const KV_TOKEN = process.env.KV_REST_API_TOKEN || process.env.UPSTASH_REDIS_REST_TOKEN || '';
const RATE_LIMIT_HMAC_SECRET = process.env.DOWNLOAD_RATE_LIMIT_HMAC_SECRET || '';
const MAX_BODY_BYTES = 8 * 1024;
const RATE_WINDOW_SECONDS = 15 * 60;
const RATE_LIMIT = 5;
const MIN_HMAC_SECRET_BYTES = 32;
const ALLOWED_FIELDS = new Set(['website']);
const textEncoder = new TextEncoder();
const RATE_LIMIT_SCRIPT = [
  "local count = redis.call('INCR', KEYS[1])",
  "if count == 1 then redis.call('EXPIRE', KEYS[1], ARGV[1]) end",
  "return {count, redis.call('TTL', KEYS[1])}",
].join('\n');

function json(obj, status = 200, extraHeaders = {}) {
  return new Response(JSON.stringify(obj), {
    status,
    headers: {
      'Content-Type': 'application/json; charset=utf-8',
      'Cache-Control': 'no-store',
      'X-Content-Type-Options': 'nosniff',
      ...extraHeaders,
    },
  });
}

async function kvPipeline(commands) {
  if (!KV_URL || !KV_TOKEN) return { configured: false, results: [] };
  const response = await fetch(`${KV_URL}/pipeline`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${KV_TOKEN}`, 'Content-Type': 'application/json' },
    body: JSON.stringify(commands),
  });
  if (!response.ok) throw new Error(`KV request failed (${response.status})`);
  const payload = await response.json();
  return {
    configured: true,
    results: Array.isArray(payload) ? payload.map((entry) => entry && entry.result) : [],
  };
}

function kvConfiguration() {
  if (!KV_URL && !KV_TOKEN) return 'disabled';
  if (!KV_URL || !KV_TOKEN) return 'invalid';
  if (textEncoder.encode(RATE_LIMIT_HMAC_SECRET).byteLength < MIN_HMAC_SECRET_BYTES) {
    return 'invalid';
  }
  return 'configured';
}

async function hmacSha256Hex(value) {
  const key = await crypto.subtle.importKey(
    'raw',
    textEncoder.encode(RATE_LIMIT_HMAC_SECRET),
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign'],
  );
  const signature = await crypto.subtle.sign('HMAC', key, textEncoder.encode(value));
  return Array.from(
    new Uint8Array(signature),
    (byte) => byte.toString(16).padStart(2, '0'),
  ).join('');
}

async function enforceRateLimit(req) {
  if (kvConfiguration() === 'disabled') {
    return { allowed: true, configured: false, retryAfter: RATE_WINDOW_SECONDS };
  }
  const source =
    req.headers.get('x-vercel-forwarded-for') ||
    req.headers.get('x-forwarded-for') ||
    req.headers.get('x-real-ip') ||
    'unknown';
  const firstAddress = source.split(',')[0].trim().slice(0, 128);
  const addressToken = await hmacSha256Hex(`register-download:v2:${firstAddress}`);
  const key = `downloads:rate:v2:${addressToken}`;
  // One atomic Redis script starts the TTL only when INCR creates the counter.
  // Later requests cannot extend the fixed window.
  const result = await kvPipeline([
    ['EVAL', RATE_LIMIT_SCRIPT, 1, key, String(RATE_WINDOW_SECONDS)],
  ]);
  const [rawCount, rawTtl] = Array.isArray(result.results[0]) ? result.results[0] : [];
  const count = Number(rawCount);
  const ttl = Number(rawTtl);
  if (
    !Number.isSafeInteger(count) ||
    count < 1 ||
    !Number.isSafeInteger(ttl) ||
    ttl < 1 ||
    ttl > RATE_WINDOW_SECONDS
  ) {
    throw new Error('Invalid rate-limit response');
  }
  return { allowed: count <= RATE_LIMIT, configured: true, retryAfter: ttl };
}

export default async function handler(req) {
  if (req.method !== 'POST') return json({ error: 'Method not allowed.' }, 405, { Allow: 'POST' });

  const contentType = (req.headers.get('content-type') || '').toLowerCase();
  if (!contentType.startsWith('application/json')) {
    return json({ error: 'Content-Type must be application/json.' }, 415);
  }

  let raw;
  try {
    raw = await req.text();
  } catch {
    return json({ error: 'Invalid request body.' }, 400);
  }
  if (new TextEncoder().encode(raw).byteLength > MAX_BODY_BYTES) {
    return json({ error: 'Request body is too large.' }, 413);
  }

  let body;
  try {
    body = JSON.parse(raw);
  } catch {
    return json({ error: 'Invalid request body.' }, 400);
  }
  if (!body || typeof body !== 'object' || Array.isArray(body)) {
    return json({ error: 'Invalid request body.' }, 400);
  }
  if (Object.keys(body).some((key) => !ALLOWED_FIELDS.has(key))) {
    return json({ error: 'Request contains unsupported fields.' }, 422);
  }

  // Honeypot: acknowledge silently so automated submitters receive no useful signal.
  if (typeof body.website === 'string' && body.website.trim()) {
    return json({ ok: true, stored: false });
  }

  if (kvConfiguration() === 'invalid') {
    // Never fall back to an unhashed or unkeyed IP-derived identifier. The web
    // client starts the download independently of this best-effort counter.
    return json(
      { error: 'Download registration is temporarily unavailable.', stored: false, counted: false },
      503,
      { 'Retry-After': '60' },
    );
  }

  try {
    const rate = await enforceRateLimit(req);
    if (!rate.allowed) {
      return json(
        { error: 'Too many requests. Please try again later.' },
        429,
        { 'Retry-After': String(rate.retryAfter) },
      );
    }

    const counter = await kvPipeline([['INCR', 'downloads:count']]);
    return json({ ok: true, stored: false, counted: counter.configured });
  } catch (error) {
    // Do not expose infrastructure details or silently bypass the privacy-safe
    // limiter. The web client still starts the installer download in `finally`.
    console.error('[register-download] privacy-minimized registration failed');
    return json(
      { error: 'Download registration is temporarily unavailable.', stored: false, counted: false },
      503,
      { 'Retry-After': '60' },
    );
  }
}
