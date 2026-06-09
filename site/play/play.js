const canvas = document.querySelector('#game-screen');
const ctx = canvas.getContext('2d');
const startButton = document.querySelector('#start-button');
const pauseButton = document.querySelector('#pause-button');
const statusLine = document.querySelector('#game-status');
const scoreValue = document.querySelector('#score-value');
const notesValue = document.querySelector('#notes-value');
const livesValue = document.querySelector('#lives-value');
const stageValue = document.querySelector('#stage-value');
const objectiveItems = [...document.querySelectorAll('[data-objective]')];
const controlButtons = [...document.querySelectorAll('[data-control]')];

const WIDTH = canvas.width;
const HEIGHT = canvas.height;
const keys = new Set();
const touchControls = new Set();

const STAGES = [
  { name: 'Truck', score: 0 },
  { name: 'Littleroot', score: 350 },
  { name: 'Route 101', score: 900 },
  { name: 'Birch', score: 1500 },
];

let state = makeInitialState();
let lastTime = 0;
let rafId = 0;

function makeInitialState() {
  return {
    mode: 'ready',
    score: 0,
    notes: 0,
    lives: 3,
    time: 0,
    invulnerableUntil: 0,
    dashUntil: 0,
    player: { x: 82, y: HEIGHT / 2, size: 18 },
    obstacles: [],
    pickups: [],
    particles: [],
    spawnTimer: 850,
    pickupTimer: 420,
  };
}

function setStatus(message) {
  statusLine.textContent = message;
}

function currentStage() {
  return [...STAGES].reverse().find((stage) => state.score >= stage.score) ?? STAGES[0];
}

function syncHud() {
  scoreValue.textContent = Math.floor(state.score).toString();
  notesValue.textContent = state.notes.toString();
  livesValue.textContent = state.lives.toString();
  stageValue.textContent = currentStage().name;

  const done = new Set();
  if (state.score > 180) done.add('truck');
  if (state.notes >= 8) done.add('notes');
  if (state.score > 900) done.add('grass');
  if (state.score > 1500 && state.notes >= 8) done.add('birch');

  for (const item of objectiveItems) {
    const key = item.dataset.objective;
    item.classList.toggle('complete', done.has(key));
    item.classList.toggle('active', !done.has(key) && firstIncompleteObjective(done) === key);
  }
}

function firstIncompleteObjective(done) {
  return ['truck', 'notes', 'grass', 'birch'].find((key) => !done.has(key));
}

function startRun() {
  state = makeInitialState();
  state.mode = 'running';
  lastTime = performance.now();
  setStatus('Run started. Collect notes, dodge grass, reach Birch.');
  startButton.textContent = 'Restart';
  pauseButton.textContent = 'Pause';
  cancelAnimationFrame(rafId);
  rafId = requestAnimationFrame(loop);
}

function togglePause() {
  if (state.mode === 'ready') return;
  if (state.mode === 'gameover' || state.mode === 'win') {
    startRun();
    return;
  }
  state.mode = state.mode === 'paused' ? 'running' : 'paused';
  pauseButton.textContent = state.mode === 'paused' ? 'Resume' : 'Pause';
  setStatus(state.mode === 'paused' ? 'Paused.' : 'Back on Route 101.');
  lastTime = performance.now();
  if (state.mode === 'running') rafId = requestAnimationFrame(loop);
  else draw();
}

function loop(now) {
  const dt = Math.min(32, now - lastTime || 16);
  lastTime = now;
  if (state.mode === 'running') {
    update(dt);
    draw();
    rafId = requestAnimationFrame(loop);
  }
}

