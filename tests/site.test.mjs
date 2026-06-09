import { access, readFile } from 'node:fs/promises';
import { test } from 'node:test';
import assert from 'node:assert/strict';

const html = await readFile(new URL('../site/index.html', import.meta.url), 'utf8');
const css = await readFile(new URL('../site/styles.css', import.meta.url), 'utf8');
const headers = await readFile(new URL('../site/_headers', import.meta.url), 'utf8');
const playHtml = await readFile(new URL('../site/play/index.html', import.meta.url), 'utf8');
const playJs = await readFile(new URL('../site/play/play.js', import.meta.url), 'utf8');
const wasmRuntimeJs = new URL('../site/play/pkg/vibe_gba.js', import.meta.url);
const wasmRuntimeBinary = new URL('../site/play/pkg/vibe_gba_bg.wasm', import.meta.url);

async function exists(url) {
  try { await access(url); return true; } catch { return false; }
}

test('public site states the real project boundary', () => {
  assert.match(html, /vibe-gba/i);
  assert.match(html, /Game Boy Advance emulator prototype/i);
  assert.match(html, /does not include ROMs/i);
  assert.match(html, /legal ROM/i);
});

test('public site links to repository', () => {
  assert.match(html, /https:\/\/github\.com\/lizliz404\/vibe-gba/);
  assert.match(html, /docs\/screenshots\/littleroot-entry\.png/);
});

test('stylesheet includes game body layout', () => {
  assert.match(css, /\.game-body/);
  assert.match(css, /#game-screen/);
  assert.match(css, /\.start-button/);
  assert.match(css, /#dialogue-box/);
  assert.match(css, /#battle-box/);
  assert.match(css, /@media/);
});

test('cloudflare pages headers set safe defaults', () => {
  assert.match(headers, /X-Content-Type-Options:\s*nosniff/);
  assert.match(headers, /Referrer-Policy:\s*strict-origin-when-cross-origin/);
});

test('play page is a self-contained game, not a ROM upload shell', () => {
  assert.match(playHtml, /START/);
  assert.match(playHtml, /id="game-screen"/);
  assert.match(playHtml, /width="480"/);
  assert.match(playHtml, /height="360"/);
  assert.match(playHtml, /play\/play\.js/);
  assert.doesNotMatch(playHtml, /id="rom-input"/);
  assert.doesNotMatch(playHtml, /Choose ROM/);
});

test('game script is a self-contained tile adventure: maps, movement, NPCs, battle', () => {
  assert.match(playJs, /MAP_TRUCK/);
  assert.match(playJs, /MAP_LITTLEROOT/);
  assert.match(playJs, /MAP_ROUTE101/);
  assert.match(playJs, /updateMovement/);
  assert.match(playJs, /interactNPC/);
  assert.match(playJs, /startBattle/);
  assert.match(playJs, /battleFight/);
  assert.match(playJs, /checkDoor/);
  assert.match(playJs, /drawTile/);
  assert.match(playJs, /drawPlayer/);
  assert.match(playJs, /requestAnimationFrame/);
  assert.match(playJs, /keydown/);
  assert.doesNotMatch(playJs, /WebGba/);
  assert.doesNotMatch(playJs, /FileReader/);
  assert.doesNotMatch(playJs, /localStorage/);
  assert.doesNotMatch(playJs, /new WebGba/);
});

test('wasm runtime artifacts are preserved for desktop build', async () => {
  assert.equal(await exists(wasmRuntimeJs), true);
  assert.equal(await exists(wasmRuntimeBinary), true);
});
