// Edge function: stream the latest signed Windows installer from mityu.bluedev.dev
// itself, so a visitor downloads directly from our domain — no GitHub account, no
// GitHub page, and no GitHub URL is ever exposed to the browser. We fetch the
// GitHub release asset server-side (following GitHub's CDN redirect) and pipe the
// bytes straight back as an attachment.
export const config = { runtime: 'edge' };

const UPSTREAM =
  'https://github.com/aydogandagidir/mityu/releases/latest/download/Mityu-Setup.exe';

export default async function handler() {
  const upstream = await fetch(UPSTREAM, { redirect: 'follow' });

  if (!upstream.ok || !upstream.body) {
    return new Response('Download is temporarily unavailable. Please try again.', {
      status: 502,
      headers: { 'Content-Type': 'text/plain; charset=utf-8', 'Cache-Control': 'no-store' },
    });
  }

  const headers = new Headers();
  headers.set('Content-Type', 'application/octet-stream');
  headers.set('Content-Disposition', 'attachment; filename="Mityu-Setup.exe"');
  const len = upstream.headers.get('content-length');
  if (len) headers.set('Content-Length', len);
  // Short cache so a new release is picked up quickly, but repeat clicks are cheap.
  headers.set('Cache-Control', 'public, max-age=300');

  return new Response(upstream.body, { status: 200, headers });
}
