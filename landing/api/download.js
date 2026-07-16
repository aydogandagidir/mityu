// Edge function: stream the latest signed Windows installer from mityu.bluedev.dev
// itself, so a visitor downloads directly from our domain — no GitHub account, no
// GitHub page, and no GitHub URL is ever exposed to the browser.
//
// The version is resolved per request from `latest.json`, the updater manifest CI
// publishes with every release, and the installer is served under its real,
// VERSIONED name (e.g. Mityu_1.0.4_x64-setup.exe) so a visitor always sees which
// version they downloaded. Two consequences worth keeping:
//   1. Nothing here needs editing per release — a new release is picked up as soon
//      as GitHub's `releases/latest` moves.
//   2. No hand-uploaded, unversioned `Mityu-Setup.exe` alias is required. That
//      alias used to be copied into each release by hand; forgetting it silently
//      broke this endpoint (502) the moment a release without it became Latest.
export const config = { runtime: 'edge' };

const REPO = 'aydogandagidir/mityu';
const MANIFEST_URL = `https://github.com/${REPO}/releases/latest/download/latest.json`;

// The installer may only ever come from this repo's own release downloads. The
// manifest is ours, but this endpoint proxies whatever URL it names, so pin the
// origin rather than trusting the document.
const ALLOWED_ASSET_PREFIX = `https://github.com/${REPO}/releases/download/`;

// Enforce the release naming architecture: every version is served as
// Mityu_<x.y.z>_x64-setup.exe. Also keeps an unexpected value out of the
// Content-Disposition header.
const INSTALLER_NAME = /^Mityu_\d+\.\d+\.\d+_x64-setup\.exe$/;

function unavailable() {
  return new Response('Download is temporarily unavailable. Please try again.', {
    status: 502,
    headers: { 'Content-Type': 'text/plain; charset=utf-8', 'Cache-Control': 'no-store' },
  });
}

export default async function handler() {
  // 1. Resolve the current release from the manifest CI publishes.
  let assetUrl;
  try {
    const manifestResponse = await fetch(MANIFEST_URL, { redirect: 'follow' });
    if (!manifestResponse.ok) return unavailable();
    const manifest = await manifestResponse.json();
    assetUrl = manifest?.platforms?.['windows-x86_64']?.url;
  } catch {
    return unavailable();
  }

  if (typeof assetUrl !== 'string' || !assetUrl.startsWith(ALLOWED_ASSET_PREFIX)) {
    return unavailable();
  }

  let fileName;
  try {
    fileName = decodeURIComponent(assetUrl.split('/').pop() || '');
  } catch {
    return unavailable();
  }
  if (!INSTALLER_NAME.test(fileName)) return unavailable();

  // 2. Stream the versioned installer back under its real name.
  let upstream;
  try {
    upstream = await fetch(assetUrl, { redirect: 'follow' });
  } catch {
    return unavailable();
  }
  if (!upstream.ok || !upstream.body) return unavailable();

  const headers = new Headers();
  headers.set('Content-Type', 'application/octet-stream');
  headers.set('Content-Disposition', `attachment; filename="${fileName}"`);
  const len = upstream.headers.get('content-length');
  if (len) headers.set('Content-Length', len);
  // Short cache so a new release is picked up quickly, but repeat clicks are cheap.
  headers.set('Cache-Control', 'public, max-age=300');

  return new Response(upstream.body, { status: 200, headers });
}
