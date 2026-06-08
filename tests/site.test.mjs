import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import assert from 'node:assert/strict';

const html = await readFile(new URL('../site/index.html', import.meta.url), 'utf8');
const css = await readFile(new URL('../site/styles.css', import.meta.url), 'utf8');

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
