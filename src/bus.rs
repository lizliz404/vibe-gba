use crate::cartridge::Flash;
use crate::ppu::Ppu;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

const EWRAM_SIZE: usize = 256 * 1024;
const IWRAM_SIZE: usize = 32 * 1024;
const PRAM_SIZE: usize = 1024;
const VRAM_SIZE: usize = 96 * 1024;
const OAM_SIZE: usize = 1024;

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
struct Timer {
    counter: u16,
    reload: u16,
    control: u16,
    divider: u32,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
struct IoWrite {
    frame: u64,
    off: usize,
    size: usize,
    value: u32,
}

#[derive(Deserialize, Serialize)]
pub struct Bus {
    rom: Vec<u8>,
    ewram: Vec<u8>,
    iwram: Vec<u8>,
    pram: Vec<u8>,
    vram: Vec<u8>,
    oam: Vec<u8>,
    #[serde(with = "BigArray")]
    io: [u8; 0x400],
    ppu: Ppu,
    timers: [Timer; 4],
    dma_src: [u32; 4],
    dma_dst: [u32; 4],
    flash: Flash,
    frame: u64,
    io_log: Vec<IoWrite>,
}

impl Bus {
    pub fn new(mut rom: Vec<u8>, flash: Flash) -> Self {
        if rom.is_empty() {
            rom.push(0xff);
        }
        let mut io = [0u8; 0x400];
        write_io_raw16(&mut io, 0x130, 0x03ff);
        write_io_raw16(&mut io, 0x204, 0x4317);
        io[0x300] = 1;
        Self {
            rom,
            ewram: vec![0; EWRAM_SIZE],
            iwram: vec![0; IWRAM_SIZE],
            pram: vec![0; PRAM_SIZE],
            vram: vec![0; VRAM_SIZE],
            oam: vec![0; OAM_SIZE],
            io,
            ppu: Ppu::new(),
            timers: [Timer::default(); 4],
            dma_src: [0; 4],
            dma_dst: [0; 4],
            flash,
            frame: 0,
            io_log: Vec::with_capacity(64),
        }
    }

    pub fn frame_count(&self) -> u64 {
        self.frame
    }

    pub fn framebuffer(&self) -> &[u32] {
        &self.ppu.frame
    }

