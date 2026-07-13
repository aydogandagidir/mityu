// Edge function: record who downloads Mityu (lead capture) before the download
// starts. Each POST appends a lead record to a Redis list and increments a
// counter in Vercel KV (Upstash) via its REST API — no npm dependency needed.
//
// Storage is best-effort: if KV isn't provisioned yet (or a write hiccups), we
// STILL return ok so the front-end never blocks the user's download on logging.
export const config = { runtime: 'edge' };

// Vercel KV sets KV_REST_API_*, a direct Upstash integration sets UPSTASH_*.
const KV_URL = process.env.KV_REST_API_URL || process.env.UPSTASH_REDIS_REST_URL || '';
const KV_TOKEN = process.env.KV_REST_API_TOKEN || process.env.UPSTASH_REDIS_REST_TOKEN || '';

function json(obj, status = 200) {
  return new Response(JSON.stringify(obj), {
    status,
    headers: { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' },
  });
}

async function kvPipeline(commands) {
  if (!KV_URL || !KV_TOKEN) return { configured: false };
  const res = await fetch(`${KV_URL}/pipeline`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${KV_TOKEN}`, 'Content-Type': 'application/json' },
    body: JSON.stringify(commands),
  });
  if (!res.ok) throw new Error(`KV ${res.status}`);
  await res.json();
  return { configured: true };
}

const EMAIL_RE = /^[^@\s]+@[^@\s]+\.[^@\s]+$/;

export default async function handler(req) {
  if (req.method !== 'POST') return json({ error: 'Method not allowed' }, 405);

  let body;
  try {
    body = await req.json();
  } catch {
    return json({ error: 'Invalid request body.' }, 400);
  }

  const email = String(body.email || '').trim().toLowerCase();
  const company = String(body.company || '').trim();
  const name = String(body.name || '').trim();
  const consent = body.consent === true;

  if (!EMAIL_RE.test(email)) return json({ error: 'A valid email is required.' }, 422);
  if (!company) return json({ error: 'Company is required.' }, 422);
  if (!consent) return json({ error: 'Please accept the privacy policy.' }, 422);

  const record = {
    email,
    company,
    name,
    at: new Date().toISOString(),
    country: req.headers.get('x-vercel-ip-country') || '',
    ua: (req.headers.get('user-agent') || '').slice(0, 300),
    ref: (req.headers.get('referer') || '').slice(0, 300),
  };

  let stored = false;
  try {
    const r = await kvPipeline([
      ['LPUSH', 'downloads:leads', JSON.stringify(record)],
      ['INCR', 'downloads:count'],
      ['SADD', 'downloads:emails', email],
    ]);
    stored = r.configured;
  } catch (e) {
    console.error('[register-download] KV write failed (download not blocked):', e);
  }

  return json({ ok: true, stored });
}
