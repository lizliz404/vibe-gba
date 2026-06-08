import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import assert from 'node:assert/strict';

const html = await readFile(new URL('../site/index.html', import.meta.url), 'utf8');
const css = await readFile(new URL('../site/styles.css', import.meta.url), 'utf8');
const headers = await readFile(new URL('../site/_headers', import.meta.url), 'utf8');
const playHtml = await readFile(new URL('../site/play/index.html', import.meta.url), 'utf8');
const playJs = await readFile(new URL('../site/play/play.js', import.meta.url), 'utf8');

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

test('play page is a browser-playable emulator entry shell', () => {
  assert.match(playHtml, /id="rom-input"/);
  assert.match(playHtml, /accept="\.gba,\.zip"/);
  assert.match(playHtml, /id="gba-screen"/);
  assert.match(playHtml, /width="240"/);
  assert.match(playHtml, /height="160"/);
  assert.match(playHtml, /ROM never leaves your browser/i);
  assert.match(playHtml, /data-objective="moving-truck"/);
  assert.match(playHtml, /data-objective="starter-acquired"/);
  assert.match(playHtml, /play\/play\.js/);
});

test('play page script wires local ROM loading and GBA controls without upload', () => {
  assert.match(playJs, /FileReader/);
  assert.match(playJs, /localStorage/);
  assert.match(playJs, /keydown/);
  assert.match(playJs, /keyup/);
  assert.match(playJs, /ArrowUp/);
  assert.match(playJs, /KeyZ/);
  assert.match(playJs, /requestAnimationFrame/);
  assert.match(playJs, /putImageData/);
  assert.match(playJs, /\.\/pkg\/vibe_gba\.js/);
  assert.match(playJs, /new WebGba/);
  assert.doesNotMatch(playJs, /WASM core: pending/);
  assert.doesNotMatch(playJs, /fetch\s*\(/);
  assert.doesNotMatch(playJs, /XMLHttpRequest/);
});
