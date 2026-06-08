# vibe-gba

`vibe-gba` is a scratch-built Rust Game Boy Advance emulator prototype. It was first developed with one concrete milestone in mind: boot a legally supplied `Pokemon Emerald.gba` ROM far enough to render the game, pass the new-game onboarding route, walk through Littleroot Town into Route 101, reach Professor Birch's starter bag, and receive the first Pokemon in Birch's lab.

This repository does not include any ROM, save file, emulator state snapshot, or existing GBA emulator engine. The implementation is intentionally small and direct: ARM7TDMI execution, memory bus, cartridge loading, save memory, DMA/timers/interrupts, PPU background and OBJ rendering, keyboard input, screenshots, save states, and debug support for narrowing the next hardware gaps.

![Littleroot entry](docs/screenshots/littleroot-entry.png)
![Walking in Littleroot](docs/screenshots/littleroot-walk.png)

## Status

Current milestone:

- Boots Pokemon Emerald from a user-provided `.gba` or zipped `.gba` file.
- Renders title/menu/intro/truck/Littleroot/Route 101/Birch Lab scenes on the fresh-ROM milestone route.
- Supports deterministic headless runs with scripted input, screenshots, state save/load, and debug dumps.
- Supports interactive keyboard play once the window is opened.
- Executes Emerald's native overworld callback/input/player-step path in the Littleroot development state with gameplay HLE disabled. A fixed BIOS `ObjAffineSet` implementation now prevents sprite affine updates from corrupting callee-saved registers and starving `CB1_Overworld`.
- The previous direct movement / Route 101 / starter high-level gameplay assists are disabled by default. The older flow shortcuts that wrote Emerald callbacks/tasks to skip title, naming, and sprite-animation waits are also disabled by default.
- With those shortcuts disabled, a fresh-ROM scripted run now reaches the real moving truck, follows Mom into the player's house, sets the wall clock, exits to Littleroot Town, meets May, enters Route 101, triggers the Birch rescue, chooses Torchic from Birch's bag, clears the first battle, arrives in Birch's lab, and has Torchic in the party from KEYINPUT alone.

This is not a general-purpose, compatibility-focused GBA emulator yet. The fresh-ROM Emerald M1 route is now proven through starter acquisition, but many games and many Emerald paths are expected to need more CPU, PPU, audio, BIOS, timing, and hardware coverage.

## Build

Install Rust, then:

```bash
cargo build --release
```

The binary is:

```bash
target/release/vibe-gba
```

## Run From A Fresh ROM

Provide your own legally dumped ROM:

```bash
target/release/vibe-gba "/path/to/Pokemon Emerald.gba" \
  --save /tmp/emerald-fresh.sav
```

Keyboard mapping:

| GBA | Keyboard |
| --- | --- |
| A | `Z` or `A` |
| B | `X` or `S` |
| Start | `Enter` |
| Select | `Backspace` |
| D-pad | Arrow keys |
| L / R | `Q` / `E` |
| Quit | `Esc` |

From the title screen:

1. Press `Enter` or `Z` on the title screen.
2. If the dry battery prompt appears, press `Z` through it until the `NEW GAME` / `OPTION` menu is visible.
3. Choose `NEW GAME` with `Z`.
4. Advance Birch's intro text with `Z`.
5. Pick gender, enter a name, then continue toward the moving truck scene.

Manual M1 smoke path:

1. In the moving truck, walk right to trigger the native Littleroot moving-in sequence.
2. Follow Mom into the house, go upstairs, set the wall clock, then return downstairs and advance the TV/Mom dialogue.
3. Exit the house into Littleroot, walk east to May's house, go upstairs, and inspect the Poke Ball to meet May.
4. Leave May's house and walk to Littleroot's north exit. The native script should lead into the Route 101 Birch rescue scene.
5. Walk to Birch's bag, press `Z`, choose a starter, then use the battle menu normally to defeat Zigzagoon.
6. Advance Birch's lab dialogue until the field is unlocked again. A successful run ends with Torchic in the party and native movement available again in Littleroot/Route 101.

