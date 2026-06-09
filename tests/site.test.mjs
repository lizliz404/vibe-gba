import { access, readFile } from 'node:fs/promises';
import { test } from 'node:test';
import assert from 'node:assert/strict';

const html = await readFile(new URL('../site/index.html', import.meta.url), 'utf8');
const css = await readFile(new URL('../site/styles.css', import.meta.url), 'utf8');
const headers = await readFile(new URL('../site/_headers', import.meta.url), 'utf8');
const playHtml = await readFile(new URL('../site/play/index.html', import.meta.url), 'utf8');
const playJs = await readFile(new URL('../site/play/play.js', import.meta.url), 'utf8');
const wasmPkgGitignore = new URL('../site/play/pkg/.gitignore', import.meta.url);
const wasmRuntimeJs = new URL('../site/play/pkg/vibe_gba.js', import.meta.url);
const wasmRuntimeBinary = new URL('../site/play/pkg/vibe_gba_bg.wasm', import.meta.url);

async function exists(url) {
  try {
    await access(url);
    return true;
  } catch {
    return false;
  }
}

test('public site states the real project boundary', () => {
  assert.match(html, /vibe-gba/i);
  assert.match(html, /Game Boy Advance emulator prototype/i);
  assert.match(html, /does not include ROMs/i);
  assert.match(html, /legal ROM/i);
  assert.match(html, /Emerald education objective mode/i);
});

test('public site exposes tester-facing next steps', () => {
  assert.match(html, /Tester loop/i);
  assert.match(html, /What broke/i);
  assert.match(html, /What you expected/i);
  assert.match(html, /What evidence you captured/i);
  assert.match(html, /test-driven development/i);
});

test('public site links to the repository and preserves screenshots', () => {
  assert.match(html, /https:\/\/github\.com\/lizliz404\/vibe-gba/);
  assert.match(html, /docs\/screenshots\/littleroot-entry\.png/);
  assert.match(html, /docs\/screenshots\/littleroot-walk\.png/);
});

test('stylesheet exists and includes responsive layout hooks', () => {
  assert.match(css, /\.hero/);
  assert.match(css, /\.card/);
  assert.match(css, /@media/);
});

test('cloudflare pages headers set safe defaults', () => {
  assert.match(headers, /X-Content-Type-Options:\s*nosniff/);
  assert.match(headers, /Referrer-Policy:\s*strict-origin-when-cross-origin/);
  assert.match(headers, /Cache-Control:\s*public, max-age=604800/);
});

test('play page is an in-browser GBA emulator with a ROM file picker, no ROMs included', () => {
  assert.match(playHtml, /vibe-gba/i);
  assert.match(playHtml, /id="rom-input"/);
  assert.match(playHtml, /id="game-screen"/);
  assert.match(playHtml, /width="240"/);
  assert.match(playHtml, /height="160"/);
  assert.match(playHtml, /bring your own legal ROM/i);
  assert.match(playHtml, /play\/play\.js/);
  assert.doesNotMatch(playHtml, /Emerald Dash/);
  assert.doesNotMatch(playHtml, /id="start-button"/);
});

test('play page script wires the WASM emulator: imports the pkg, creates WebGba, renders frames', () => {
  assert.match(playJs, /pkg\/vibe_gba\.js/);
  assert.match(playJs, /new WebGba/);
  assert.match(playJs, /requestAnimationFrame/);
  assert.match(playJs, /putImageData/);
  assert.match(playJs, /run_frame/);
  assert.match(playJs, /set_button/);
  assert.match(playJs, /keydown/);
  assert.match(playJs, /keyup/);
  assert.match(playJs, /ArrowUp/);
  assert.match(playJs, /romInput/);
  assert.match(playJs, /arrayBuffer/);
  assert.doesNotMatch(playJs, /localStorage/);
  assert.doesNotMatch(playJs, /fetch\s*\(/);
  assert.doesNotMatch(playJs, /XMLHttpRequest/);
});

test('wasm runtime artifacts are present and not hidden by pkg gitignore', async () => {
  assert.equal(await exists(wasmRuntimeJs), true);
  assert.equal(await exists(wasmRuntimeBinary), true);
  assert.equal(await exists(wasmPkgGitignore), false);
});