function update(dt) {
  state.time += dt;
  const seconds = dt / 1000;
  state.score += 46 * seconds;

  const dashPressed = keys.has('Space');
  if (dashPressed && state.time > state.dashUntil + 460) {
    state.dashUntil = state.time + 150;
  }
  const speed = state.time < state.dashUntil ? 265 : 170;
  const move = speed * seconds;
  if (isPressed('left')) state.player.x -= move;
  if (isPressed('right')) state.player.x += move;
  if (isPressed('up')) state.player.y -= move;
  if (isPressed('down')) state.player.y += move;
  state.player.x = clamp(state.player.x, 26, WIDTH - 26);
  state.player.y = clamp(state.player.y, 42, HEIGHT - 26);

  state.spawnTimer -= dt;
  if (state.spawnTimer <= 0) {
    spawnObstacle();
    state.spawnTimer = Math.max(310, 920 - state.score * 0.22);
  }

  state.pickupTimer -= dt;
  if (state.pickupTimer <= 0) {
    spawnPickup();
    state.pickupTimer = 1050 + Math.random() * 700;
  }

  for (const obstacle of state.obstacles) {
    obstacle.x -= obstacle.speed * seconds;
    obstacle.wobble += dt * 0.006;
    obstacle.y += Math.sin(obstacle.wobble) * 0.36;
  }
  state.obstacles = state.obstacles.filter((obstacle) => obstacle.x > -60);

  for (const pickup of state.pickups) {
    pickup.x -= pickup.speed * seconds;
    pickup.bob += dt * 0.005;
  }
  state.pickups = state.pickups.filter((pickup) => pickup.x > -30);

  for (const particle of state.particles) {
    particle.x += particle.vx * seconds;
    particle.y += particle.vy * seconds;
    particle.life -= dt;
  }
  state.particles = state.particles.filter((particle) => particle.life > 0);

  handleCollisions();

  if (state.score > 1680 && state.notes >= 8) {
    state.mode = 'win';
    setStatus('Objective clear: Birch rescued. That is a browser game, finally.');
    draw();
  }

  syncHud();
}

function isPressed(direction) {
  const map = {
    left: ['ArrowLeft', 'KeyA'],
    right: ['ArrowRight', 'KeyD'],
    up: ['ArrowUp', 'KeyW'],
    down: ['ArrowDown', 'KeyS'],
  };
  return touchControls.has(direction) || map[direction].some((key) => keys.has(key));
}

function spawnObstacle() {
  const size = 22 + Math.random() * 20;
  state.obstacles.push({
    x: WIDTH + size,
    y: 46 + Math.random() * (HEIGHT - 84),
    size,
    speed: 100 + Math.random() * 62 + state.score * 0.018,
    wobble: Math.random() * 10,
  });
}

function spawnPickup() {
  state.pickups.push({
    x: WIDTH + 28,
    y: 52 + Math.random() * (HEIGHT - 96),
    size: 13,
    speed: 92 + Math.random() * 34,
    bob: Math.random() * 10,
  });
}

function handleCollisions() {
  const player = state.player;
  for (const pickup of state.pickups) {
    if (pickup.collected) continue;
    if (distance(player.x, player.y, pickup.x, pickup.y) < player.size + pickup.size) {
      pickup.collected = true;
      state.notes += 1;
      state.score += 110;
      burst(pickup.x, pickup.y, '#b7ff6a');
      setStatus(`Research note collected: ${state.notes}/8.`);
    }
  }
  state.pickups = state.pickups.filter((pickup) => !pickup.collected);

  if (state.time < state.invulnerableUntil) return;
  for (const obstacle of state.obstacles) {
    if (obstacle.hit) continue;
    if (distance(player.x, player.y, obstacle.x, obstacle.y) < player.size + obstacle.size * 0.48) {
      obstacle.hit = true;
      state.lives -= 1;
      state.invulnerableUntil = state.time + 900;
      burst(player.x, player.y, '#ff7ac8');
      setStatus(`Tall grass hit. ${state.lives} ${state.lives === 1 ? 'life' : 'lives'} left.`);
      if (state.lives <= 0) {
        state.mode = 'gameover';
        setStatus('Run failed. Press Enter or Restart and make it less embarrassing.');
        draw();
      }
      break;
    }
  }
}

function burst(x, y, color) {
  for (let i = 0; i < 12; i += 1) {
    const angle = Math.random() * Math.PI * 2;
    state.particles.push({
      x,
      y,
      vx: Math.cos(angle) * (60 + Math.random() * 80),
      vy: Math.sin(angle) * (60 + Math.random() * 80),
      life: 360 + Math.random() * 220,
      color,
    });
  }
}

function draw() {
  ctx.imageSmoothingEnabled = false;
  drawWorld();
  drawPickups();
  drawObstacles();
  drawPlayer();
  drawParticles();
  if (state.mode === 'ready') drawOverlay('Emerald Dash', 'Press Start Run or Enter');
  if (state.mode === 'paused') drawOverlay('Paused', 'Press Pause to resume');
  if (state.mode === 'gameover') drawOverlay('Run failed', 'Press Enter to restart');
  if (state.mode === 'win') drawOverlay('Birch rescued', 'Press Enter for another run');
}

