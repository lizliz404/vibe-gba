// ─────────────────────────────── vibe-gba: Route 101 ───────────────────────────────
// Self-contained tile adventure game. No ROM, no upload — press START.
// ─────────────────────────────────────────────────────────────────────────────────────

const canvas = document.getElementById('game-screen');
const ctx = canvas.getContext('2d');
const TILE = 24;
const COLS = 20;
const ROWS = 15;
const W = canvas.width;
const H = canvas.height;

// ── Tile types ──────────────────────────────────────────────────────────────────────
const T = { FLOOR:0, WALL:1, GRASS:2, TREE:3, WATER:4, DOOR:5, HOUSE:6, ROOF:7, BED:8 };

// ── Palettes ────────────────────────────────────────────────────────────────────────
const P = {
  grass:    '#60b048', grass2: '#489830', path:  '#d8c878', path2: '#c0b060',
  tree:     '#388038', tree2:  '#205820', water: '#4888d8', water2:'#3068b8',
  wall:     '#887848', wall2:  '#685838', roof:  '#d84040', roof2: '#a83030',
  house:    '#e8d8b0', house2: '#d0c098',
  player:   '#e04848', player2:'#ff7860', npc:   '#6090e0', npc2:  '#80b0ff',
  enemy:    '#a060c0', enemy2: '#c080e0',
  ui:       '#f8f0e0', uiDark: '#181820', uiMid: '#383850',
  battleBg: '#282840', hpGreen: '#48d860', hpRed: '#e04040',
};

// ── Map data ────────────────────────────────────────────────────────────────────────
// Legend: . = floor(T0)  # = wall(T1)  : = grass(T2)  T = tree(T3)  ~ = water(T4)
//         D = door(T5)   H = house(T6) R = roof(T7)   B = bed(T8)

const MAP_TRUCK = {
  cols:20, rows:15,
  tiles:`
####################
#..................#
#..................#
#..................#
#..................#
#..................#
#..................#
#..................#
#..................#
#..................#
#..................#
#..................#
#..................#
#........DD........#
####################
`.trim(),
  playerSpawn: [10, 12],
  exits: [{ x:9, y:13, w:2, h:1, target:'littleroot', tx:9, ty:1 }],
  npcs: [],
  bgColor: '#383028',
  name: 'Moving Truck',
};

const MAP_LITTLEROOT = {
  cols:20, rows:15,
  tiles:`
TTTTTTTTTTTTTTTTTTTT
TTT:::::TTTTT:::::TTT
TT::::::::::::::::TT
T::::::::::::::::::T
T::HHH:::RRRR:::HH:T
T::HHH:::RRRR:::HH:T
T::HHH:::RRRR:::HH:T
T::::::........:::T
T::::::........:::T
T:::..............T
T:::..............T
T::HHH:......:HHH:T
T::HHH:......:HHH:T
T::HHH:..DD...:HHH:T
T:::::..####...:::T
`.trim(),
  playerSpawn: [10, 13],
  exits: [
    { x:9, y:13, w:2, h:1, target:'truck', tx:10, ty:2 },
    { x:8, y:0, w:4, h:1, target:'route101', tx:10, ty:13 },
  ],
  npcs: [
    { x:4, y:5, name:'Mom', chat:['Take care out there!','Professor Birch is waiting north of town.'] },
    { x:14, y:5, name:'May', chat:["Hey neighbor! I heard you're getting a Pokémon today.","The lab is up north — don't keep the Professor waiting!"] },
  ],
  bgColor: '#305828',
  name: 'Littleroot Town',
};

const MAP_ROUTE101 = {
  cols:20, rows:15,
  tiles:`
TTTT:::::DDD::::TTTT
TT:::............::T
T:....TTTTTTTT....:T
T:...TTTTTTTTTT...:T
T:...TTT::::TTT...:T
:....TTT::::TTT....:
:....TTT::::TTT....:
:.....TT::::TT.....:
:.....TT::::TT.....:
:......T::::T......:
:......T::::T......:
:.......::::.......:
:.......::::.......:
:.......:..:.......:
:.......:..:.......:
`.trim(),
  playerSpawn: [10, 13],
  exits: [
    { x:8, y:13, w:4, h:1, target:'littleroot', tx:10, ty:1 },
    { x:8, y:0, w:5, h:1, target:'win', tx:0, ty:0 },
  ],
  npcs: [
    { x:5, y:4, name:'???', chat:["Help! A wild Zigzagoon has me cornered!","Quick — grab a Poké Ball from my bag!"] },
  ],
  wilds: [{ name:'Zigzagoon', hp:15, atk:8 }, { name:'Poochyena', hp:18, atk:10 }, { name:'Wurmple', hp:12, atk:6 }],
  bgColor: '#285830',
  name: 'Route 101',
};

