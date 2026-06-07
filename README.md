# vibe-gba

`vibe-gba` is a scratch-built Rust Game Boy Advance emulator prototype. It was developed with one concrete milestone in mind: boot a legally supplied `Pokemon Emerald.gba` ROM far enough to render the game, pass the new-game onboarding route, exit the moving truck, and walk around Littleroot Town.

This repository does not include any ROM, save file, emulator state snapshot, or existing GBA emulator engine. The implementation is intentionally small and direct: ARM7TDMI execution, memory bus, cartridge loading, save memory, DMA/timers/interrupts, PPU background and OBJ rendering, keyboard input, screenshots, save states, and a handful of Emerald-specific high-level assists for the current milestone path.

![Littleroot entry](docs/screenshots/littleroot-entry.png)
![Walking in Littleroot](docs/screenshots/littleroot-walk.png)

## Status

Current milestone:

- Boots Pokemon Emerald from a user-provided `.gba` or zipped `.gba` file.
- Renders title/menu/intro/truck/Littleroot scenes.
- Supports deterministic headless runs with scripted input, screenshots, state save/load, and debug dumps.
- Supports interactive keyboard play once the window is opened.
- Can enter Littleroot Town from the truck and move the visible player sprite around the town.

This is not a general-purpose, compatibility-focused GBA emulator yet. Many games and many Emerald paths are expected to need more CPU, PPU, audio, BIOS, timing, and hardware coverage.

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

From the title screen, press Start/A, choose New Game, advance the intro text, enter the truck, walk right to exit, then move around Littleroot with the arrow keys.

## Deterministic Milestone Check

The local development run used save states to verify the final milestone. State files are not committed because they may contain ROM-derived data. If you have generated compatible local states, the check looks like this:

```bash
target/release/vibe-gba "/path/to/Pokemon Emerald.gba" \
  --load-state states/truck_palette_60.bin \
  --frames 1200 \
  --input-script 0:40:right \
  --save-state /tmp/littleroot_entry.bin \
  --screenshot /tmp/littleroot_entry.png \
  --dump-state
```

Expected debug markers:

- `layoutId=000a`
- `callback2=08085e5d`
- `PLAYER_OBJECT` reports `map=0.9`
- The screenshot is Littleroot Town with the player visible.

Then verify movement:

```bash
target/release/vibe-gba "/path/to/Pokemon Emerald.gba" \
  --load-state /tmp/littleroot_entry.bin \
  --frames 700 \
  --input-script 0:32:down,120:64:right \
  --screenshot /tmp/littleroot_walk.png \
  --dump-state
```

The player coordinates and visible OAM position should move while staying on `layoutId=000a`.

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

- `src/cpu.rs`: ARM/Thumb CPU core plus the Emerald milestone HLE hooks.
- `src/bus.rs`: memory map, IO, DMA, timers, save memory, and state serialization.
- `src/ppu.rs`: framebuffer rendering for bitmap/text/affine backgrounds, OBJ sprites, and blending.
- `src/cartridge.rs`: raw and zipped ROM loading plus flash save behavior.
- `src/gba.rs`: emulator orchestration, frame stepping, save states, and debug summaries.
- `src/main.rs`: CLI, keyboard input, window loop, screenshots, and scripted input.

The original development prompt is preserved in [PROMPT.md](PROMPT.md).

## License

MIT. See [LICENSE](LICENSE).
