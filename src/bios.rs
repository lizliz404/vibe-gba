use crate::bus::Bus;
use crate::cpu::Cpu;

pub fn handle_swi(code: u8, cpu: &mut Cpu, bus: &mut Bus) {
    match code {
        0x00 => soft_reset(cpu),
        0x01 => register_ram_reset(cpu.reg(0), bus),
        0x02 | 0x03 => cpu.set_halted(true),
        0x04 => intr_wait(cpu, bus, cpu.reg(0) != 0, cpu.reg(1) as u16),
        0x05 => intr_wait(cpu, bus, true, 1),
        0x06 => div(cpu, cpu.reg(0) as i32, cpu.reg(1) as i32),
        0x07 => div(cpu, cpu.reg(1) as i32, cpu.reg(0) as i32),
        0x08 => cpu.set_reg(0, (cpu.reg(0) as f64).sqrt() as u32),
        0x09 => arc_tan(cpu),
        0x0a => arc_tan2(cpu),
        0x0b => cpu_set(cpu, bus),
        0x0c => cpu_fast_set(cpu, bus),
        0x0e => bg_affine_set(cpu, bus),
        0x0f => obj_affine_set(cpu, bus),
        0x10 => bit_unpack(cpu, bus),
        0x11 => lz77(cpu, bus),
        0x12 => lz77(cpu, bus),
        0x14 => rl_uncomp(cpu, bus),
        0x15 => rl_uncomp(cpu, bus),
        0x16 => diff8(cpu, bus),
        0x17 => diff8(cpu, bus),
        0x18 => diff16(cpu, bus),
        0x19 => {}
        0x1f => midi_key_to_freq(cpu, bus),
        _ => {}
    }
}

fn soft_reset(cpu: &mut Cpu) {
    cpu.set_reg(13, 0x0300_7f00);
    cpu.set_reg(14, 0);
    cpu.set_reg(15, 0x0800_0000);
}

fn register_ram_reset(mask: u32, bus: &mut Bus) {
    if mask & (1 << 0) != 0 {
        for addr in 0x0200_0000..0x0204_0000 {
            bus.write8_raw(addr, 0);
        }
    }
    if mask & (1 << 1) != 0 {
        for addr in 0x0300_0000..0x0300_7e00 {
            bus.write8_raw(addr, 0);
        }
    }
    if mask & (1 << 2) != 0 {
        for addr in 0x0500_0000..0x0500_0400 {
            bus.write8_raw(addr, 0);
        }
    }
    if mask & (1 << 3) != 0 {
        for addr in 0x0600_0000..0x0601_8000 {
            bus.write8_raw(addr, 0);
        }
    }
    if mask & (1 << 4) != 0 {
        for addr in 0x0700_0000..0x0700_0400 {
            bus.write8_raw(addr, 0);
        }
    }
}

fn intr_wait(cpu: &mut Cpu, bus: &mut Bus, discard_old: bool, flags: u16) {
    if discard_old {
        bus.clear_if(flags);
    }
    if bus.wait_ready(flags) {
        bus.clear_if(flags);
    } else {
        cpu.set_wait_flags(flags);
    }
}

fn div(cpu: &mut Cpu, num: i32, den: i32) {
    if den == 0 {
        cpu.set_reg(0, 0);
        cpu.set_reg(1, num as u32);
        cpu.set_reg(3, 0);
        return;
    }
    let q = num.wrapping_div(den);
    let r = num.wrapping_rem(den);
    cpu.set_reg(0, q as u32);
    cpu.set_reg(1, r as u32);
    cpu.set_reg(3, q.unsigned_abs());
}

fn arc_tan(cpu: &mut Cpu) {
    let x = cpu.reg(0) as i16 as f64 / 16384.0;
    let angle = x.atan() * 32768.0 / std::f64::consts::PI;
    cpu.set_reg(0, angle as i16 as u16 as u32);
}

fn arc_tan2(cpu: &mut Cpu) {
    let x = cpu.reg(0) as i16 as f64;
    let y = cpu.reg(1) as i16 as f64;
    let angle = y.atan2(x) * 32768.0 / std::f64::consts::PI;
    cpu.set_reg(0, angle as i16 as u16 as u32);
}