function drawWorld() {
  const sky = ctx.createLinearGradient(0, 0, 0, HEIGHT);
  sky.addColorStop(0, '#15325a');
  sky.addColorStop(0.45, '#1f6b74');
  sky.addColorStop(1, '#173c26');
  ctx.fillStyle = sky;
  ctx.fillRect(0, 0, WIDTH, HEIGHT);

  ctx.fillStyle = 'rgba(255,255,255,0.08)';
  for (let x = -80 + ((state.time * 0.025) % 80); x < WIDTH; x += 80) {
    ctx.fillRect(x, 0, 42, HEIGHT);
  }

  ctx.fillStyle = '#225f34';
  for (let y = 54; y < HEIGHT; y += 34) {
    ctx.fillRect(0, y, WIDTH, 3);
  }

  ctx.fillStyle = '#9fd66f';
  ctx.fillRect(0, 30, WIDTH, 6);
  ctx.fillRect(0, HEIGHT - 18, WIDTH, 8);
}

function drawPlayer() {
  const { x, y, size } = state.player;
  const blink = state.time < state.invulnerableUntil && Math.floor(state.time / 90) % 2 === 0;
  if (blink) return;
  const dashing = state.time < state.dashUntil;
  ctx.fillStyle = dashing ? '#b7ff6a' : '#69e7ff';
  ctx.fillRect(x - size * 0.8, y - size * 0.8, size * 1.6, size * 1.6);
  ctx.fillStyle = '#f6f4ff';
  ctx.fillRect(x - 5, y - 4, 4, 4);
  ctx.fillRect(x + 4, y - 4, 4, 4);
  ctx.fillStyle = '#090a12';
  ctx.fillRect(x - 7, y + 7, 14, 4);
}

function drawObstacles() {
  for (const obstacle of state.obstacles) {
    ctx.fillStyle = obstacle.hit ? '#6c3252' : '#143f22';
    const x = obstacle.x;
    const y = obstacle.y;
    const s = obstacle.size;
    ctx.fillRect(x - s * 0.25, y - s * 0.8, s * 0.5, s * 1.6);
    ctx.fillRect(x - s * 0.7, y - s * 0.15, s * 1.4, s * 0.3);
    ctx.fillStyle = '#2fd163';
    ctx.fillRect(x - s * 0.15, y - s, s * 0.3, s * 0.36);
  }
}

function drawPickups() {
  for (const pickup of state.pickups) {
    const y = pickup.y + Math.sin(pickup.bob) * 4;
    ctx.fillStyle = '#f6f4ff';
    ctx.fillRect(pickup.x - 9, y - 11, 18, 22);
    ctx.fillStyle = '#b7ff6a';
    ctx.fillRect(pickup.x - 6, y - 7, 12, 3);
    ctx.fillRect(pickup.x - 6, y - 1, 10, 3);
  }
}

function drawParticles() {
  for (const particle of state.particles) {
    ctx.globalAlpha = Math.max(0, particle.life / 580);
    ctx.fillStyle = particle.color;
    ctx.fillRect(particle.x, particle.y, 4, 4);
  }
  ctx.globalAlpha = 1;
}

function drawOverlay(title, subtitle) {
  ctx.fillStyle = 'rgba(5,7,16,0.68)';
  ctx.fillRect(34, 88, WIDTH - 68, 128);
  ctx.strokeStyle = '#69e7ff';
  ctx.lineWidth = 3;
  ctx.strokeRect(34, 88, WIDTH - 68, 128);
  ctx.fillStyle = '#f6f4ff';
  ctx.font = 'bold 34px monospace';
  ctx.fillText(title, 62, 144);
  ctx.fillStyle = '#b8b4d8';
  ctx.font = '18px monospace';
  ctx.fillText(subtitle, 64, 178);
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

function distance(ax, ay, bx, by) {
  return Math.hypot(ax - bx, ay - by);
}

startButton.addEventListener('click', startRun);
pauseButton.addEventListener('click', togglePause);

window.addEventListener('keydown', (event) => {
  if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Space'].includes(event.code)) {
    event.preventDefault();
  }
  if (event.code === 'Enter' && state.mode !== 'running') {
    startRun();
    return;
  }
  if (event.code === 'Escape') {
    togglePause();
    return;
  }
  keys.add(event.code);
});

window.addEventListener('keyup', (event) => {
  keys.delete(event.code);
});

for (const button of controlButtons) {
  const control = button.dataset.control;
  const press = (event) => {
    event.preventDefault();
    touchControls.add(control);
  };
  const release = (event) => {
    event.preventDefault();
    touchControls.delete(control);
  };
  button.addEventListener('pointerdown', press);
  button.addEventListener('pointerup', release);
  button.addEventListener('pointercancel', release);
  button.addEventListener('pointerleave', release);
}

syncHud();
draw();