// ── Parse map tiles ─────────────────────────────────────────────────────────────────
function parseMap(raw) {
  const rows = raw.trim().split('\n');
  const tiles = [];
  for (const row of rows) {
    const tr = [];
    for (const ch of row) {
      const map = {'.':T.FLOOR, '#':T.WALL, ':':T.GRASS, 'T':T.TREE, '~':T.WATER,
                   'D':T.DOOR,  'H':T.HOUSE, 'R':T.ROOF,  'B':T.BED};
      tr.push(map[ch] ?? T.FLOOR);
    }
    while (tr.length < 20) tr.push(T.WALL);
    tiles.push(tr);
  }
  while (tiles.length < 15) tiles.push(Array(20).fill(T.WALL));
  return tiles;
}

const MAPS = {
  truck:     { ...MAP_TRUCK,     tiles: parseMap(MAP_TRUCK.tiles) },
  littleroot:{ ...MAP_LITTLEROOT,tiles: parseMap(MAP_LITTLEROOT.tiles) },
  route101:  { ...MAP_ROUTE101,  tiles: parseMap(MAP_ROUTE101.tiles) },
};

// ── Game state ──────────────────────────────────────────────────────────────────────
let state, cam, animQueue, lastTime;

function resetGame() {
  const m = MAPS.truck;
  const [sx, sy] = m.playerSpawn;
  state = {
    zone: 'truck',
    px: sx, py: sy,
    face: 2, // 0=up 1=right 2=down 3=left
    moving: false, moveTimer: 0,
    hp: 20, maxHp: 20,
    potions: 3,
    starter: null,
    battle: null,
    dialogue: null,
    won: false,
  };
  cam = { x:0, y:0 };
  animQueue = [];
  updateHud();
}

// ── Input ───────────────────────────────────────────────────────────────────────────
const keys = new Set();
window.addEventListener('keydown', e => {
  keys.add(e.code);
  if (['ArrowUp','ArrowDown','ArrowLeft','ArrowRight','Space'].includes(e.code)) e.preventDefault();

  // Dialogue advance
  if (e.code === 'KeyZ' && state.dialogue) {
    e.preventDefault();
    advanceDialogue();
  }
});
window.addEventListener('keyup', e => keys.delete(e.code));

document.getElementById('start-btn').addEventListener('click', () => {
  document.getElementById('start-overlay').classList.add('hidden');
  resetGame();
  lastTime = performance.now();
  requestAnimationFrame(loop);
});

// Battle button clicks
document.getElementById('battle-actions').addEventListener('click', e => {
  const btn = e.target.closest('.battle-btn');
  if (!btn || !state.battle) return;
  const action = btn.dataset.action;
  if (action === 'fight') battleFight();
  else if (action === 'bag') battleBag();
  else if (action === 'run') battleRun();
});

// ── Game loop ───────────────────────────────────────────────────────────────────────
function loop(now) {
  const dt = Math.min(50, now - lastTime);
  lastTime = now;

  if (!state.battle && !state.dialogue && !state.won) {
    updateMovement(dt);
  }

  draw();
  requestAnimationFrame(loop);
}