fn cpu_set(cpu: &Cpu, bus: &mut Bus) {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    let mode = cpu.reg(2);
    let count = mode & 0x1f_ffff;
    let fixed = mode & (1 << 24) != 0;
    let word = mode & (1 << 26) != 0;
    if word {
        let fill = bus.read32(src);
        for i in 0..count {
            let value = if fixed { fill } else { bus.read32(src + i * 4) };
            bus.write32(dst + i * 4, value);
        }
    } else {
        let fill = bus.read16(src);
        for i in 0..count {
            let value = if fixed { fill } else { bus.read16(src + i * 2) };
            bus.write16(dst + i * 2, value);
        }
    }
}

fn cpu_fast_set(cpu: &Cpu, bus: &mut Bus) {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    let mode = cpu.reg(2);
    let count = mode & 0x1f_ffff;
    let fixed = mode & (1 << 24) != 0;
    let fill = bus.read32(src);
    let rounded = (count + 7) & !7;
    for i in 0..rounded {
        let value = if fixed { fill } else { bus.read32(src + i * 4) };
        bus.write32(dst + i * 4, value);
    }
}

fn bg_affine_set(cpu: &Cpu, bus: &mut Bus) {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    let count = cpu.reg(2);
    for i in 0..count {
        let s = src + i * 20;
        let d = dst + i * 16;
        let cx = bus.read32(s) as i32;
        let cy = bus.read32(s + 4) as i32;
        let disp_x = bus.read16(s + 8) as i16 as i32;
        let disp_y = bus.read16(s + 10) as i16 as i32;
        let sx = bus.read16(s + 12) as i16 as f64 / 256.0;
        let sy = bus.read16(s + 14) as i16 as f64 / 256.0;
        let theta = bus.read16(s + 16) as f64 * std::f64::consts::TAU / 65536.0;
        let sin = theta.sin();
        let cos = theta.cos();
        let pa = (cos * sx * 256.0) as i16 as u16;
        let pb = (-sin * sx * 256.0) as i16 as u16;
        let pc = (sin * sy * 256.0) as i16 as u16;
        let pd = (cos * sy * 256.0) as i16 as u16;
        let start_x = cx - (pa as i16 as i32 * disp_x + pb as i16 as i32 * disp_y);
        let start_y = cy - (pc as i16 as i32 * disp_x + pd as i16 as i32 * disp_y);
        bus.write16(d, pa);
        bus.write16(d + 2, pb);
        bus.write16(d + 4, pc);
        bus.write16(d + 6, pd);
        bus.write32(d + 8, start_x as u32);
        bus.write32(d + 12, start_y as u32);
    }
}