The old direct gameplay HLE path could force parts of the route. That path is disabled now; the current milestone proof is for Emerald's own native field engine to drive truck movement, Littleroot movement, Route 101 transitions, starter selection, the first battle, and starter acquisition from KEYINPUT alone.

## Native-Loop Development Checks

### Full Fresh-ROM M1 Check

This is the current headless acceptance check for the Emerald native-loop M1 route. It does not load a save state, and the input script only presses Start, A, and D-pad buttons:

```bash
rm -f /tmp/vibe_goal_full_fresh_starter.sav
SCRIPT="$(cat scripts/emerald-m1-onboarding.input)"

target/release/vibe-gba "/path/to/Pokemon Emerald.gba" \
  --save /tmp/vibe_goal_full_fresh_starter.sav \
  --frames 65200 \
  --input-script "$SCRIPT" \
  --screenshot /tmp/vibe_goal_full_fresh_starter.png \
  --save-state /tmp/vibe_goal_full_fresh_starter.bin \
  --dump-state
```

Expected shape from the validated run:

- `CPU ... gameplay_hle=false legacy_flow_hle=false`
- `FIELD ... layoutId=003a`
- `SAVE save1=... route101=3 birch_lab=3 starter=1`
- `PARTY count=1 species=280 level=5 hp=19/19`
- `SAVE_FLAGS pokemon_get=true rescued_birch=true hide_bag=true hide_zigzagoon=true hide_lab_birch=false`
- non-zero native-loop counts for `FieldGetPlayerInput`, `ProcessPlayerFieldInput`, `PlayerStep`, `ScrCmd_applymovement`, and `ScrCmd_waitmovement`
- screenshot shows Professor Birch's lab after the starter sequence.

`SPECIES_TORCHIC` is `280` in Emerald's internal species constants.

After that full check, this shorter state-based probe exits Birch's lab, walks through Littleroot, and crosses into Route 101 with the starter still in the party:

```bash
target/release/vibe-gba "/path/to/Pokemon Emerald.gba" \
  --load-state /tmp/vibe_goal_full_fresh_starter.bin \
  --frames 10400 \
  --input-script '20:1:down,180:1:down,340:1:down,500:1:down,660:1:down,820:1:down,980:1:down,1200:1:down,1600:1:left,1800:1:right,2000:1:up,2220:1:right,2380:1:right,2540:1:right,2700:1:right,2920:1:up,3080:1:up,3240:1:up,3400:1:up,3560:1:up,3720:1:up,3880:1:up,4040:1:up,4200:1:up,4360:1:up,4520:1:up,4680:1:up,4840:1:up,5000:1:up,5160:1:up,5320:1:up,5680:1:left,5840:1:right,6000:1:up,6160:1:down,7420:1:up,7640:1:up,7920:1:left,8160:1:right,8400:1:down,8820:1:up,9060:1:left,9300:1:right,9540:1:down,9780:1:up' \
  --screenshot /tmp/vibe_goal_after_starter_route101_probe.png \
  --dump-state
```

Expected shape:

- `FIELD ... layoutId=0011`
- `PARTY count=1 species=280 level=5`
- `SAVE save1=... route101=3 birch_lab=3 starter=1`
- non-zero `FieldGetPlayerInput`, `ProcessPlayerFieldInput`, and `PlayerStep`

The current fresh-ROM no-gameplay-HLE check reaches the player's house 1F after the moving-truck and Mom escort sequence:

