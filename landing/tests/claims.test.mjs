import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

/**
 * What the public pages are allowed to say.
 *
 * This file replaces `whats-new.test.mjs`, which guarded a per-release
 * changelog that used to sit on the homepage between the product story and
 * pricing. That section is gone (ADR-0074). Release history belongs on a
 * destination of its own — the pattern every comparable product follows:
 * Anthropic keeps it on docs and support subdomains, Google Antigravity on
 * `/changelog` and `/releases`, neither on the marketing page. Mityu's is
 * generated per release on GitHub, which is also where the download endpoint
 * resolves, so the footer links there rather than duplicating it by hand.
 *
 * Three rules survive, because they are about truth rather than layout:
 *
 * - the copilot is described the way the product ships it — a beta, off by
 *   default, reached from Settings → Beta, on-device in this release — on both
 *   public pages, and the desktop About dialog uses the same load-bearing
 *   phrases (`frontend/src/components/About.test.tsx` asserts that side);
 * - ADR-0038 invariant 1 plus the two open gates: no "undetectable", no
 *   latency figure before I9, no accuracy figure before A5;
 * - no version number is hard-coded into the marketing copy. That is what let
 *   the page sit on "Mityu 1.0 is available now" two minor releases after it
 *   stopped being true.
 */

const index = await readFile(new URL('../index.html', import.meta.url), 'utf8');
const privacy = await readFile(new URL('../privacy.html', import.meta.url), 'utf8');

test('the copilot is described as a beta that is off by default, reached from Settings → Beta', () => {
  for (const [name, text] of [['index.html', index], ['privacy.html', privacy]]) {
    assert.match(text, /off by default/i, `${name} must say the copilot is off by default`);
    assert.match(text, /Settings → Beta/, `${name} must say where the switch is`);
    assert.match(text, /beta/i, `${name} must call it a beta`);
  }
  assert.match(index, /not yet been checked against a real model/i);
  assert.match(index, /cites the transcript segment it came from, or is refused/i);
  assert.match(privacy, /last few minutes of transcript text/i);
  // The shipped build DOES expose the cloud-egress setting: `CopilotSettings.tsx`
  // renders an "Allow cloud insights" switch, `BetaSettings.tsx` mounts it, and
  // Settings -> Beta reaches it. The assertions that used to live here pinned the
  // opposite ("offers no way to turn that setting on"). That was true when it was
  // written against v1.2.1 and was falsified by the copilot settings shipping in
  // the next release -- and because a test required the sentence, CI stayed green
  // BECAUSE the privacy notice was wrong. A page that under-states egress is the
  // worst thing this file can let through, so it now pins the shape of the truth
  // instead of one sentence: name a switch the reader can actually find, say it is
  // off by default, say what turning it on costs -- and forbid, by name, the claim
  // that the switch does not exist.
  assert.match(privacy, /separate setting[\s\S]{0,60}Allow cloud insights/i);
  assert.match(privacy, /off by default/i);
  for (const [name, text] of [['index.html', index], ['privacy.html', privacy]]) {
    assert.match(text, /Allow cloud insights/, `${name} must name the cloud switch a reader can find`);
    assert.match(text, /on every (press of an )?action/i, `${name} must say what turning the switch on does`);
    assert.doesNotMatch(text, /no way to turn that setting on/i, `${name}: that switch IS reachable in the shipped build`);
    assert.doesNotMatch(text, /works only with a model running on your device/i, `${name}: egress is conditional on a setting, not impossible`);
  }
});

test('no page claims what the I9 and A5 gates have not measured', () => {
  for (const [name, text] of [['index.html', index], ['privacy.html', privacy]]) {
    assert.doesNotMatch(text, /undetectable/i, `${name}: "undetectable" never appears in product copy (ADR-0038)`);
    assert.doesNotMatch(text, /\b\d+(\.\d+)?\s?(ms|milliseconds|seconds? latency)\b/i, `${name}: no latency figure before I9`);
    assert.doesNotMatch(text, /\b\d{2,3}\s?%\s?(accura|WER|word error)/i, `${name}: no accuracy figure before A5`);
  }
});

test('the marketing page hard-codes no version, so it cannot go stale', () => {
  assert.doesNotMatch(index, /Mityu 1\.0 is available now/);
  // A literal x.y.z anywhere in the page is a promise that rots at the next
  // release. The only version a visitor needs is the one the installer serves,
  // and that is resolved at request time by api/download.js.
  assert.doesNotMatch(
    index,
    /\b\d+\.\d+\.\d+\b/,
    'a hard-coded version in marketing copy will go stale — let the download endpoint name it',
  );
});

test('release history is linked, not reproduced, and the page keeps no changelog', () => {
  assert.match(
    index,
    /https:\/\/github\.com\/aydogandagidir\/mityu\/releases/,
    'the footer must point at where release notes are actually published',
  );
  // The homepage is a product story. A changelog section, a roadmap list, or a
  // column of version badges belongs elsewhere — this is the regression guard
  // for the section ADR-0074 removed.
  assert.doesNotMatch(index, /data-release=/, 'no per-release changelog markup on the homepage');
  assert.doesNotMatch(index, /id="whats-new"/, 'no what-is-new section on the homepage');
  // Narrow on purpose. "macOS in development" is a platform fact a buyer needs
  // and appears four times legitimately; what must not come back is the roadmap
  // CARD — its badge and its heading. A first draft of this assertion matched
  // the bare phrase and failed on the honest copy.
  assert.doesNotMatch(index, />\s*In development\s*</, 'no roadmap card on the marketing page');
  assert.doesNotMatch(index, /Being built/i, 'no roadmap card on the marketing page');
});