// ── Movement ────────────────────────────────────────────────────────────────────────
function updateMovement(dt) {
  if (state.moving) {
    state.moveTimer -= dt;
    if (state.moveTimer <= 0) {
      state.moving = false;
      // Check door after move completes
      checkDoor();
      // Check wild encounter
      checkWild();
    }
    return;
  }

  let dx = 0, dy = 0;
  if (keys.has('ArrowUp')    || keys.has('KeyW')) { dy = -1; state.face = 0; }
  else if (keys.has('ArrowDown')  || keys.has('KeyS')) { dy = 1;  state.face = 2; }
  else if (keys.has('ArrowLeft')  || keys.has('KeyA')) { dx = -1; state.face = 3; }
  else if (keys.has('ArrowRight') || keys.has('KeyD')) { dx = 1;  state.face = 1; }

  if (dx === 0 && dy === 0) return;

  // Interact with NPC (Z key)
  if (keys.has('KeyZ')) {
    interactNPC();
    return;
  }

  const nx = state.px + dx, ny = state.py + dy;
  if (!walkable(nx, ny)) return;

  state.px = nx;
  state.py = ny;
  state.moving = true;
  state.moveTimer = 140;
}

function walkable(x, y) {
  const m = currentMap();
  if (x < 0 || x >= m.cols || y < 0 || y >= m.rows) return false;
  const t = m.tiles[y][x];
  return t === T.FLOOR || t === T.GRASS || t === T.DOOR || t === T.BED;
}

function currentMap() { return MAPS[state.zone]; }

function checkDoor() {
  const m = currentMap();
  const t = m.tiles[state.py]?.[state.px];
  if (t !== T.DOOR) return;
  for (const exit of m.exits) {
    if (state.px >= exit.x && state.px < exit.x + exit.w &&
        state.py >= exit.y && state.py < exit.y + exit.h) {
      if (exit.target === 'win') {
        state.won = true;
        showDialogue('Congratulations!', "You've reached Professor Birch and claimed your first Pokémon. Welcome to the world of Hoenn!", () => {});
        return;
      }
      state.zone = exit.target;
      state.px = exit.tx;
      state.py = exit.ty;
      updateHud();
      return;
    }
  }
}

function checkWild() {
  const m = currentMap();
  const t = m.tiles[state.py]?.[state.px];
  if (t !== T.GRASS) return;
  if (!m.wilds) return;
  if (Math.random() > 0.12) return; // 12% encounter rate per step in grass

  const wild = m.wilds[Math.floor(Math.random() * m.wilds.length)];
  startBattle(wild);
}

// ── NPC interaction ─────────────────────────────────────────────────────────────────
function interactNPC() {
  const m = currentMap();
  if (!m.npcs) return;
  for (const npc of m.npcs) {
    if (Math.abs(state.px - npc.x) <= 1 && Math.abs(state.py - npc.y) <= 1) {
      showDialogue(npc.name, npc.chat[0], () => {});
      animQueue = npc.chat.slice(1);
    }
  }
}

function showDialogue(name, text) {
  state.dialogue = { name, text, queue: [...animQueue] };
  document.getElementById('dialogue-name').textContent = name;
  document.getElementById('dialogue-text').textContent = text;
  document.getElementById('dialogue-box').classList.remove('hidden');
  animQueue = [];
}

function advanceDialogue() {
  if (!state.dialogue) return;
  const q = state.dialogue.queue;
  if (q && q.length > 0) {
    document.getElementById('dialogue-text').textContent = q.shift();
  } else {
    state.dialogue = null;
    document.getElementById('dialogue-box').classList.add('hidden');
  }
}

// ── Battle system ────────────────────────────────────────────────────────────────────
function startBattle(wild) {
  state.battle = {
    name: wild.name,
    hp: wild.hp,
    maxHp: wild.hp,
    atk: wild.atk,
    log: [`A wild ${wild.name} appeared!`],
  };
  document.getElementById('battle-box').classList.remove('hidden');
  showBattleLog();
}

function battleFight() {
  if (!state.battle) return;
  const b = state.battle;
  if (b.hp <= 0) return;

  const dmg = 8 + Math.floor(Math.random() * 8);
  b.hp -= dmg;
  b.log.push(`You attack! Dealt ${dmg} damage.`);

  if (b.hp <= 0) {
    b.hp = 0;
    b.log.push(`Wild ${b.name} fainted!`);
    endBattle(true);
    return;
  }

  enemyTurn();
}

function battleBag() {
  if (!state.battle) return;
  if (state.potions <= 0) {
    state.battle.log.push('No potions left!');
    showBattleLog();
    return;
  }
  state.potions -= 1;
  state.hp = Math.min(state.maxHp, state.hp + 20);
  state.battle.log.push(`Used a Potion! HP restored to ${state.hp}.`);
  updateHud();
  enemyTurn();
}