    pub fn debug_summary(&self) -> String {
        let mut out = String::new();
        use std::fmt::Write;
        let _ = writeln!(
            out,
            "BUS frame={} dispcnt={:04x} dispstat={:04x} vcount={} bg0={:04x} bg1={:04x} bg2={:04x} bg3={:04x}",
            self.frame,
            self.read_io16(0x000),
            self.read_io16(0x004),
            self.read_io16(0x006),
            self.read_io16(0x008),
            self.read_io16(0x00a),
            self.read_io16(0x00c),
            self.read_io16(0x00e)
        );
        let _ = writeln!(
            out,
            "IRQ ie={:04x} if={:04x} ime={:04x} key={:04x} waitcnt={:04x}",
            self.read_io16(0x200),
            self.read_io16(0x202),
            self.read_io16(0x208),
            self.read_io16(0x130),
            self.read_io16(0x204)
        );
        let _ = writeln!(
            out,
            "MEMSTAT pram_nz={} vram_nz={} oam_nz={} ewram_nz={} iwram_nz={}",
            self.pram.iter().filter(|&&b| b != 0).count(),
            self.vram.iter().filter(|&&b| b != 0).count(),
            self.oam.iter().filter(|&&b| b != 0).count(),
            self.ewram.iter().filter(|&&b| b != 0).count(),
            self.iwram.iter().filter(|&&b| b != 0).count()
        );
        let _ = write!(out, "PRAM");
        for i in 0..8 {
            let off = i * 2;
            let value = u16::from_le_bytes([self.pram[off], self.pram[off + 1]]);
            let _ = write!(out, " {value:04x}");
        }
        let _ = writeln!(out);
        let _ = write!(out, "VRAM0");
        for i in 0..8 {
            let off = i * 2;
            let value = u16::from_le_bytes([self.vram[off], self.vram[off + 1]]);
            let _ = write!(out, " {value:04x}");
        }
        let _ = writeln!(out);
        for base in [
            0x4000usize,
            0x8000,
            0xe000,
            0xe800,
            0xf000,
            0xf800,
            0x10000,
            0x14000,
        ] {
            let _ = write!(out, "VRAM{base:05x}");
            for i in 0..8 {
                let off = base + i * 2;
                let value = u16::from_le_bytes([self.vram[off], self.vram[off + 1]]);
                let _ = write!(out, " {value:04x}");
            }
            let _ = writeln!(out);
        }
        let _ = write!(out, "PRAM100");
        for i in 0..8 {
            let off = 0x100 + i * 2;
            let value = u16::from_le_bytes([self.pram[off], self.pram[off + 1]]);
            let _ = write!(out, " {value:04x}");
        }
        let _ = writeln!(out);
        let _ = write!(out, "VRAM_NZ");
        for (idx, value) in self
            .vram
            .iter()
            .enumerate()
            .filter(|(_, value)| **value != 0)
            .take(16)
        {
            let _ = write!(out, " {idx:05x}:{value:02x}");
        }
        let _ = writeln!(out);
        let _ = write!(out, "PRAM_NZ");
        for (idx, value) in self
            .pram
            .iter()
            .enumerate()
            .filter(|(_, value)| **value != 0)
            .take(16)
        {
            let _ = write!(out, " {idx:03x}:{value:02x}");
        }
        let _ = writeln!(out);
        let _ = writeln!(out, "IOLOG");
        for item in &self.io_log {
            let _ = writeln!(
                out,
                "  f={} off={:03x} size={} value={:08x}",
                item.frame, item.off, item.size, item.value
            );
        }
        for ch in 0..4 {
            let base = 0x0b0 + ch * 12;
            let _ = writeln!(
                out,
                "DMA{} sad={:08x} dad={:08x} cur={:08x}->{:08x} count={:04x} cnt={:04x}",
                ch,
                self.read_io32(base),
                self.read_io32(base + 4),
                self.dma_src[ch],
                self.dma_dst[ch],
                self.read_io16(base + 8),
                self.read_io16(base + 10)
            );
        }
        for idx in 0..4 {
            let base = 0x100 + idx * 4;
            let _ = writeln!(
                out,
                "TM{} count={:04x} reload={:04x} cnt={:04x}",
                idx,
                self.read_io16(base),
                self.timers[idx].reload,
                self.read_io16(base + 2)
            );
        }
        out
    }

    pub fn step(&mut self, cycles: u32) {
        self.step_timers(cycles);
        let events = self
            .ppu
            .step(cycles, &mut self.io, &self.vram, &self.pram, &self.oam);
        if events.hblank {
            self.run_dma_timing(2);
        }
        if events.vblank {
            self.iwram[0x22dc] |= 1;
            self.iwram[0x3171] = 5;
            if self.read_io16(0x004) & (1 << 3) != 0 {
                let ie = self.read_io16(0x200) | 1;
                write_io_raw16(&mut self.io, 0x200, ie);
            }
            self.process_emerald_dma3_requests();
            self.run_dma_timing(1);
        }
        if events.frame {
            self.frame = self.frame.wrapping_add(1);
        }
    }

    pub fn set_button(&mut self, bit: u16, pressed: bool) {
        let mut keys = self.read_io16(0x130);
        if pressed {
            keys &= !(1 << bit);
        } else {
            keys |= 1 << bit;
        }
        write_io_raw16(&mut self.io, 0x130, keys & 0x03ff);
        let keycnt = self.read_io16(0x132);
        if keycnt & (1 << 14) != 0 {
            let mask = keycnt & 0x03ff;
            let pressed_bits = (!keys) & 0x03ff;
            let hit = if keycnt & (1 << 15) != 0 {
                pressed_bits & mask == mask
            } else {
                pressed_bits & mask != 0
            };
            if hit {
                self.request_irq(1 << 12);
            }
        }
    }