fn obj_affine_set(cpu: &Cpu, bus: &mut Bus) {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    let count = cpu.reg(2);
    let offset = cpu.reg(3);
    for i in 0..count {
        let s = src + i * 8;
        let d = dst + i * offset * 4;
        for j in 0..4 {
            bus.write16(d + j * offset, bus.read16(s + j * 2));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::Flash;

    #[test]
    fn obj_affine_set_uses_destination_offset_bytes() {
        let mut cpu = Cpu::new(false);
        let mut bus = Bus::new(vec![0xff], Flash::new(None));
        let src = 0x0200_0000;
        let dst = 0x0200_0100;

        for (idx, value) in [0x1111, 0x2222, 0x3333, 0x4444].into_iter().enumerate() {
            bus.write16(src + idx as u32 * 2, value);
        }
        for off in (0..0x20).step_by(2) {
            bus.write16(dst + off, 0xdead);
        }

        cpu.set_reg(0, src);
        cpu.set_reg(1, dst);
        cpu.set_reg(2, 1);
        cpu.set_reg(3, 2);

        handle_swi(0x0f, &mut cpu, &mut bus);

        assert_eq!(bus.read16(dst), 0x1111);
        assert_eq!(bus.read16(dst + 2), 0x2222);
        assert_eq!(bus.read16(dst + 4), 0x3333);
        assert_eq!(bus.read16(dst + 6), 0x4444);
        assert_eq!(bus.read16(dst + 8), 0xdead);
        assert_eq!(bus.read16(dst + 16), 0xdead);
        assert_eq!(bus.read16(dst + 24), 0xdead);
    }
}

fn bit_unpack(cpu: &Cpu, bus: &mut Bus) {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    let info = cpu.reg(2);
    let len = bus.read16(info) as u32;
    let src_bits = bus.read8(info + 2) as u32;
    let dst_bits = bus.read8(info + 3) as u32;
    let mut offset = bus.read32(info + 4);
    let add_zero = offset & (1 << 31) != 0;
    offset &= 0x7fff_ffff;
    let dst_mask = (1u32 << dst_bits) - 1;
    let mut out_word = 0u32;
    let mut out_bits = 0u32;
    let mut out_addr = dst;
    for bit_index in 0..(len * 8 / src_bits) {
        let byte = bus.read8(src + (bit_index * src_bits) / 8) as u32;
        let shift = (bit_index * src_bits) % 8;
        let mut value = (byte >> shift) & ((1 << src_bits) - 1);
        if value != 0 || add_zero {
            value = value.wrapping_add(offset) & dst_mask;
        }
        out_word |= value << out_bits;
        out_bits += dst_bits;
        if out_bits >= 32 {
            bus.write32(out_addr, out_word);
            out_addr += 4;
            out_word = 0;
            out_bits = 0;
        }
    }
    if out_bits != 0 {
        bus.write32(out_addr, out_word);
    }
}

fn lz77(cpu: &Cpu, bus: &mut Bus) {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    let header = bus.read32(src);
    let size = header >> 8;
    let mut in_addr = src + 4;
    let mut out = Vec::with_capacity(size as usize);
    while out.len() < size as usize {
        let flags = bus.read8(in_addr);
        in_addr += 1;
        for bit in (0..8).rev() {
            if out.len() >= size as usize {
                break;
            }
            if flags & (1 << bit) == 0 {
                out.push(bus.read8(in_addr));
                in_addr += 1;
            } else {
                let b1 = bus.read8(in_addr);
                let b2 = bus.read8(in_addr + 1);
                in_addr += 2;
                let len = (b1 >> 4) as usize + 3;
                let disp = (((b1 as usize) & 0x0f) << 8) | b2 as usize;
                let pos = out.len().wrapping_sub(disp + 1);
                for i in 0..len {
                    let value = out[pos + i];
                    out.push(value);
                    if out.len() >= size as usize {
                        break;
                    }
                }
            }
        }
    }
    write_bytes(dst, &out, bus);
}

fn rl_uncomp(cpu: &Cpu, bus: &mut Bus) {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    let header = bus.read32(src);
    let size = header >> 8;
    let mut in_addr = src + 4;
    let mut out = Vec::with_capacity(size as usize);
    while out.len() < size as usize {
        let flag = bus.read8(in_addr);
        in_addr += 1;
        if flag & 0x80 != 0 {
            let count = (flag & 0x7f) as usize + 3;
            let value = bus.read8(in_addr);
            in_addr += 1;
            for _ in 0..count {
                out.push(value);
                if out.len() >= size as usize {
                    break;
                }
            }
        } else {
            let count = flag as usize + 1;
            for _ in 0..count {
                out.push(bus.read8(in_addr));
                in_addr += 1;
                if out.len() >= size as usize {
                    break;
                }
            }
        }
    }
    write_bytes(dst, &out, bus);
}

fn diff8(cpu: &Cpu, bus: &mut Bus) {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    let size = bus.read32(src) >> 8;
    let mut prev = 0u8;
    let mut out = Vec::with_capacity(size as usize);
    for i in 0..size {
        prev = prev.wrapping_add(bus.read8(src + 4 + i));
        out.push(prev);
    }
    write_bytes(dst, &out, bus);
}

fn diff16(cpu: &Cpu, bus: &mut Bus) {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    let size = bus.read32(src) >> 8;
    let mut prev = 0u16;
    let mut out = Vec::with_capacity(size as usize);
    let mut in_addr = src + 4;
    while out.len() < size as usize {
        prev = prev.wrapping_add(bus.read16(in_addr));
        in_addr += 2;
        out.extend_from_slice(&prev.to_le_bytes());
    }
    write_bytes(dst, &out[..size as usize], bus);
}

fn midi_key_to_freq(cpu: &mut Cpu, bus: &mut Bus) {
    let wave = cpu.reg(0);
    let mk = cpu.reg(1) as i32;
    let fp = cpu.reg(2) as i32;
    let base = bus.read32(wave + 4);
    let exponent = (180 - mk - fp / 256) as f64 / 12.0;
    let freq = (base as f64 / 2f64.powf(exponent)) as u32;
    cpu.set_reg(0, freq);
}

fn write_bytes(dst: u32, bytes: &[u8], bus: &mut Bus) {
    for (i, byte) in bytes.iter().copied().enumerate() {
        bus.write8_raw(dst + i as u32, byte);
    }
}
