import init, { WebGba } from './pkg/vibe_gba.js';

const romInput = document.querySelector('#rom-input');
const romStatus = document.querySelector('#rom-status');
const screen = document.querySelector('#gba-screen');
const ctx = screen.getContext('2d');

const STORAGE_KEY = 'vibe-gba:last-rom-meta';
const pressedButtons = new Set();

let wasmReady = false;
let gba = null;
let animationFrame = 0;
let imageData = ctx.createImageData(screen.width, screen.height);
let frameCount = 0;
let lastFpsAt = performance.now();

const BUTTON = Object.freeze({
  a: 0,
  b: 1,
  select: 2,
  start: 3,
  right: 4,
  left: 5,
  up: 6,
  down: 7,
  r: 8,
  l: 9,
});

const keyToButton = new Map([
  ['ArrowUp', 'up'],
  ['ArrowDown', 'down'],
  ['ArrowLeft', 'left'],
  ['ArrowRight', 'right'],
  ['KeyZ', 'a'],
  ['KeyA', 'a'],
  ['KeyX', 'b'],
  ['KeyS', 'b'],
  ['Enter', 'start'],
  ['Backspace', 'select'],
  ['KeyQ', 'l'],
  ['KeyE', 'r'],
]);

function drawBootScreen(message = 'Choose a local ROM to start') {
  ctx.imageSmoothingEnabled = false;
  ctx.fillStyle = '#07111f';
  ctx.fillRect(0, 0, screen.width, screen.height);
  ctx.fillStyle = '#69e7ff';
  ctx.fillRect(8, 8, screen.width - 16, screen.height - 16);
  ctx.fillStyle = '#090a12';
  ctx.fillRect(10, 10, screen.width - 20, screen.height - 20);
  ctx.fillStyle = '#f6f4ff';
  ctx.font = '12px monospace';
  ctx.fillText('vibe-gba', 20, 42);
  ctx.fillStyle = '#b8b4d8';
  ctx.fillText(message, 20, 72);
  ctx.fillText('browser runtime ready', 20, 96);
}

function setStatus(message) {
  romStatus.textContent = message;
}

function rememberRomMeta(file, bytes) {
  localStorage.setItem(
    STORAGE_KEY,
    JSON.stringify({
      name: file.name,
      size: file.size,
      bytes,
      loadedAt: new Date().toISOString(),
    }),
  );
}

function syncButtons() {
  for (const [name, id] of Object.entries(BUTTON)) {
    gba.set_button(id, pressedButtons.has(name));
  }
}

function stopLoop() {
  if (animationFrame) cancelAnimationFrame(animationFrame);
  animationFrame = 0;
}

function renderFrame() {
  if (!gba) return;

  syncButtons();
  const rgba = gba.run_frame();
  imageData.data.set(rgba);
  ctx.putImageData(imageData, 0, 0);

  frameCount += 1;
  const now = performance.now();
  if (now - lastFpsAt >= 1000) {
    setStatus(`Running locally · ${frameCount} fps-ish · ${Array.from(pressedButtons).join(', ') || 'no input'}`);
    frameCount = 0;
    lastFpsAt = now;
  }

  animationFrame = requestAnimationFrame(renderFrame);
}

function startEmulator(romBytes, file) {
  stopLoop();
  try {
    gba = new WebGba(romBytes);
    imageData = ctx.createImageData(gba.width(), gba.height());
    rememberRomMeta(file, romBytes.length);
    setStatus(`${file.name} loaded locally (${romBytes.length.toLocaleString()} bytes). Running in browser.`);
    renderFrame();
  } catch (error) {
    gba = null;
    console.error(error);
    setStatus(`Could not start emulator: ${error.message || error}`);
    drawBootScreen('emulator start failed');
  }
}

function loadLocalRom(file) {
  if (!wasmReady) {
    setStatus('Runtime still loading. Try again in a second.');
    return;
  }

  const reader = new FileReader();
  reader.addEventListener('load', () => {
    const bytes = new Uint8Array(reader.result);
    startEmulator(bytes, file);
  });
  reader.addEventListener('error', () => {
    setStatus('Could not read that ROM file. Try a .gba or zipped .gba file.');
    drawBootScreen('ROM read failed');
  });
  reader.readAsArrayBuffer(file);
}

romInput.addEventListener('change', (event) => {
  const [file] = event.target.files;
  if (!file) return;
  loadLocalRom(file);
});

window.addEventListener('keydown', (event) => {
  const button = keyToButton.get(event.code);
  if (!button) return;
  event.preventDefault();
  pressedButtons.add(button);
});

window.addEventListener('keyup', (event) => {
  const button = keyToButton.get(event.code);
  if (!button) return;
  event.preventDefault();
  pressedButtons.delete(button);
});

const lastRom = localStorage.getItem(STORAGE_KEY);
if (lastRom) {
  try {
    const meta = JSON.parse(lastRom);
    setStatus(`Last local ROM: ${meta.name}. Choose it again to load; files are not persisted.`);
  } catch {
    localStorage.removeItem(STORAGE_KEY);
  }
}

drawBootScreen('loading browser runtime');

init()
  .then(() => {
    wasmReady = true;
    if (!lastRom) setStatus('Browser runtime ready. Choose a local ROM.');
    drawBootScreen();
  })
  .catch((error) => {
    console.error(error);
    setStatus(`Could not load browser runtime: ${error.message || error}`);
    drawBootScreen('runtime load failed');
  });