    pub fn irq_pending(&self) -> u16 {
        if self.read_io16(0x208) & 1 == 0 {
            return 0;
        }
        self.read_io16(0x200) & self.read_io16(0x202)
    }

    pub fn wait_ready(&self, flags: u16) -> bool {
        self.read_io16(0x202) & flags != 0
    }

    pub fn clear_if(&mut self, flags: u16) {
        let value = self.read_io16(0x202) & !flags;
        write_io_raw16(&mut self.io, 0x202, value);
    }

    pub fn irq_handler(&self) -> u32 {
        self.read32(0x0300_7ffc)
    }

    pub fn flush_save(&mut self) -> std::io::Result<()> {
        self.flash.flush()
    }

    pub fn flush_save_if_dirty(&mut self) -> std::io::Result<()> {
        self.flash.flush_if_dirty()
    }

    pub fn read8(&self, addr: u32) -> u8 {
        match addr {
            0x0000_0000..=0x0000_3fff => 0,
            0x0200_0000..=0x02ff_ffff => self.ewram[addr as usize & (EWRAM_SIZE - 1)],
            0x0300_0000..=0x03ff_ffff => self.iwram[addr as usize & (IWRAM_SIZE - 1)],
            0x0400_0000..=0x04ff_ffff => self.read_io8((addr as usize) & 0x3ff),
            0x0500_0000..=0x05ff_ffff => self.pram[(addr as usize) & (PRAM_SIZE - 1)],
            0x0600_0000..=0x06ff_ffff => self.vram[vram_offset(addr)],
            0x0700_0000..=0x07ff_ffff => self.oam[(addr as usize) & (OAM_SIZE - 1)],
            0x0800_0000..=0x0dff_ffff => self.rom[rom_offset(addr, self.rom.len())],
            0x0e00_0000..=0x0eff_ffff => self.flash.read(addr),
            _ => 0,
        }
    }

    pub fn read16(&self, addr: u32) -> u16 {
        let a = addr & !1;
        u16::from_le_bytes([self.read8(a), self.read8(a + 1)])
    }

    pub fn read32(&self, addr: u32) -> u32 {
        let a = addr & !3;
        u32::from_le_bytes([
            self.read8(a),
            self.read8(a + 1),
            self.read8(a + 2),
            self.read8(a + 3),
        ])
    }

    pub fn write8(&mut self, addr: u32, value: u8) {
        match addr {
            0x0200_0000..=0x02ff_ffff => {
                let off = addr as usize & (EWRAM_SIZE - 1);
                self.ewram[off] = value;
            }
            0x0300_0000..=0x03ff_ffff => {
                let off = addr as usize & (IWRAM_SIZE - 1);
                self.iwram[off] = value;
            }
            0x0400_0000..=0x04ff_ffff => self.write_io8((addr as usize) & 0x3ff, value),
            0x0500_0000..=0x05ff_ffff => self.write_narrow16(addr, value, Region::Palette),
            0x0600_0000..=0x06ff_ffff => self.write_narrow16(addr, value, Region::Vram),
            0x0700_0000..=0x07ff_ffff => self.write_narrow16(addr, value, Region::Oam),
            0x0e00_0000..=0x0eff_ffff => self.flash.write(addr, value),
            _ => {}
        }
    }

    pub fn write16(&mut self, addr: u32, value: u16) {
        let a = addr & !1;
        if (0x0400_0000..=0x0400_03ff).contains(&a) {
            let off = (a as usize) & 0x3ff;
            self.log_io_write(off, 2, value as u32);
            self.write_io16_no_after(off, value);
            self.after_io_write(off, 2);
            return;
        }
        let bytes = value.to_le_bytes();
        self.write8_raw(a, bytes[0]);
        self.write8_raw(a + 1, bytes[1]);
    }

    pub fn write32(&mut self, addr: u32, value: u32) {
        let a = addr & !3;
        if (0x0400_0000..=0x0400_03ff).contains(&a) {
            let off = (a as usize) & 0x3ff;
            self.log_io_write(off, 4, value);
            self.write_io16_no_after(off, value as u16);
            self.write_io16_no_after(off + 2, (value >> 16) as u16);
            self.after_io_write(off, 4);
            return;
        }
        let bytes = value.to_le_bytes();
        for (i, byte) in bytes.into_iter().enumerate() {
            self.write8_raw(a + i as u32, byte);
        }
    }

