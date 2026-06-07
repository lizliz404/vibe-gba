use std::path::PathBuf;

use crate::bus::Bus;
use crate::cartridge::Flash;
use crate::cpu::Cpu;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Gba {
    cpu: Cpu,
    bus: Bus,
}

#[derive(Clone, Copy)]
pub enum Button {
    A = 0,
    B = 1,
    Select = 2,
    Start = 3,
    Right = 4,
    Left = 5,
    Up = 6,
    Down = 7,
    R = 8,
    L = 9,
}

impl Gba {
    pub fn new(rom: Vec<u8>, save_path: Option<PathBuf>, trace: bool) -> Self {
        let flash = Flash::new(save_path);
        Self {
            cpu: Cpu::new(trace),
            bus: Bus::new(rom, flash),
        }
    }

    pub fn load_state(
        path: impl AsRef<std::path::Path>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(path)?;
        Ok(bincode::deserialize(&bytes)?)
    }

    pub fn save_state(
        &self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let bytes = bincode::serialize(self)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    pub fn run_frame(&mut self) {
        let target = self.bus.frame_count().wrapping_add(1);
        let mut guard = 0u64;
        while self.bus.frame_count() != target {
            self.step_once();
            guard += 1;
            if guard > 20_000_000 {
                eprintln!("frame guard tripped at pc={:08x}", self.cpu.pc());
                break;
            }
        }
    }

    pub fn run_until_pc(&mut self, pc: u32, max_steps: u64) -> bool {
        self.run_until_pc_hit(pc, 1, max_steps)
    }

    pub fn run_until_pc_hit(&mut self, pc: u32, hit_target: u64, max_steps: u64) -> bool {
        let mut hits = 0;
        for _ in 0..max_steps {
            if self.cpu.pc() == pc {
                hits += 1;
                if hits >= hit_target {
                    return true;
                }
            }
            self.step_once();
        }
        false
    }

    pub fn run_until_invalid(&mut self, max_steps: u64) -> bool {
        for _ in 0..max_steps {
            if !valid_pc(self.cpu.pc()) {
                return true;
            }
            self.step_once();
        }
        false
    }

    fn step_once(&mut self) {
        let cycles = self.cpu.step(&mut self.bus);
        self.bus.step(cycles.max(1));
    }

    pub fn framebuffer(&self) -> &[u32] {
        self.bus.framebuffer()
    }

    pub fn set_button(&mut self, button: Button, pressed: bool) {
        self.bus.set_button(button as u16, pressed);
    }

    pub fn flush_save(&mut self) -> std::io::Result<()> {
        self.bus.flush_save()
    }

    pub fn flush_save_if_dirty(&mut self) -> std::io::Result<()> {
        self.bus.flush_save_if_dirty()
    }

    pub fn debug_summary(&self) -> String {
        let mut out = format!("{}\n{}", self.cpu.debug_summary(), self.bus.debug_summary());
        use std::fmt::Write;
        let pc = self.cpu.pc();
        let _ = writeln!(out, "PC words:");
        for i in 0..8 {
            let addr = pc.wrapping_sub(16).wrapping_add(i * 4);
            let _ = writeln!(out, "  {addr:08x}: {:08x}", self.bus.read32(addr));
        }
        let sp = self.cpu.sp();
        let _ = writeln!(out, "SP words:");
        for i in 0..20 {
            let addr = sp.wrapping_add(i * 4);
            let _ = writeln!(out, "  {addr:08x}: {:08x}", self.bus.read32(addr));
        }
        for base in [
            0x0203_7fd4,
            0x0203_7fe4,
            0x0203_7318,
            0x0203_7350,
            0x0203_7590,
            0x0300_22b4,
            0x0300_22c0,
            0x0300_22e0,
            0x0300_26f0,
            0x0300_27f0,
            0x0300_3090,
            0x0300_30fc,
            0x0300_3110,
            0x0300_3144,
            0x0300_3170,
        ] {
            let _ = writeln!(out, "MEM {base:08x}:");
            for i in 0..8 {
                let addr = base + i * 4;
                let _ = writeln!(out, "  {addr:08x}: {:08x}", self.bus.read32(addr));
            }
        }
        self.write_field_debug(&mut out);
        let _ = writeln!(out, "SPRITES:");
        for sprite_id in 0..6 {
            let base = 0x0202_0630 + sprite_id * 0x44;
            let flags = self.bus.read16(base + 0x3e);
            if flags & 1 == 0 {
                continue;
            }
            let _ = writeln!(
                out,
                "  spr#{sprite_id:02} attr={:04x}/{:04x}/{:04x} cb={:08x} pos=({}, {}) center=({}, {}) data0={} data1={} flags={:04x} subprio={}",
                self.bus.read16(base),
                self.bus.read16(base + 2),
                self.bus.read16(base + 4),
                self.bus.read32(base + 0x1c),
                self.bus.read16(base + 0x20) as i16,
                self.bus.read16(base + 0x22) as i16,
                self.bus.read8(base + 0x28) as i8,
                self.bus.read8(base + 0x29) as i8,
                self.bus.read16(base + 0x2e) as i16,
                self.bus.read16(base + 0x30) as i16,
                flags,
                self.bus.read8(base + 0x43),
            );
            let _ = writeln!(
                out,
                "       subsprites={:08x} table={} mode={} sheetStart={}",
                self.bus.read32(base + 0x18),
                self.bus.read8(base + 0x42) & 0x3f,
                self.bus.read8(base + 0x42) >> 6,
                self.bus.read16(base + 0x40),
            );
        }
        let _ = writeln!(out, "OAM:");
        for obj in 0..64 {
            let base = 0x0700_0000 + obj * 8;
            let attr0 = self.bus.read16(base);
            let attr1 = self.bus.read16(base + 2);
            let attr2 = self.bus.read16(base + 4);
            if attr0 == 0 && attr1 == 0 && attr2 == 0 {
                continue;
            }
            let _ = writeln!(
                out,
                "  obj#{obj:02} attr={attr0:04x}/{attr1:04x}/{attr2:04x} xy=({}, {})",
                attr1 & 0x01ff,
                attr0 & 0x00ff,
            );
        }
        let _ = writeln!(out, "TASKS:");
        for task_id in 0..16 {
            let base = 0x0300_5e00 + task_id * 40;
            let func = self.bus.read32(base);
            let active = self.bus.read8(base + 4);
            if active == 0 && func == 0 {
                continue;
            }
            let _ = write!(
                out,
                "  #{task_id:02} func={func:08x} active={} prio={} data",
                active,
                self.bus.read8(base + 7)
            );
            for i in 0..8 {
                let _ = write!(out, " {:04x}", self.bus.read16(base + 8 + i * 2));
            }
            let _ = writeln!(out);
        }
        out
    }

    fn write_field_debug(&self, out: &mut String) {
        use std::fmt::Write;

        const G_MAP_HEADER: u32 = 0x0203_7318;
        const G_OBJECT_EVENTS: u32 = 0x0203_7350;
        const G_PLAYER_AVATAR: u32 = 0x0203_7590;
        const OBJECT_EVENT_SIZE: u32 = 0x24;

        let _ = writeln!(
            out,
            "FIELD map layout={:08x} events={:08x} scripts={:08x} conns={:08x} music={:04x} layoutId={:04x} region={} cave={} weather={} type={} battle={}",
            self.bus.read32(G_MAP_HEADER),
            self.bus.read32(G_MAP_HEADER + 4),
            self.bus.read32(G_MAP_HEADER + 8),
            self.bus.read32(G_MAP_HEADER + 12),
            self.bus.read16(G_MAP_HEADER + 16),
            self.bus.read16(G_MAP_HEADER + 18),
            self.bus.read8(G_MAP_HEADER + 20),
            self.bus.read8(G_MAP_HEADER + 21),
            self.bus.read8(G_MAP_HEADER + 22),
            self.bus.read8(G_MAP_HEADER + 23),
            self.bus.read8(G_MAP_HEADER + 27),
        );

        let player_obj_id = self.bus.read8(G_PLAYER_AVATAR + 5);
        let player_obj = G_OBJECT_EVENTS + player_obj_id as u32 * OBJECT_EVENT_SIZE;
        let _ = writeln!(
            out,
            "PLAYER_AVATAR flags={:02x} trans={:02x} run={} tile={} sprite={} obj={} prevent={} gender={} bikeState={} newDir={} bikeCounter={} bikeSpeed={}",
            self.bus.read8(G_PLAYER_AVATAR),
            self.bus.read8(G_PLAYER_AVATAR + 1),
            self.bus.read8(G_PLAYER_AVATAR + 2),
            self.bus.read8(G_PLAYER_AVATAR + 3),
            self.bus.read8(G_PLAYER_AVATAR + 4),
            player_obj_id,
            self.bus.read8(G_PLAYER_AVATAR + 6),
            self.bus.read8(G_PLAYER_AVATAR + 7),
            self.bus.read8(G_PLAYER_AVATAR + 8),
            self.bus.read8(G_PLAYER_AVATAR + 9),
            self.bus.read8(G_PLAYER_AVATAR + 10),
            self.bus.read8(G_PLAYER_AVATAR + 11),
        );
        let _ = writeln!(out, "PLAYER_OBJECT:");
        self.write_object_event(out, player_obj_id, player_obj);

        let _ = writeln!(out, "OBJECT_EVENTS:");
        for object_id in 0..16u8 {
            let base = G_OBJECT_EVENTS + object_id as u32 * OBJECT_EVENT_SIZE;
            let flags = self.bus.read32(base);
            if flags & 1 != 0 || object_id == player_obj_id {
                self.write_object_event(out, object_id, base);
            }
        }
    }

    fn write_object_event(&self, out: &mut String, object_id: u8, base: u32) {
        use std::fmt::Write;

        let flags = self.bus.read32(base);
        let initial_x = self.bus.read16(base + 0x0c) as i16;
        let initial_y = self.bus.read16(base + 0x0e) as i16;
        let current_x = self.bus.read16(base + 0x10) as i16;
        let current_y = self.bus.read16(base + 0x12) as i16;
        let previous_x = self.bus.read16(base + 0x14) as i16;
        let previous_y = self.bus.read16(base + 0x16) as i16;
        let directions = self.bus.read16(base + 0x18);
        let elevation = self.bus.read8(base + 0x0b);
        let _ = writeln!(
            out,
            "  obj#{object_id:02} flags={flags:08x} sprite={} gfx={} moveType={} local={} map={}.{} elev={}/{} init=({}, {}) cur=({}, {}) prev=({}, {}) face={} moveDir={} range={:02x} action={} metatile={:02x}/{:02x} copyMove={:02x}",
            self.bus.read8(base + 4),
            self.bus.read8(base + 5),
            self.bus.read8(base + 6),
            self.bus.read8(base + 8),
            self.bus.read8(base + 10),
            self.bus.read8(base + 9),
            elevation & 0x0f,
            elevation >> 4,
            initial_x,
            initial_y,
            current_x,
            current_y,
            previous_x,
            previous_y,
            directions & 0xf,
            (directions >> 4) & 0xf,
            self.bus.read8(base + 0x19),
            self.bus.read8(base + 0x1c),
            self.bus.read8(base + 0x1e),
            self.bus.read8(base + 0x1f),
            self.bus.read8(base + 0x22),
        );
    }
}

fn valid_pc(pc: u32) -> bool {
    pc <= 0x0dff_ffff
}
