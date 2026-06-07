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

    pub fn set_trace(&mut self, trace: bool) {
        self.cpu.set_trace(trace);
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
            0x0203_75c8,
            0x0203_7fd4,
            0x0203_7fe4,
            0x0203_7318,
            0x0203_7350,
            0x0203_7590,
            0x0300_0e38,
            0x0300_0f2c,
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
            0x0300_7420,
        ] {
            let _ = writeln!(out, "MEM {base:08x}:");
            for i in 0..8 {
                let addr = base + i * 4;
                let _ = writeln!(out, "  {addr:08x}: {:08x}", self.bus.read32(addr));
            }
        }
        self.write_field_debug(&mut out);
        self.write_script_debug(&mut out);
        self.write_sound_debug(&mut out);
        self.write_palette_debug(&mut out);
        let _ = writeln!(out, "SPRITES:");
        for sprite_id in 0..64 {
            let base = 0x0202_0630 + sprite_id * 0x44;
            let flags = self.bus.read16(base + 0x3e);
            if flags & 1 == 0 {
                continue;
            }
            let _ = writeln!(
                out,
                "  spr#{sprite_id:02} attr={:04x}/{:04x}/{:04x} cb={:08x} pos=({}, {}) center=({}, {}) data0={} data1={} flags={:04x} [{}] subprio={}",
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
                sprite_flags(flags),
                self.bus.read8(base + 0x43),
            );
            let _ = writeln!(
                out,
                "       anims={:08x} images={:08x} affine={:08x} template={:08x} subsprites={:08x} anim={}/{} delay={} loop={} table={} mode={} sheetStart={}",
                self.bus.read32(base + 0x08),
                self.bus.read32(base + 0x0c),
                self.bus.read32(base + 0x10),
                self.bus.read32(base + 0x14),
                self.bus.read32(base + 0x18),
                self.bus.read8(base + 0x2a),
                self.bus.read8(base + 0x2b),
                self.bus.read8(base + 0x2c) & 0x3f,
                self.bus.read8(base + 0x2d),
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
        const G_MAIN: u32 = 0x0300_22c0;
        const OBJECT_EVENT_SIZE: u32 = 0x24;

        let _ = writeln!(
            out,
            "GMAIN cb1={:08x} cb2={:08x} saved={:08x} vblank={:08x} hblank={:08x} state={} heldRaw={:04x} newRaw={:04x} held={:04x} new={:04x} repeat={:04x} repeatCounter={:04x}",
            self.bus.read32(G_MAIN),
            self.bus.read32(G_MAIN + 4),
            self.bus.read32(G_MAIN + 8),
            self.bus.read32(G_MAIN + 0x0c),
            self.bus.read32(G_MAIN + 0x10),
            self.bus.read8(G_MAIN + 0x438),
            self.bus.read16(G_MAIN + 0x28),
            self.bus.read16(G_MAIN + 0x2a),
            self.bus.read16(G_MAIN + 0x2c),
            self.bus.read16(G_MAIN + 0x2e),
            self.bus.read16(G_MAIN + 0x30),
            self.bus.read16(G_MAIN + 0x32),
        );

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
        let player_flags = self.bus.read8(G_PLAYER_AVATAR);
        let tile_transition = self.bus.read8(G_PLAYER_AVATAR + 3);
        let held_keys = self.bus.read16(G_MAIN + 0x2c);
        let input_direction = field_direction_from_keys(held_keys);
        let _ = writeln!(
            out,
            "PLAYER_AVATAR flags={:02x} [{}] trans={:02x} run={} tile={} sprite={} obj={} prevent={} gender={} bikeState={} newDir={} bikeCounter={} bikeSpeed={}",
            player_flags,
            player_avatar_flags(player_flags),
            self.bus.read8(G_PLAYER_AVATAR + 1),
            self.bus.read8(G_PLAYER_AVATAR + 2),
            tile_transition,
            self.bus.read8(G_PLAYER_AVATAR + 4),
            player_obj_id,
            self.bus.read8(G_PLAYER_AVATAR + 6),
            self.bus.read8(G_PLAYER_AVATAR + 7),
            self.bus.read8(G_PLAYER_AVATAR + 8),
            self.bus.read8(G_PLAYER_AVATAR + 9),
            self.bus.read8(G_PLAYER_AVATAR + 10),
            self.bus.read8(G_PLAYER_AVATAR + 11),
        );
        let _ = writeln!(
            out,
            "PLAYER_INPUT heldDir={} dpadDir={} tileReady={} controllable={} preventStep={}",
            (held_keys & 0x00f0) != 0,
            input_direction,
            matches!(tile_transition, 0 | 2),
            player_flags & 0x20 != 0,
            self.bus.read8(G_PLAYER_AVATAR + 6) != 0,
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
        self.write_save_progress_debug(out);
    }

    fn write_palette_debug(&self, out: &mut String) {
        use std::fmt::Write;

        const G_PALETTE_FADE: u32 = 0x0203_7fd4;
        const G_PLTT_BUFFER_UNFADED: u32 = 0x0203_7714;
        const G_PLTT_BUFFER_FADED: u32 = 0x0203_7b14;
        const S_PLTT_BUFFER_TRANSFER_PENDING: u32 = 0x0203_7fe4;

        let coeffs = self.bus.read16(G_PALETTE_FADE + 4);
        let blend = self.bus.read16(G_PALETTE_FADE + 6);
        let flags0 = self.bus.read8(G_PALETTE_FADE + 8);
        let flags = u32::from_le_bytes([
            self.bus.read8(G_PALETTE_FADE + 8),
            self.bus.read8(G_PALETTE_FADE + 9),
            self.bus.read8(G_PALETTE_FADE + 10),
            self.bus.read8(G_PALETTE_FADE + 11),
        ]);

        let active = (blend & 0x8000) != 0;
        let mode = (flags >> 8) & 0x03;
        let sw_counter = (flags >> 12) & 0x1f;
        let software_finishing = (flags >> 17) & 1 != 0;
        let obj_toggle = (flags >> 18) & 1 != 0;
        let delta_y = (flags >> 19) & 0x0f;
        let _ = write!(
            out,
            "PALFADE base={G_PALETTE_FADE:08x} selected={:08x} transferPending={:08x} delay={} y={} target={} blend={:04x} active={} delay2={} yDec={} bufferOff={} mode={} hardFinish={} swCounter={} swFinish={} objToggle={} deltaY={} raw",
            self.bus.read32(G_PALETTE_FADE),
            self.bus.read32(S_PLTT_BUFFER_TRANSFER_PENDING),
            coeffs & 0x003f,
            (coeffs >> 6) & 0x001f,
            (coeffs >> 11) & 0x001f,
            blend & 0x7fff,
            active,
            flags0 & 0x3f,
            (flags0 >> 6) & 1,
            (flags0 >> 7) & 1,
            mode,
            (flags >> 11) & 1,
            sw_counter,
            software_finishing,
            obj_toggle,
            delta_y,
        );
        for i in 0..12 {
            let _ = write!(out, " {:02x}", self.bus.read8(G_PALETTE_FADE + i));
        }
        let _ = writeln!(out);

        let _ = write!(out, "PLTTBUF unfaded");
        for i in 0..8 {
            let _ = write!(
                out,
                " {:04x}",
                self.bus.read16(G_PLTT_BUFFER_UNFADED + i * 2)
            );
        }
        let _ = write!(out, " faded");
        for i in 0..8 {
            let _ = write!(out, " {:04x}", self.bus.read16(G_PLTT_BUFFER_FADED + i * 2));
        }
        let _ = writeln!(out);
    }

    fn write_script_debug(&self, out: &mut String) {
        use std::fmt::Write;

        const S_GLOBAL_SCRIPT_CONTEXT_STATUS: u32 = 0x0300_0e38;
        const S_GLOBAL_SCRIPT_CONTEXT: u32 = 0x0300_0e40;
        const S_LOCK_FIELD_CONTROLS: u32 = 0x0300_0f2c;
        const S_PAUSE_COUNTER: u32 = 0x0203_75c8;
        let status = self.bus.read8(S_GLOBAL_SCRIPT_CONTEXT_STATUS);
        let lock = self.bus.read8(S_LOCK_FIELD_CONTROLS);
        let ctx0 = self.bus.read32(S_GLOBAL_SCRIPT_CONTEXT);
        let stack_depth = ctx0 & 0xff;
        let mode = (ctx0 >> 8) & 0xff;
        let comparison = (ctx0 >> 16) & 0xff;
        let native_ptr = self.bus.read32(S_GLOBAL_SCRIPT_CONTEXT + 4);
        let script_ptr = self.bus.read32(S_GLOBAL_SCRIPT_CONTEXT + 8);
        let stack0 = self.bus.read32(S_GLOBAL_SCRIPT_CONTEXT + 12);
        let stack1 = self.bus.read32(S_GLOBAL_SCRIPT_CONTEXT + 16);
        let cmd_table = self.bus.read32(S_GLOBAL_SCRIPT_CONTEXT + 0x5c);
        let cmd_table_end = self.bus.read32(S_GLOBAL_SCRIPT_CONTEXT + 0x60);
        let pause_counter = self.bus.read16(S_PAUSE_COUNTER);
        let _ = writeln!(
            out,
            "SCRIPT globalStatus={} lockFieldControls={} mode={} stackDepth={} cmp={} native={:08x} scriptPtr={:08x} stack0={:08x} stack1={:08x} cmdTable={:08x} cmdEnd={:08x} pauseCounter={} ctxWords={:08x} {:08x} {:08x} {:08x}",
            status,
            lock,
            mode,
            stack_depth,
            comparison,
            native_ptr,
            script_ptr,
            stack0,
            stack1,
            cmd_table,
            cmd_table_end,
            pause_counter,
            self.bus.read32(S_GLOBAL_SCRIPT_CONTEXT_STATUS),
            self.bus.read32(S_GLOBAL_SCRIPT_CONTEXT_STATUS + 4),
            self.bus.read32(S_GLOBAL_SCRIPT_CONTEXT_STATUS + 8),
            self.bus.read32(S_LOCK_FIELD_CONTROLS),
        );
    }

    fn write_sound_debug(&self, out: &mut String) {
        use std::fmt::Write;

        const G_MPLAY_INFO_BGM: u32 = 0x0300_7420;
        const MUSIC_PLAYER_TRACK_SIZE: u32 = 0x50;
        let status = self.bus.read32(G_MPLAY_INFO_BGM + 4);
        let track_count = self.bus.read8(G_MPLAY_INFO_BGM + 8);
        let fade_oi = self.bus.read16(G_MPLAY_INFO_BGM + 0x24);
        let fade_oc = self.bus.read16(G_MPLAY_INFO_BGM + 0x26);
        let fade_ov = self.bus.read16(G_MPLAY_INFO_BGM + 0x28);
        let tracks = self.bus.read32(G_MPLAY_INFO_BGM + 0x2c);
        let _ = write!(
            out,
            "SOUND BGM status={status:08x} trackBits={:04x} paused={} trackCount={} fadeOI={} fadeOC={} fadeOV={:04x} tracks={tracks:08x} flags",
            status & 0xffff,
            status & 0x8000_0000 != 0,
            track_count,
            fade_oi,
            fade_oc,
            fade_ov,
        );
        if (0x0200_0000..0x0204_0000).contains(&tracks)
            || (0x0300_0000..0x0300_8000).contains(&tracks)
        {
            for idx in 0..track_count.min(8) {
                let flag = self
                    .bus
                    .read8(tracks + u32::from(idx) * MUSIC_PLAYER_TRACK_SIZE);
                let _ = write!(out, " {flag:02x}");
            }
        }
        let _ = writeln!(out);
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
            "  obj#{object_id:02} flags={flags:08x} [{}] sprite={} gfx={} moveType={} local={} map={}.{} elev={}/{} init=({}, {}) cur=({}, {}) prev=({}, {}) face={} moveDir={} range={:02x} action={} metatile={:02x}/{:02x} copyMove={:02x}",
            object_event_flags(flags),
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

    fn write_save_progress_debug(&self, out: &mut String) {
        use std::fmt::Write;

        const GSAVEBLOCK1_PTR: u32 = 0x0300_5d8c;
        const G_PLAYER_PARTY_COUNT: u32 = 0x0202_44e9;
        const G_PLAYER_PARTY: u32 = 0x0202_44ec;
        const VAR_STARTER_MON: u16 = 0x4023;
        const VAR_LITTLEROOT_TOWN_STATE: u16 = 0x4050;
        const VAR_ROUTE101_STATE: u16 = 0x4060;
        const VAR_LITTLEROOT_HOUSES_STATE_MAY: u16 = 0x4082;
        const VAR_BIRCH_LAB_STATE: u16 = 0x4084;
        const VAR_LITTLEROOT_HOUSES_STATE_BRENDAN: u16 = 0x408c;
        const VAR_LITTLEROOT_RIVAL_STATE: u16 = 0x408d;
        const VAR_LITTLEROOT_INTRO_STATE: u16 = 0x4092;
        const FLAG_RESCUED_BIRCH: u16 = 0x052;
        const FLAG_HIDE_ROUTE_101_BIRCH_STARTERS_BAG: u16 = 0x2bc;
        const FLAG_HIDE_ROUTE_101_BIRCH_ZIGZAGOON_BATTLE: u16 = 0x2d0;
        const FLAG_HIDE_LITTLEROOT_TOWN_BIRCHS_LAB_BIRCH: u16 = 0x2d1;
        const FLAG_HIDE_LITTLEROOT_TOWN_MAYS_HOUSE_RIVAL_BEDROOM: u16 = 0x2d2;
        const FLAG_HIDE_LITTLEROOT_TOWN_MAYS_HOUSE_MAY: u16 = 0x2ea;
        const FLAG_HIDE_ROUTE_101_BOY: u16 = 0x3df;
        const FLAG_HIDE_LITTLEROOT_TOWN_MAYS_HOUSE_2F_POKE_BALL: u16 = 0x332;
        const FLAG_SYS_POKEMON_GET: u16 = 0x860;

        let save = self.bus.read32(GSAVEBLOCK1_PTR);
        if !(0x0200_0000..0x0204_0000).contains(&save) {
            return;
        }

        let party_mon = G_PLAYER_PARTY;
        let party_personality = self.bus.read32(party_mon);
        let party_ot_id = self.bus.read32(party_mon + 4);
        let party_key = party_personality ^ party_ot_id;
        let party_sub0 = pokemon_substruct_index(party_personality, 0) as u32;
        let party_sub0_word0 = self.bus.read32(party_mon + 0x20 + party_sub0 * 12) ^ party_key;
        let party_species = party_sub0_word0 as u16;

        let _ = writeln!(
            out,
            "SAVE save1={save:08x} route101={} birch_lab={} starter={}",
            save_var(&self.bus, save, VAR_ROUTE101_STATE),
            save_var(&self.bus, save, VAR_BIRCH_LAB_STATE),
            save_var(&self.bus, save, VAR_STARTER_MON),
        );
        let _ = writeln!(
            out,
            "PARTY count={} species={} level={} hp={}/{} checksum={:04x}",
            self.bus.read8(G_PLAYER_PARTY_COUNT),
            party_species,
            self.bus.read8(party_mon + 0x54),
            self.bus.read16(party_mon + 0x56),
            self.bus.read16(party_mon + 0x58),
            self.bus.read16(party_mon + 0x1c),
        );
        let _ = writeln!(
            out,
            "SAVE_LITTLEROOT town={} rival={} intro={} house_may={} house_brendan={}",
            save_var(&self.bus, save, VAR_LITTLEROOT_TOWN_STATE),
            save_var(&self.bus, save, VAR_LITTLEROOT_RIVAL_STATE),
            save_var(&self.bus, save, VAR_LITTLEROOT_INTRO_STATE),
            save_var(&self.bus, save, VAR_LITTLEROOT_HOUSES_STATE_MAY),
            save_var(&self.bus, save, VAR_LITTLEROOT_HOUSES_STATE_BRENDAN),
        );
        let _ = writeln!(
            out,
            "SAVE_FLAGS pokemon_get={} rescued_birch={} hide_bag={} hide_zigzagoon={} hide_lab_birch={} route101_boy={} hide_may_1f={} hide_may_2f={} hide_may_ball={}",
            save_flag(&self.bus, save, FLAG_SYS_POKEMON_GET),
            save_flag(&self.bus, save, FLAG_RESCUED_BIRCH),
            save_flag(&self.bus, save, FLAG_HIDE_ROUTE_101_BIRCH_STARTERS_BAG),
            save_flag(&self.bus, save, FLAG_HIDE_ROUTE_101_BIRCH_ZIGZAGOON_BATTLE),
            save_flag(&self.bus, save, FLAG_HIDE_LITTLEROOT_TOWN_BIRCHS_LAB_BIRCH),
            save_flag(&self.bus, save, FLAG_HIDE_ROUTE_101_BOY),
            save_flag(&self.bus, save, FLAG_HIDE_LITTLEROOT_TOWN_MAYS_HOUSE_MAY),
            save_flag(
                &self.bus,
                save,
                FLAG_HIDE_LITTLEROOT_TOWN_MAYS_HOUSE_RIVAL_BEDROOM,
            ),
            save_flag(&self.bus, save, FLAG_HIDE_LITTLEROOT_TOWN_MAYS_HOUSE_2F_POKE_BALL),
        );
    }
}

fn player_avatar_flags(flags: u8) -> String {
    let mut names = Vec::new();
    if flags & 0x01 != 0 {
        names.push("foot");
    }
    if flags & 0x02 != 0 {
        names.push("machBike");
    }
    if flags & 0x04 != 0 {
        names.push("acroBike");
    }
    if flags & 0x08 != 0 {
        names.push("surf");
    }
    if flags & 0x10 != 0 {
        names.push("underwater");
    }
    if flags & 0x20 != 0 {
        names.push("controllable");
    }
    if flags & 0x40 != 0 {
        names.push("forced");
    }
    if flags & 0x80 != 0 {
        names.push("dash");
    }
    if names.is_empty() {
        "-".to_string()
    } else {
        names.join("|")
    }
}

fn object_event_flags(flags: u32) -> String {
    const FLAGS: &[(u32, &str)] = &[
        (1 << 0, "active"),
        (1 << 1, "singleMove"),
        (1 << 2, "groundMove"),
        (1 << 3, "groundStop"),
        (1 << 4, "noCoverFx"),
        (1 << 5, "landingJump"),
        (1 << 6, "heldMove"),
        (1 << 7, "heldDone"),
        (1 << 8, "frozen"),
        (1 << 9, "faceLocked"),
        (1 << 10, "animOff"),
        (1 << 11, "animOn"),
        (1 << 12, "inanimate"),
        (1 << 13, "invisible"),
        (1 << 14, "offscreen"),
        (1 << 15, "camera"),
        (1 << 16, "player"),
        (1 << 17, "reflection"),
        (1 << 18, "shortGrass"),
        (1 << 19, "shallowWater"),
        (1 << 20, "sandPile"),
        (1 << 21, "hotSprings"),
        (1 << 22, "shadow"),
        (1 << 23, "spritePause"),
        (1 << 24, "affinePause"),
        (1 << 25, "noJumpFx"),
        (1 << 26, "fixedPrio"),
        (1 << 27, "hideReflection"),
    ];

    let mut names = Vec::new();
    for (bit, name) in FLAGS {
        if flags & bit != 0 {
            names.push(*name);
        }
    }
    if names.is_empty() {
        "-".to_string()
    } else {
        names.join("|")
    }
}

fn sprite_flags(flags: u16) -> String {
    const FLAGS: &[(u16, &str)] = &[
        (1 << 0, "inUse"),
        (1 << 1, "coordOffset"),
        (1 << 2, "invisible"),
        (1 << 8, "hFlip"),
        (1 << 9, "vFlip"),
        (1 << 10, "animBegin"),
        (1 << 11, "affineBegin"),
        (1 << 12, "animEnd"),
        (1 << 13, "affineEnd"),
        (1 << 14, "usingSheet"),
        (1 << 15, "anchored"),
    ];

    let mut names = Vec::new();
    for (bit, name) in FLAGS {
        if flags & bit != 0 {
            names.push(*name);
        }
    }
    if names.is_empty() {
        "-".to_string()
    } else {
        names.join("|")
    }
}

fn field_direction_from_keys(keys: u16) -> u8 {
    if keys & 0x0040 != 0 {
        2
    } else if keys & 0x0080 != 0 {
        1
    } else if keys & 0x0020 != 0 {
        3
    } else if keys & 0x0010 != 0 {
        4
    } else {
        0
    }
}

fn valid_pc(pc: u32) -> bool {
    pc <= 0x0dff_ffff
}

fn save_var(bus: &Bus, save: u32, var: u16) -> u16 {
    const VARS_START: u16 = 0x4000;
    const VARS_OFFSET: u32 = 0x139c;

    if var >= VARS_START {
        bus.read16(save + VARS_OFFSET + (var - VARS_START) as u32 * 2)
    } else {
        0
    }
}

fn save_flag(bus: &Bus, save: u32, flag: u16) -> bool {
    const FLAGS_OFFSET: u32 = 0x1270;

    let byte = bus.read8(save + FLAGS_OFFSET + (flag as u32 / 8));
    byte & (1u8 << (flag & 7)) != 0
}

fn pokemon_substruct_index(personality: u32, substruct_type: usize) -> usize {
    const ORDERS: [[usize; 4]; 24] = [
        [0, 1, 2, 3],
        [0, 1, 3, 2],
        [0, 2, 1, 3],
        [0, 3, 1, 2],
        [0, 2, 3, 1],
        [0, 3, 2, 1],
        [1, 0, 2, 3],
        [1, 0, 3, 2],
        [2, 0, 1, 3],
        [3, 0, 1, 2],
        [2, 0, 3, 1],
        [3, 0, 2, 1],
        [1, 2, 0, 3],
        [1, 3, 0, 2],
        [2, 1, 0, 3],
        [3, 1, 0, 2],
        [2, 3, 0, 1],
        [3, 2, 0, 1],
        [1, 2, 3, 0],
        [1, 3, 2, 0],
        [2, 1, 3, 0],
        [3, 1, 2, 0],
        [2, 3, 1, 0],
        [3, 2, 1, 0],
    ];

    ORDERS[(personality % 24) as usize][substruct_type]
}