    pub fn write8_raw(&mut self, addr: u32, value: u8) {
        match addr {
            0x0200_0000..=0x02ff_ffff => {
                let off = addr as usize & (EWRAM_SIZE - 1);
                self.ewram[off] = value;
            }
            0x0300_0000..=0x03ff_ffff => {
                let off = addr as usize & (IWRAM_SIZE - 1);
                self.iwram[off] = value;
            }
            0x0400_0000..=0x04ff_ffff => self.write_io_raw8((addr as usize) & 0x3ff, value),
            0x0500_0000..=0x05ff_ffff => {
                let off = (addr as usize) & (PRAM_SIZE - 1);
                self.pram[off] = value;
            }
            0x0600_0000..=0x06ff_ffff => {
                let off = vram_offset(addr);
                self.vram[off] = value;
            }
            0x0700_0000..=0x07ff_ffff => {
                let off = (addr as usize) & (OAM_SIZE - 1);
                self.oam[off] = value;
            }
            0x0e00_0000..=0x0eff_ffff => self.flash.write(addr, value),
            _ => {}
        }
    }

    pub fn request_irq(&mut self, flags: u16) {
        let iflag = self.read_io16(0x202) | flags;
        write_io_raw16(&mut self.io, 0x202, iflag);
    }

    fn read_io8(&self, off: usize) -> u8 {
        match off {
            0x006 | 0x007 | 0x130 | 0x131 => self.io[off],
            _ => self.io[off],
        }
    }

    fn read_io16(&self, off: usize) -> u16 {
        u16::from_le_bytes([self.read_io8(off), self.read_io8(off + 1)])
    }

    fn write_io8(&mut self, off: usize, value: u8) {
        self.log_io_write(off, 1, value as u32);
        self.write_io_raw8(off, value);
        self.after_io_write(off, 1);
    }

    fn log_io_write(&mut self, off: usize, size: usize, value: u32) {
        let interesting = off < 0x060 || (0x200..=0x209).contains(&off);
        if !interesting {
            return;
        }
        if self.io_log.len() == 64 {
            self.io_log.remove(0);
        }
        self.io_log.push(IoWrite {
            frame: self.frame,
            off,
            size,
            value,
        });
    }

    fn write_io16_no_after(&mut self, off: usize, value: u16) {
        match off {
            0x004 => {
                let status = self.read_io16(0x004) & 0x0007;
                write_io_raw16(&mut self.io, 0x004, status | (value & !0x0007));
            }
            0x006 | 0x130 => {}
            0x202 => {
                let now = self.read_io16(0x202) & !value;
                write_io_raw16(&mut self.io, 0x202, now);
            }
            _ => write_io_raw16(&mut self.io, off, value),
        }
    }

    fn write_io_raw8(&mut self, off: usize, value: u8) {
        match off {
            0x006 | 0x007 | 0x130 | 0x131 => {}
            0x202 | 0x203 => {
                let shift = if off & 1 == 0 { 0 } else { 8 };
                let clear = (value as u16) << shift;
                let now = self.read_io16(0x202) & !clear;
                write_io_raw16(&mut self.io, 0x202, now);
            }
            _ => self.io[off] = value,
        }
    }