```bash
target/release/vibe-gba "/path/to/Pokemon Emerald.gba" \
  --save /tmp/vibe_goal_fresh_no_helper.sav \
  --frames 13200 \
  --input-script '3200:20:start,3850:20:a,4400:20:a,4800:20:a,5600:20:a,6700:20:a,7120:8:start,7180:8:a,8000:8:a,8900:140:right,10000:8:a,10300:8:a,10600:8:a,10900:8:a,11200:8:a,11500:8:a,11800:8:a,12100:8:a,12400:8:a,12700:8:a,13000:8:a' \
  --screenshot /tmp/vibe_goal_fresh_no_helper.png \
  --save-state /tmp/vibe_goal_fresh_no_helper.bin \
  --dump-state
```

Expected shape:

- `CPU ... gameplay_hle=false legacy_flow_hle=false`
- `FIELD ... layoutId=0036`
- `SCRIPT ... lockFieldControls=0`
- non-zero `PC_HITS` for `CB1_Overworld`, `DoCB1_Overworld`, `FieldGetPlayerInput`, `ProcessPlayerFieldInput`, and `PlayerStep`
- screenshot shows the player inside the first floor of the house.

This is an intermediate debug checkpoint for shortening route work; the full fresh-ROM M1 check above is the current acceptance test.

Native movement can be probed from that generated house state:

```bash
target/release/vibe-gba "/path/to/Pokemon Emerald.gba" \
  --load-state /tmp/vibe_goal_fresh_no_helper.bin \
  --frames 360 \
  --input-script '20:80:left,140:80:right,260:80:up' \
  --screenshot /tmp/vibe_goal_house_native_walk.png \
  --dump-state
```

Expected shape:

- `CPU ... gameplay_hle=false legacy_flow_hle=false`
- `PC_HITS` shows `FieldGetPlayerInput`, `ProcessPlayerFieldInput`, and `PlayerStep`
- `PLAYER_OBJECT` coordinates and facing/movement direction change through the native object-event path.

The current full fresh-ROM no-gameplay-HLE Littleroot check does not load a save state. It starts from the title/new-game route, goes through the moving truck, Mom escort, wall clock, 1F TV/Mom sequence, exits the house, and performs a short native movement probe outside:

```bash
target/release/vibe-gba "/path/to/Pokemon Emerald.gba" \
  --save /tmp/vibe_goal_full_fresh_littleroot.sav \
  --frames 22300 \
  --input-script '3200:20:start,3850:20:a,4400:20:a,4800:20:a,5600:20:a,6700:20:a,7120:8:start,7180:8:a,8000:8:a,8900:140:right,10000:8:a,10300:8:a,10600:8:a,10900:8:a,11200:8:a,11500:8:a,11800:8:a,12100:8:a,12400:8:a,12700:8:a,13000:8:a,13220:340:up,13640:8:a,13780:8:a,13940:8:a,14120:8:a,14320:8:a,14540:2:left,14630:1:up,14700:8:a,14880:8:a,15060:8:a,15240:8:a,15519:8:a,15640:8:up,15740:8:a,16060:8:a,16360:8:a,16660:8:a,16960:8:a,17210:8:a,17400:1:right,17480:1:up,17700:8:a,17920:8:a,18140:8:a,18360:8:a,18580:8:a,18800:8:a,19020:8:a,19240:8:a,19460:8:a,19680:8:a,19900:8:a,20120:8:a,20260:1:right,20400:1:right,20540:1:right,20680:1:right,20840:1:down,21000:1:down,21140:1:down,21400:1:left,21600:1:right,21800:1:up,22000:1:down' \
  --screenshot /tmp/vibe_goal_full_fresh_littleroot.png \
  --save-state /tmp/vibe_goal_full_fresh_littleroot.bin \
  --dump-state
```

Expected shape:

- `CPU ... gameplay_hle=false legacy_flow_hle=false`
- `FIELD ... layoutId=000a`
- `PLAYER_AVATAR flags=21 [foot|controllable]`
- non-zero native-loop counts for `FieldGetPlayerInput`, `ProcessPlayerFieldInput`, `PlayerStep`, `ScrCmd_applymovement`, and `ScrCmd_waitmovement`
- `SCRIPT ... lockFieldControls=0`
- screenshot shows the player outside in Littleroot Town.

