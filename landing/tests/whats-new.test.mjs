import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

/**
 * The landing page now carries a "What's new / In development" section and a
 * copilot FAQ. These tests protect the claims that section makes, in the same
 * spirit as the privacy-notice tests next door:
 *
 * - it never advertises a version newer than the one the app build carries
 *   (`frontend/package.json` is the canonical version source per
 *   RELEASE_CHECKLIST §0), because `main` publishes to Vercel on merge and a
 *   listed release nobody can download is the failure PR #35 held merge for;
 * - the copilot is described the way the product ships it — a beta, off by
 *   default, reached from Settings → Beta — on both the page and the privacy
 *   notice, and the desktop About dialog uses the same load-bearing phrases
 *   (`frontend/src/components/About.test.tsx` asserts that side);
 * - ADR-0038 invariant 1: the word "undetectable" never appears in product
 *   copy or marketing.
 */

const index = await readFile(new URL('../index.html', import.meta.url), 'utf8');
const privacy = await readFile(new URL('../privacy.html', import.meta.url), 'utf8');
const pkg = JSON.parse(await readFile(new URL('../../frontend/package.json', import.meta.url), 'utf8'));

const SEMVER = /^(\d+)\.(\d+)\.(\d+)$/;

function parse(version) {
  const m = SEMVER.exec(version);
  assert.ok(m, `not a three-part version: ${version}`);
  return m.slice(1).map(Number);
}

function compare(a, b) {
  const [pa, pb] = [parse(a), parse(b)];
  for (let i = 0; i < 3; i += 1) {
    if (pa[i] !== pb[i]) return pa[i] - pb[i];
  }
  return 0;
}

function listedReleases() {
  return [...index.matchAll(/data-release="([^"]+)"/g)].map((m) => m[1]);
}

test('the what\'s-new list exists, is newest-first, and names only released versions', () => {
  const releases = listedReleases();
  assert.ok(releases.length >= 2, 'expected at least two release entries');
  for (let i = 1; i < releases.length; i += 1) {
    assert.ok(
      compare(releases[i - 1], releases[i]) > 0,
      `release entries must be newest-first: ${releases[i - 1]} then ${releases[i]}`,
    );
  }
  assert.ok(
    compare(releases[0], pkg.version) <= 0,
    `the landing lists ${releases[0]} but the app build is ${pkg.version} — never advertise a release before it exists`,
  );
});

test('the newest listed release is on the same release line as the app version', () => {
  // The list may LAG the app version (a bump lands on main before its signed
  // release exists, and merging publishes the site), but it may never lead it,
  // and it must not fall a whole minor behind — that is the staleness this PR
  // fixed, where the page still said "Mityu 1.0" two minors later.
  const [newest] = listedReleases();
  const [major, minor] = parse(newest);
  const [appMajor, appMinor] = parse(pkg.version);
  assert.equal(major, appMajor, `landing lists ${newest}, app is ${pkg.version}`);
  assert.ok(appMinor - minor <= 1, `landing lists ${newest}, app is ${pkg.version} — the what's-new list is a minor behind`);
});

test('the copilot is described as a beta that is off by default, reached from Settings → Beta', () => {
  for (const [name, text] of [['index.html', index], ['privacy.html', privacy]]) {
    assert.match(text, /off by default/i, `${name} must say the copilot is off by default`);
    assert.match(text, /Settings → Beta/, `${name} must say where the switch is`);
    assert.match(text, /beta/i, `${name} must call it a beta`);
  }
  assert.match(index, /not yet been checked against a real model/i);
  assert.match(index, /cites the transcript segment it came from, or is refused/i);
  assert.match(privacy, /last few minutes of transcript text/i);
  assert.match(privacy, /separate, explicit switch/i);
});

test('no page claims what the I9 gate has not measured, and ADR-0038 wording rules hold', () => {
  for (const [name, text] of [['index.html', index], ['privacy.html', privacy]]) {
    assert.doesNotMatch(text, /undetectable/i, `${name}: "undetectable" never appears in product copy (ADR-0038)`);
    assert.doesNotMatch(text, /\b\d+(\.\d+)?\s?(ms|milliseconds|seconds? latency)\b/i, `${name}: no latency figure before I9`);
    assert.doesNotMatch(text, /\b\d{2,3}\s?%\s?(accura|WER|word error)/i, `${name}: no accuracy figure before A5`);
  }
});

test('the stale "Mityu 1.0 is available now" claim is gone and the version lives in one place', () => {
  assert.doesNotMatch(index, /Mityu 1\.0 is available now/);
  // Outside the data-release list, no literal x.y.z version is hard-coded into
  // prose, so a release bump has exactly one landing edit to make.
  const prose = index.replace(/data-release="[^"]+"/g, '').replace(/<span class="ver">[^<]*<\/span>/g, '');
  assert.doesNotMatch(prose, /\b1\.\d+\.\d+\b/, 'a literal version outside the release list will go stale');
});