    fn after_io_write(&mut self, off: usize, size: usize) {
        if overlaps(off, size, 0x004, 2) {
            let status = self.read_io16(0x004) & 0x0007;
            let writable = u16::from_le_bytes([self.io[0x004], self.io[0x005]]) & !0x0007;
            write_io_raw16(&mut self.io, 0x004, status | writable);
        }
        for timer in 0..4 {
            let base = 0x100 + timer * 4;
            if overlaps(off, size, base, 2) {
                self.timers[timer].reload = self.read_io16(base);
            }
            if overlaps(off, size, base + 2, 2) {
                let old = self.timers[timer].control;
                let new = self.read_io16(base + 2);
                self.timers[timer].control = new;
                if old & (1 << 7) == 0 && new & (1 << 7) != 0 {
                    self.timers[timer].counter = self.timers[timer].reload;
                    self.timers[timer].divider = 0;
                    write_io_raw16(&mut self.io, base, self.timers[timer].counter);
                }
            }
        }
        for ch in 0..4 {
            let base = 0x0b0 + ch * 12;
            if overlaps(off, size, base, 4) {
                self.dma_src[ch] = self.read_io32(base);
            }
            if overlaps(off, size, base + 4, 4) {
                self.dma_dst[ch] = self.read_io32(base + 4);
            }
            if overlaps(off, size, base + 10, 2) && self.read_io16(base + 10) & (1 << 15) != 0 {
                self.dma_src[ch] = self.read_io32(base);
                self.dma_dst[ch] = self.read_io32(base + 4);
                let timing = (self.read_io16(base + 10) >> 12) & 3;
                if timing == 0 {
                    self.run_dma_channel(ch, 0);
                }
            }
        }
    }

    fn write_narrow16(&mut self, addr: u32, value: u8, region: Region) {
        let a = addr & !1;
        self.write8_raw(a, value);
        self.write8_raw(a + 1, value);
        if matches!(region, Region::Palette | Region::Vram | Region::Oam) {
            let _ = region;
        }
    }

    fn run_dma_timing(&mut self, timing: u16) {
        for ch in 0..4 {
            self.run_dma_channel(ch, timing);
        }
    }

    fn run_dma_channel(&mut self, ch: usize, timing: u16) {
        let base = 0x0b0 + ch * 12;
        let control = self.read_io16(base + 10);
        if control & (1 << 15) == 0 || ((control >> 12) & 3) != timing {
            return;
        }
        let src_start = self.dma_src[ch];
        let dst_start = self.dma_dst[ch];
        let mut src = src_start;
        let mut dst = dst_start;
        let mut count = self.read_io16(base + 8) as u32;
        if count == 0 {
            count = if ch == 3 { 0x1_0000 } else { 0x4000 };
        }
        let fifo_dma = timing == 3 && (ch == 1 || ch == 2);
        if fifo_dma {
            count = 4;
        }
        let unit = if control & (1 << 10) != 0 || fifo_dma {
            4
        } else {
            2
        };
        let src_step = addr_step((control >> 7) & 3, unit);
        let dst_mode = (control >> 5) & 3;
        let dst_step = if fifo_dma {
            0
        } else {
            addr_step(dst_mode, unit)
        };

        for _ in 0..count {
            if unit == 4 {
                let value = self.read32(src);
                self.write32(dst, value);
            } else {
                let value = self.read16(src);
                self.write16(dst, value);
            }
            src = src.wrapping_add_signed(src_step);
            dst = dst.wrapping_add_signed(dst_step);
        }

        if control & (1 << 14) != 0 {
            self.request_irq(1 << (8 + ch));
        }

        let repeat = control & (1 << 9) != 0;
        let mut new_control = control;
        if repeat && timing != 0 {
            if fifo_dma || dst_mode == 3 {
                self.dma_dst[ch] = dst_start;
            } else {
                self.dma_dst[ch] = dst;
            }
            self.dma_src[ch] = src;
        } else {
            new_control &= !(1 << 15);
            write_io_raw16(&mut self.io, base + 10, new_control);
            self.dma_src[ch] = src;
            self.dma_dst[ch] = dst;
        }
    }

    fn read_io32(&self, off: usize) -> u32 {
        u32::from_le_bytes([
            self.read_io8(off),
            self.read_io8(off + 1),
            self.read_io8(off + 2),
            self.read_io8(off + 3),
        ])
    }

    fn step_timers(&mut self, cycles: u32) {
        for idx in 0..4 {
            if self.timers[idx].control & (1 << 7) == 0 {
                continue;
            }
            if self.timers[idx].control & (1 << 2) != 0 {
                continue;
            }
            let prescaler = match self.timers[idx].control & 3 {
                0 => 1,
                1 => 64,
                2 => 256,
                _ => 1024,
            };
            self.timers[idx].divider += cycles;
            while self.timers[idx].divider >= prescaler {
                self.timers[idx].divider -= prescaler;
                self.increment_timer(idx);
            }
        }
    }

