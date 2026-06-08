const romInput = document.querySelector('#rom-input');
const romStatus = document.querySelector('#rom-status');
const screen = document.querySelector('#gba-screen');
const ctx = screen.getContext('2d');

const STORAGE_KEY = 'vibe-gba:last-rom-meta';
const pressedButtons = new Set();

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
  ctx.fillText('WASM core: pending', 20, 96);
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

function loadLocalRom(file) {
  const reader = new FileReader();
  reader.addEventListener('load', () => {
    const buffer = reader.result;
    const bytes = buffer.byteLength;
    rememberRomMeta(file, bytes);
    setStatus(`${file.name} loaded locally (${bytes.toLocaleString()} bytes). Emulator core hookup next.`);
    drawBootScreen('ROM loaded locally');
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
  setStatus(`Input: ${Array.from(pressedButtons).join(', ')}`);
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

drawBootScreen();