For one-frame headless D-pad pulses, Emerald may temporarily clear `PLAYER_AVATAR_FLAG_CONTROLLABLE` while a step or forced-movement check is in progress. Use `PC_HITS`, `PLAYER_OBJECT cur=(x, y)`, facing/move direction, and the final settled `lockFieldControls=0` state to distinguish native movement from direct movement HLE.

The local native naming-screen segment can be checked from a generated state at Birch's "What's your name?" prompt:

```bash
target/release/vibe-gba "/path/to/Pokemon Emerald.gba" \
  --load-state /tmp/vibe_after_boy_1200.bin \
  --frames 400 \
  --input-script '0:20:a' \
  --screenshot /tmp/vibe_native_name.png \
  --save-state /tmp/vibe_native_name.bin \
  --dump-state
```

Expected shape:

- `CPU ... gameplay_hle=false legacy_flow_hle=false`
- `GMAIN cb2=080e4f59`
- active naming-screen tasks around `080e...`
- screenshot shows the native `YOUR NAME?` keyboard.

From that naming-screen state, pressing `Start` then `A` accepts the default name and returns to Birch's name-confirmation menu:

```bash
target/release/vibe-gba "/path/to/Pokemon Emerald.gba" \
  --load-state /tmp/vibe_native_name.bin \
  --frames 300 \
  --input-script '20:8:start,80:8:a' \
  --screenshot /tmp/vibe_after_name_ok.png \
  --save-state /tmp/vibe_after_name_ok.bin \
  --dump-state
```

From the name-confirmation menu, pressing `A` advances through the native Birch/title-to-field path into the moving truck:

```bash
target/release/vibe-gba "/path/to/Pokemon Emerald.gba" \
  --load-state /tmp/vibe_after_name_ok.bin \
  --frames 800 \
  --input-script '20:8:a' \
  --screenshot /tmp/vibe_native_truck.png \
  --save-state /tmp/vibe_native_truck.bin \
  --dump-state
```

Expected shape:

- `CPU ... gameplay_hle=false legacy_flow_hle=false`
- `GMAIN cb1=08085e05 cb2=08085e5d`
- `FIELD ... layoutId=00ed`
- non-zero `PC_HITS` for `CB1_Overworld`, `DoCB1_Overworld`, `FieldGetPlayerInput`, `ProcessPlayerFieldInput`, and `PlayerStep`.

From the truck state, holding Right currently triggers Emerald's native truck exit / Littleroot transition:

```bash
target/release/vibe-gba "/path/to/Pokemon Emerald.gba" \
  --load-state /tmp/vibe_native_truck.bin \
  --frames 160 \
  --input-script '0:140:right' \
  --screenshot /tmp/vibe_native_littleroot_from_truck.png \
  --dump-state
```

Expected shape:

- `GMAIN cb1=08085e05 cb2=08085e5d`
- `FIELD ... layoutId=000a`
- `PLAYER_OBJECT` reports native object-event coordinates near `cur=(10, 17)`
- `WATCHLOG` contains object/player writes generated by the native PC path, while both HLE guard flags remain false.

When `--dump-state` is enabled, the CPU line should include both `gameplay_hle=false` and `legacy_flow_hle=false`. That is the important guardrail for the current native-loop work: scripted inputs may press buttons, but movement and gameplay progression must not come from direct player-coordinate, sprite/OAM, map-var, flag, party-data, callback, or task-function writes.

The old local development run used save states plus direct gameplay HLE to verify the starter milestone. Those state files are not committed because they may contain ROM-derived data, and that HLE route is now legacy-only. If you have generated compatible local states, the old command shape looked like this, but it is no longer a passing criterion for the native-loop goal:

```bash
target/release/vibe-gba "/path/to/Pokemon Emerald.gba" \
  --load-state states/littleroot_entry_fix15.bin \
  --frames 820 \
  --input-script '0:54:right,90:125:up,300:32:left,350:8:up,420:40:a,580:40:a' \
  --save-state /tmp/vibe_littleroot_to_starter_lab.bin \
  --screenshot /tmp/vibe_littleroot_to_starter_lab.png \
  --dump-state
```

Former legacy-HLE debug markers:

- `layoutId=003a`
- `callback2=08085e5d`
- `PLAYER_OBJECT` reports `map=1.4`
- `SAVE route101=3 birch_lab=3 starter=0` plus party debug showing a starter in the party
- `SAVE_FLAGS pokemon_get=true rescued_birch=true hide_bag=true hide_zigzagoon=true hide_lab_birch=false`
- The screenshot is Professor Birch's lab with the player and Birch visible.

The current no-gameplay-HLE Littleroot native-loop probe can be run from a locally generated Littleroot development state:

```bash
target/release/vibe-gba "/path/to/Pokemon Emerald.gba" \
  --load-state states/littleroot_entry_fix15.bin \
  --frames 200 \
  --input-script '0:200:right' \
  --screenshot /tmp/littleroot_native_probe.png \
  --dump-state
```

The dump should include `gameplay_hle=false`. The important native-loop proof is the function chain, not just the screenshot:

Current expected diagnostic shape for that probe after the `ObjAffineSet` fix:

- `GMAIN held=0010` or another D-pad bit proves KEYINPUT reached Emerald's `gMain`.
- `PC_HITS` should show non-zero counts for `CB1_Overworld`, `DoCB1_Overworld`, `FieldGetPlayerInput`, `ProcessPlayerFieldInput`, and `PlayerStep`.
- `PLAYER_OBJECT` should change through Emerald's object-event movement path, for example `face=4 moveDir=4`, `heldMove`, `action=28`, and a changed `cur=(x, y)` when holding Right.
- `WATCHLOG` records writes to player/object fields such as `obj00.action`, `obj00.flags`, `obj00.cur*`, and `player.tile`; these are expected only when they are produced by the native PC path above, not by gameplay HLE.
- This state-based probe is not the final acceptance test. Use the full fresh-ROM M1 check above for the no-save-state route through truck/Littleroot/Route 101/starter acquisition.

## CLI

```text
vibe-gba <rom.gba|zip>
  --save file.sav
  --frames N
  --screenshot out.png
  --save-state state.bin
  --load-state state.bin
  --dump-state
  --hold-a
  --hold-start
  --input-script frame:duration:buttons,...
  --stop-pc HEX
  --stop-hit N
  --stop-invalid
  --max-steps N
  --turbo
  --trace
```

Input script example:

```text
0:32:down,120:64:right,260:16:a
```

## Implementation Notes

- `src/cpu.rs`: ARM/Thumb CPU core plus disabled legacy Emerald milestone HLE hooks for early movement, map transitions, and starter acquisition.
- `src/bios.rs`: BIOS SWI helpers, including `ObjAffineSet`; the destination offset is byte-based and is covered by a regression test because Emerald uses `offset=2` for stack-local affine matrices.
- `src/bus.rs`: memory map, IO, DMA, timers, save memory, state serialization, and debug watch logging. The bus no longer contains Emerald-specific VBlank helpers for DMA3 request queues or window tile mirroring; Emerald's own code must drive those through emulated hardware.
- `src/ppu.rs`: framebuffer rendering for bitmap/text/affine backgrounds, OBJ sprites, and blending.
- `src/cartridge.rs`: raw and zipped ROM loading plus flash save behavior.
- `src/gba.rs`: emulator orchestration, frame stepping, save states, and debug summaries, including read-only Emerald progress/party diagnostics.
- `src/main.rs`: CLI, keyboard input, window loop, screenshots, and scripted input.

The original development prompt is preserved in [PROMPT.md](PROMPT.md).

## License

MIT. See [LICENSE](LICENSE).