    fn increment_timer(&mut self, idx: usize) {
        let (value, overflow) = self.timers[idx].counter.overflowing_add(1);
        if overflow {
            self.timers[idx].counter = self.timers[idx].reload;
            if idx <= 1 {
                self.run_fifo_dma(idx);
            }
            if self.timers[idx].control & (1 << 6) != 0 {
                self.request_irq(1 << (3 + idx));
            }
            if idx < 3
                && self.timers[idx + 1].control & (1 << 7) != 0
                && self.timers[idx + 1].control & (1 << 2) != 0
            {
                self.increment_timer(idx + 1);
            }
        } else {
            self.timers[idx].counter = value;
        }
        write_io_raw16(&mut self.io, 0x100 + idx * 4, self.timers[idx].counter);
    }

    fn run_fifo_dma(&mut self, timer: usize) {
        let fifo = if timer == 0 { 0x0400_00a0 } else { 0x0400_00a4 };
        for ch in 1..=2 {
            let base = 0x0b0 + ch * 12;
            let control = self.read_io16(base + 10);
            let dst = self.read_io32(base + 4);
            if control & (1 << 15) != 0 && ((control >> 12) & 3) == 3 && dst == fifo {
                self.run_dma_channel(ch, 3);
            }
        }
    }

    fn process_emerald_dma3_requests(&mut self) {
        const DMA3_REQUESTS: u32 = 0x0300_0938;
        const DMA3_REQUEST_SIZE: u32 = 16;

        for idx in 0..128 {
            let base = DMA3_REQUESTS + idx * DMA3_REQUEST_SIZE;
            let src = self.read32(base);
            let dst = self.read32(base + 4);
            let size = self.read16(base + 8) as u32;
            let mode = self.read16(base + 10);
            let value = self.read32(base + 12);

            if size == 0 {
                continue;
            }

            match mode {
                1 => {
                    let words = (size + 3) / 4;
                    for i in 0..words {
                        let off = i * 4;
                        self.write32(dst + off, self.read32(src + off));
                    }
                }
                2 => {
                    let words = (size + 3) / 4;
                    for i in 0..words {
                        self.write32(dst + i * 4, value);
                    }
                }
                3 => {
                    let halfwords = (size + 1) / 2;
                    for i in 0..halfwords {
                        let off = i * 2;
                        self.write16(dst + off, self.read16(src + off));
                    }
                }
                4 => {
                    let halfwords = (size + 1) / 2;
                    for i in 0..halfwords {
                        self.write16(dst + i * 2, value as u16);
                    }
                }
                _ => {}
            }

            self.write32(base, 0);
            self.write32(base + 4, 0);
            self.write16(base + 8, 0);
            self.write16(base + 10, 0);
            self.write32(base + 12, 0);
        }
    }
}

#[derive(Clone, Copy)]
enum Region {
    Palette,
    Vram,
    Oam,
}

fn overlaps(off: usize, size: usize, target: usize, target_size: usize) -> bool {
    off < target + target_size && target < off + size
}

fn vram_offset(addr: u32) -> usize {
    let mut off = (addr as usize) & 0x1ffff;
    if off >= VRAM_SIZE {
        off = 0x10000 + (off & 0x7fff);
    }
    off
}

fn rom_offset(addr: u32, len: usize) -> usize {
    ((addr as usize) & 0x01ff_ffff) % len
}

fn addr_step(mode: u16, unit: u32) -> i32 {
    match mode {
        0 | 3 => unit as i32,
        1 => -(unit as i32),
        2 => 0,
        _ => unit as i32,
    }
}

fn write_io_raw16(io: &mut [u8; 0x400], off: usize, value: u16) {
    let bytes = value.to_le_bytes();
    io[off] = bytes[0];
    io[off + 1] = bytes[1];
}