function battleRun() {
  if (!state.battle) return;
  if (Math.random() < 0.5) {
    state.battle.log.push('Got away safely!');
    endBattle(false);
  } else {
    state.battle.log.push("Can't escape!");
    showBattleLog();
    enemyTurn();
  }
}

function enemyTurn() {
  const b = state.battle;
  if (b.hp <= 0) return;

  const dmg = b.atk + Math.floor(Math.random() * 5) - 2;
  const actual = Math.max(1, dmg);
  state.hp -= actual;
  b.log.push(`${b.name} attacks! Took ${actual} damage.`);
  updateHud();

  if (state.hp <= 0) {
    state.hp = 0;
    b.log.push('You blacked out!');
    endBattle(false);
  } else {
    showBattleLog();
  }
}

function showBattleLog() {
  if (!state.battle) return;
  const b = state.battle;
  document.getElementById('battle-text').innerHTML =
    `<span class="enemy-name">${b.name}</span> HP: <span class="hp-bar"><span style="width:${(b.hp/b.maxHp)*100}%"></span></span> ${b.hp}/${b.maxHp}<br>` +
    b.log[b.log.length - 1];
}

function endBattle(won) {
  if (won) {
    // Heal a bit
    state.hp = Math.min(state.maxHp, state.hp + 10);
    updateHud();
  } else if (state.hp <= 0) {
    // Respawn
    state.hp = state.maxHp;
    const m = currentMap();
    [state.px, state.py] = m.playerSpawn;
    updateHud();
  }
  state.battle = null;
  document.getElementById('battle-box').classList.add('hidden');
}

// ── HUD ──────────────────────────────────────────────────────────────────────────────
function updateHud() {
  const m = currentMap();
  document.getElementById('hud-zone').textContent = m.name;
  document.getElementById('hp-val').textContent = `${state.hp}/${state.maxHp}`;
  document.getElementById('potions-val').textContent = state.potions;
}

// ── Rendering ────────────────────────────────────────────────────────────────────────
let walkAnim = 0;

function draw() {
  ctx.imageSmoothingEnabled = false;
  const m = currentMap();

  // Background
  ctx.fillStyle = m.bgColor;
  ctx.fillRect(0, 0, W, H);

  // Camera centers on player
  const cx = state.px * TILE + TILE/2;
  const cy = state.py * TILE + TILE/2;
  cam.x += (cx - W/2 - cam.x) * 0.3;
  cam.y += (cy - H/2 - cam.y) * 0.3;
  const ox = Math.round(-cam.x);
  const oy = Math.round(-cam.y);

  // Draw tiles
  for (let y = 0; y < m.rows; y++) {
    for (let x = 0; x < m.cols; x++) {
      const tx = ox + x * TILE, ty = oy + y * TILE;
      if (tx + TILE < -10 || tx > W + 10 || ty + TILE < -10 || ty > H + 10) continue;
      drawTile(m.tiles[y][x], tx, ty, x, y);
    }
  }

  // Draw NPCs
  if (m.npcs) {
    for (const npc of m.npcs) {
      const tx = ox + npc.x * TILE, ty = oy + npc.y * TILE;
      drawNpc(tx, ty);
    }
  }

  // Draw player
  const ptx = ox + state.px * TILE;
  const pty = oy + state.py * TILE;
  if (state.moving) {
    walkAnim += 0.06;
    const bob = Math.sin(walkAnim * Math.PI) * 2;
    drawPlayer(ptx, pty + bob);
  } else {
    walkAnim = 0;
    drawPlayer(ptx, pty);
  }

  // Win overlay
  if (state.won) {
    ctx.fillStyle = 'rgba(0,0,0,0.6)';
    ctx.fillRect(0, 0, W, H);
    ctx.fillStyle = '#f8f0e0';
    ctx.font = 'bold 28px monospace';
    ctx.textAlign = 'center';
    ctx.fillText('ROUTE CLEARED!', W/2, H/2 - 30);
    ctx.font = '16px monospace';
    ctx.fillText('You reached Professor Birch.', W/2, H/2 + 10);
    ctx.fillText('Refresh to play again.', W/2, H/2 + 34);
    ctx.textAlign = 'start';
  }
}

