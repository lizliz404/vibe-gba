import init, { WebGba } from './pkg/vibe_gba.js';

const canvas = document.getElementById('game-screen');
const ctx = canvas.getContext('2d');
const romInput = document.getElementById('rom-input');
const statusLine = document.getElementById('game-status');

const WIDTH = 240;
const HEIGHT = 160;

let gba = null;
let running = false;
let rafId = null;
const keys = new Set();

const KEY_BUTTON = {
  KeyZ: 0, KeyA: 0,
  KeyX: 1, KeyS: 1,
  Backspace: 2,
  Enter: 3,
  ArrowRight: 4,
  ArrowLeft: 5,
  ArrowUp: 6,
  ArrowDown: 7,
  KeyE: 8,
  KeyQ: 9,
};

async function loadRom() {
  const file = romInput.files[0];
  if (!file) {
    statusLine.textContent = 'Select a .gba ROM file to begin.';
    return;
  }

  statusLine.textContent = 'Loading ROM...';
  await init();

  const buffer = await file.arrayBuffer();
  const rom = new Uint8Array(buffer);

  try {
    gba = new WebGba(rom);
    running = true;
    statusLine.textContent = 'Running — Arrows/WASD move, Z=A, X=B, Enter=Start, Esc=pause.';
    rafId = requestAnimationFrame(emulatorLoop);
  } catch (err) {
    statusLine.textContent = `Failed to start: ${err.message}`;
  }
}

function emulatorLoop() {
  if (!running || !gba) return;

  applyInput();
  const rgba = gba.run_frame();

  const imageData = new ImageData(new Uint8ClampedArray(rgba), WIDTH, HEIGHT);
  ctx.putImageData(imageData, 0, 0);

  rafId = requestAnimationFrame(emulatorLoop);
}

function applyInput() {
  gba.set_button(0, keys.has('KeyZ') || keys.has('KeyA'));
  gba.set_button(1, keys.has('KeyX') || keys.has('KeyS'));
  gba.set_button(2, keys.has('Backspace'));
  gba.set_button(3, keys.has('Enter'));
  gba.set_button(4, keys.has('ArrowRight'));
  gba.set_button(5, keys.has('ArrowLeft'));
  gba.set_button(6, keys.has('ArrowUp'));
  gba.set_button(7, keys.has('ArrowDown'));
  gba.set_button(8, keys.has('KeyE'));
  gba.set_button(9, keys.has('KeyQ'));
}

function togglePause() {
  if (!gba) return;
  running = !running;
  if (running) {
    statusLine.textContent = 'Running — Arrows/WASD move, Z=A, X=B, Enter=Start.';
    rafId = requestAnimationFrame(emulatorLoop);
  } else {
    cancelAnimationFrame(rafId);
    statusLine.textContent = 'Paused. Press Esc to resume.';
  }
}

window.addEventListener('keydown', (event) => {
  if (event.code === 'Escape') {
    event.preventDefault();
    togglePause();
    return;
  }
  if (KEY_BUTTON[event.code] !== undefined) {
    event.preventDefault();
  }
  keys.add(event.code);
});

window.addEventListener('keyup', (event) => {
  keys.delete(event.code);
});

romInput.addEventListener('change', loadRom);