function drawTile(t, x, y, gx, gy) {
  // Animated water
  const woff = Math.sin(gx * 0.5 + Date.now() * 0.003) * 0.5 + Math.cos(gy * 0.4 + Date.now() * 0.004) * 0.5;

  switch (t) {
    case T.FLOOR:
      ctx.fillStyle = P.path; ctx.fillRect(x, y, TILE, TILE);
      ctx.fillStyle = P.path2;
      ctx.fillRect(x+2, y+2, 2, 2); ctx.fillRect(x+16, y+10, 2, 2);
      ctx.fillRect(x+10, y+18, 2, 2);
      break;
    case T.WALL:
      ctx.fillStyle = P.wall; ctx.fillRect(x, y, TILE, TILE);
      ctx.fillStyle = P.wall2;
      ctx.fillRect(x, y, TILE, 2); ctx.fillRect(x, y+11, TILE, 2);
      ctx.fillRect(x, y+22, TILE, 2);
      // Brick lines
      ctx.fillStyle = P.wall2;
      ctx.fillRect(x+6, y, 1, TILE); ctx.fillRect(x+13, y, 1, TILE);
      ctx.fillRect(x+19, y, 1, TILE);
      break;
    case T.GRASS:
      ctx.fillStyle = P.grass; ctx.fillRect(x, y, TILE, TILE);
      // Grass blades
      ctx.fillStyle = P.grass2;
      ctx.fillRect(x+4, y+2, 1, 4); ctx.fillRect(x+14, y+6, 1, 4);
      ctx.fillRect(x+8, y+14, 1, 4); ctx.fillRect(x+18, y+18, 1, 4);
      ctx.fillRect(x+2, y+10, 1, 3);
      // Tall grass variant
      const m = currentMap();
      if (m.wilds && Math.random() > 0.999) { // occasionally extra blade
        ctx.fillStyle = '#70c858'; ctx.fillRect(x+10, y+4, 1, 5);
      }
      break;
    case T.TREE:
      // Trunk
      ctx.fillStyle = '#6b4c30'; ctx.fillRect(x+8, y+10, 8, 14);
      // Canopy shadow
      ctx.fillStyle = P.tree2; ctx.fillRect(x+2, y+2, 20, 18);
      ctx.beginPath(); ctx.arc(x+12, y+10, 10, 0, Math.PI*2); ctx.fill();
      // Canopy top
      ctx.fillStyle = P.tree; ctx.fillRect(x, y, 24, 16);
      ctx.beginPath(); ctx.arc(x+12, y+8, 11, 0, Math.PI*2); ctx.fill();
      // Highlight
      ctx.fillStyle = '#50b050';
      ctx.beginPath(); ctx.arc(x+9, y+5, 5, 0, Math.PI*2); ctx.fill();
      break;
    case T.WATER:
      ctx.fillStyle = P.water; ctx.fillRect(x, y, TILE, TILE);
      // Wave highlights
      ctx.fillStyle = P.water2;
      const w1 = 4 + woff * 2, w2 = 16 - woff;
      ctx.fillRect(x+w1, y+4, 8, 1); ctx.fillRect(x+w2, y+12, 8, 1);
      ctx.fillRect(x+w1+2, y+20, 6, 1);
      break;
    case T.DOOR:
      ctx.fillStyle = P.path; ctx.fillRect(x, y, TILE, TILE);
      ctx.fillStyle = '#483818'; ctx.fillRect(x+4, y, 16, 24);
      ctx.fillStyle = '#604820';
      ctx.fillRect(x+5, y+1, 14, 22);
      ctx.fillStyle = '#d8a030';
      ctx.fillRect(x+18, y+10, 2, 4);
      break;
    case T.HOUSE:
      // House wall
      ctx.fillStyle = P.house; ctx.fillRect(x, y, TILE, TILE);
      ctx.fillStyle = P.house2;
      ctx.fillRect(x, y+6, TILE, 2); ctx.fillRect(x, y+12, TILE, 2);
      ctx.fillRect(x, y+18, TILE, 2);
      // Window
      ctx.fillStyle = '#88c8f0';
      ctx.fillRect(x+6, y+3, 4, 4); ctx.fillRect(x+14, y+3, 4, 4);
      break;
    case T.ROOF:
      ctx.fillStyle = P.roof; ctx.fillRect(x, y, TILE, TILE);
      ctx.fillStyle = P.roof2;
      ctx.fillRect(x, y+4, TILE, 2); ctx.fillRect(x, y+10, TILE, 2);
      ctx.fillRect(x, y+16, TILE, 2); ctx.fillRect(x, y+22, TILE, 2);
      // Roof triangle
      ctx.fillStyle = '#e86060';
      ctx.beginPath(); ctx.moveTo(x, y+4); ctx.lineTo(x+12, y-2); ctx.lineTo(x+24, y+4); ctx.fill();
      break;
    case T.BED:
      ctx.fillStyle = P.path; ctx.fillRect(x, y, TILE, TILE);
      ctx.fillStyle = '#e8c870'; ctx.fillRect(x+2, y+6, 20, 14);
      ctx.fillStyle = '#f0d880'; ctx.fillRect(x+2, y+6, 20, 4);
      ctx.fillStyle = '#c0a848'; ctx.fillRect(x+14, y+20, 8, 4);
      break;
  }
}

function drawPlayer(x, y) {
  const s = TILE;
  // Shadow
  ctx.fillStyle = 'rgba(0,0,0,0.2)';
  ctx.beginPath(); ctx.ellipse(x+s/2, y+s-2, s/3, 3, 0, 0, Math.PI*2); ctx.fill();

  // Body
  const bodyColor = P.player;
  const hatColor = P.player2;

  // Hat (red cap)
  ctx.fillStyle = '#d83838';
  ctx.beginPath(); ctx.arc(x+s/2, y+6, 8, Math.PI, 0); ctx.fill();
  ctx.fillRect(x+5, y+4, 6, 3);
  ctx.fillStyle = '#ff6050';
  ctx.beginPath(); ctx.arc(x+s/2, y+6, 5, Math.PI, 0); ctx.fill();

  // Hair
  ctx.fillStyle = '#483018';
  ctx.fillRect(x+9, y+8, 3, 5); ctx.fillRect(x+13, y+7, 3, 5);

  // Face
  ctx.fillStyle = '#fcd8b8';
  ctx.fillRect(x+7, y+10, 10, 8);

  // Eyes
  const eyeDir = [[8,12,9,12], [10,12,11,12], [9,11,8,11], [10,12,9,12]][state.face];
  ctx.fillStyle = '#181820';
  ctx.fillRect(x+eyeDir[0], y+eyeDir[1], 2, 2);
  ctx.fillRect(x+eyeDir[2], y+eyeDir[3], 2, 2);

  // Body
  ctx.fillStyle = '#3068d0';
  ctx.fillRect(x+7, y+17, 10, 7);
  // Arms
  ctx.fillStyle = '#fcd8b8';
  ctx.fillRect(x+3, y+18, 4, 5); ctx.fillRect(x+17, y+18, 4, 5);
  // Legs
  ctx.fillStyle = '#284898';
  ctx.fillRect(x+8, y+22, 4, 3);
}

function drawNpc(x, y) {
  const s = TILE;
  // Shadow
  ctx.fillStyle = 'rgba(0,0,0,0.2)';
  ctx.beginPath(); ctx.ellipse(x+s/2, y+s-2, s/3, 3, 0, 0, Math.PI*2); ctx.fill();

  // Body
  ctx.fillStyle = P.npc;
  ctx.fillRect(x+7, y+13, 10, 11);
  // Head
  ctx.fillStyle = '#fcd8b8';
  ctx.fillRect(x+7, y+6, 10, 8);
  // Hair
  ctx.fillStyle = '#604828';
  ctx.fillRect(x+6, y+4, 12, 6);
  // Eyes
  ctx.fillStyle = '#181820';
  ctx.fillRect(x+9, y+9, 2, 2); ctx.fillRect(x+13, y+9, 2, 2);
}

// ── Init ────────────────────────────────────────────────────────────────────────────
resetGame();
draw();
updateHud();
