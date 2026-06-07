use crate::bios;
use crate::bus::Bus;
use serde::{Deserialize, Serialize};

const FLAG_N: u32 = 1 << 31;
const FLAG_Z: u32 = 1 << 30;
const FLAG_C: u32 = 1 << 29;
const FLAG_V: u32 = 1 << 28;
const FLAG_I: u32 = 1 << 7;
const FLAG_T: u32 = 1 << 5;
const MODE_SYSTEM: u32 = 0x1f;
const IRQ_RETURN_SENTINEL: u32 = 0xffff_fff0;
const G_SPRITES: u32 = 0x0202_0630;
const SPRITE_SIZE: u32 = 0x44;
const G_OBJECT_EVENTS: u32 = 0x0203_7350;
const OBJECT_EVENT_SIZE: u32 = 0x24;
const G_PLAYER_AVATAR: u32 = 0x0203_7590;
const G_MAIN_CALLBACK2: u32 = 0x0300_22c4;
const CB2_OVERWORLD: u32 = 0x0808_5e5d;
const DIRECT_MOVE_FRAME_LATCH: u32 = 0x0300_3090;

#[derive(Clone, Deserialize, Serialize)]
struct IrqFrame {
    regs: [u32; 16],
    cpsr: u32,
}

#[derive(Deserialize, Serialize)]
pub struct Cpu {
    pub(crate) r: [u32; 16],
    pub(crate) cpsr: u32,
    trace: bool,
    halted: bool,
    wait_flags: Option<u16>,
    irq_frame: Option<IrqFrame>,
    last_pc: u32,
    last_instr: u32,
    last_thumb: bool,
}

impl Cpu {
    pub fn new(trace: bool) -> Self {
        let mut r = [0u32; 16];
        r[13] = 0x0300_7f00;
        r[15] = 0x0800_0000;
        Self {
            r,
            cpsr: MODE_SYSTEM | FLAG_I,
            trace,
            halted: false,
            wait_flags: None,
            irq_frame: None,
            last_pc: 0,
            last_instr: 0,
            last_thumb: false,
        }
    }

    pub fn pc(&self) -> u32 {
        self.r[15]
    }

    pub fn sp(&self) -> u32 {
        self.r[13]
    }

    pub fn debug_summary(&self) -> String {
        let mut out = format!(
            "CPU pc={:08x} lr={:08x} sp={:08x} cpsr={:08x} thumb={} halted={} wait={:?} irq_hle={} last={:08x}:{:08x}:{}",
            self.r[15],
            self.r[14],
            self.r[13],
            self.cpsr,
            self.is_thumb(),
            self.halted,
            self.wait_flags,
            self.irq_frame.is_some(),
            self.last_pc,
            self.last_instr,
            if self.last_thumb { "thumb" } else { "arm" }
        );
        use std::fmt::Write;
        for row in 0..4 {
            let _ = write!(out, "\nr{:02}-r{:02}", row * 4, row * 4 + 3);
            for col in 0..4 {
                let reg = row * 4 + col;
                let _ = write!(out, " {:08x}", self.r[reg]);
            }
        }
        out
    }

    pub fn step(&mut self, bus: &mut Bus) -> u32 {
        if let Some(flags) = self.wait_flags {
            if bus.wait_ready(flags) {
                bus.clear_if(flags);
                self.wait_flags = None;
                self.halted = false;
            } else {
                return 32;
            }
        }

        if self.halted {
            if bus.irq_pending() != 0 {
                self.halted = false;
            } else {
                return 32;
            }
        }

        if self.irq_frame.is_none() && self.cpsr & FLAG_I == 0 && bus.irq_pending() != 0 {
            self.enter_irq_hle(bus);
        }

        if self.try_sound_mixer_hle(bus) {
            return 1;
        }

        if self.is_thumb() {
            let pc = self.r[15] & !1;
            let instr = bus.read16(pc);
            self.last_pc = pc;
            self.last_instr = instr as u32;
            self.last_thumb = true;
            self.r[15] = pc.wrapping_add(2);
            if self.trace {
                eprintln!("{pc:08x}: {instr:04x}  thumb");
            }
            self.execute_thumb(instr, pc, bus);
            1
        } else {
            let pc = self.r[15] & !3;
            let instr = bus.read32(pc);
            self.last_pc = pc;
            self.last_instr = instr;
            self.last_thumb = false;
            self.r[15] = pc.wrapping_add(4);
            if self.trace {
                eprintln!("{pc:08x}: {instr:08x}  arm");
            }
            if self.condition((instr >> 28) as u8) {
                self.execute_arm(instr, pc, bus);
            }
            1
        }
    }

    pub(crate) fn set_halted(&mut self, halted: bool) {
        self.halted = halted;
    }

    pub(crate) fn set_wait_flags(&mut self, flags: u16) {
        self.wait_flags = Some(flags);
        self.halted = true;
    }

    pub(crate) fn reg(&self, idx: usize) -> u32 {
        self.r[idx]
    }

    pub(crate) fn set_reg(&mut self, idx: usize, value: u32) {
        if idx == 15 {
            self.branch_to(value);
        } else {
            self.r[idx] = value;
        }
    }

    fn execute_arm(&mut self, instr: u32, pc: u32, bus: &mut Bus) {
        if instr & 0x0fff_fff0 == 0x012f_ff10 {
            let rn = (instr & 0xf) as usize;
            self.branch_to(self.reg_arm(rn, pc));
            return;
        }

        if instr & 0x0f00_0000 == 0x0f00_0000 {
            let code = ((instr >> 16) & 0xff) as u8;
            bios::handle_swi(code, self, bus);
            return;
        }

        if self.try_psr(instr, pc) {
            return;
        }

        if instr & 0x0fc0_00f0 == 0x0000_0090 {
            self.arm_multiply(instr, pc);
            return;
        }

        if instr & 0x0f80_00f0 == 0x0080_0090 {
            self.arm_multiply_long(instr, pc);
            return;
        }

        if instr & 0x0e00_0090 == 0x0000_0090 {
            self.arm_halfword_transfer(instr, pc, bus);
            return;
        }

        match (instr >> 25) & 7 {
            0b000 | 0b001 => self.arm_data_processing(instr, pc),
            0b010 | 0b011 => self.arm_single_transfer(instr, pc, bus),
            0b100 => self.arm_block_transfer(instr, pc, bus),
            0b101 => self.arm_branch(instr, pc),
            _ => {}
        }
    }

    fn execute_thumb(&mut self, instr: u16, pc: u32, bus: &mut Bus) {
        match instr {
            0x0000..=0x17ff => self.thumb_shift(instr, pc),
            0x1800..=0x1fff => self.thumb_add_sub(instr, pc),
            0x2000..=0x3fff => self.thumb_imm(instr),
            0x4000..=0x43ff => self.thumb_alu(instr),
            0x4400..=0x47ff => self.thumb_hi(instr, pc),
            0x4800..=0x4fff => {
                let rd = ((instr >> 8) & 7) as usize;
                let addr = ((pc + 4) & !3).wrapping_add(((instr & 0xff) as u32) << 2);
                self.r[rd] = bus.read32(addr);
            }
            0x5000..=0x5fff => self.thumb_load_store_reg(instr, pc, bus),
            0x6000..=0x7fff => self.thumb_load_store_imm(instr, pc, bus),
            0x8000..=0x8fff => self.thumb_load_store_half(instr, pc, bus),
            0x9000..=0x9fff => self.thumb_sp_relative(instr, bus),
            0xa000..=0xafff => self.thumb_load_address(instr, pc),
            0xb000..=0xb0ff => {
                let imm = ((instr & 0x7f) as u32) << 2;
                if instr & (1 << 7) != 0 {
                    self.r[13] = self.r[13].wrapping_sub(imm);
                } else {
                    self.r[13] = self.r[13].wrapping_add(imm);
                }
            }
            0xb400..=0xb5ff | 0xbc00..=0xbdff => self.thumb_push_pop(instr, bus),
            0xc000..=0xcfff => self.thumb_multiple(instr, bus),
            0xd000..=0xdfff => self.thumb_cond_branch_swi(instr, pc, bus),
            0xe000..=0xe7ff => {
                let off = sign_extend(((instr & 0x7ff) as u32) << 1, 12);
                self.r[15] = pc.wrapping_add(4).wrapping_add(off as u32) & !1;
            }
            0xf000..=0xf7ff => {
                let off = sign_extend(((instr & 0x7ff) as u32) << 12, 23);
                self.r[14] = pc.wrapping_add(4).wrapping_add(off as u32);
            }
            0xf800..=0xffff => {
                let target = self.r[14].wrapping_add(((instr & 0x7ff) as u32) << 1);
                self.r[14] = (pc + 2) | 1;
                self.branch_to(target | 1);
            }
            _ => {}
        }
    }

    fn arm_branch(&mut self, instr: u32, pc: u32) {
        if instr & (1 << 24) != 0 {
            self.r[14] = pc + 4;
        }
        let off = sign_extend((instr & 0x00ff_ffff) << 2, 26);
        self.r[15] = pc.wrapping_add(8).wrapping_add(off as u32) & !3;
    }

    fn arm_data_processing(&mut self, instr: u32, pc: u32) {
        let op = ((instr >> 21) & 0xf) as u8;
        let set_flags = instr & (1 << 20) != 0;
        let rn = ((instr >> 16) & 0xf) as usize;
        let rd = ((instr >> 12) & 0xf) as usize;
        let (op2, sh_carry) = self.arm_operand2(instr, pc);
        let a = self.reg_arm(rn, pc);
        let old_c = self.carry();
        let mut write = true;
        let result;

        match op {
            0x0 => result = a & op2,
            0x1 => result = a ^ op2,
            0x2 => {
                result = a.wrapping_sub(op2);
                if set_flags {
                    self.set_sub_flags(a, op2, result);
                }
            }
            0x3 => {
                result = op2.wrapping_sub(a);
                if set_flags {
                    self.set_sub_flags(op2, a, result);
                }
            }
            0x4 => {
                result = a.wrapping_add(op2);
                if set_flags {
                    self.set_add_flags(a, op2, result);
                }
            }
            0x5 => {
                let carry = old_c as u32;
                result = a.wrapping_add(op2).wrapping_add(carry);
                if set_flags {
                    self.set_adc_flags(a, op2, carry, result);
                }
            }
            0x6 => {
                let borrow = (!old_c) as u32;
                result = a.wrapping_sub(op2).wrapping_sub(borrow);
                if set_flags {
                    self.set_sbc_flags(a, op2, borrow, result);
                }
            }
            0x7 => {
                let borrow = (!old_c) as u32;
                result = op2.wrapping_sub(a).wrapping_sub(borrow);
                if set_flags {
                    self.set_sbc_flags(op2, a, borrow, result);
                }
            }
            0x8 => {
                write = false;
                result = a & op2;
                self.set_nz(result);
                self.set_c(sh_carry);
            }
            0x9 => {
                write = false;
                result = a ^ op2;
                self.set_nz(result);
                self.set_c(sh_carry);
            }
            0xa => {
                write = false;
                result = a.wrapping_sub(op2);
                self.set_sub_flags(a, op2, result);
            }
            0xb => {
                write = false;
                result = a.wrapping_add(op2);
                self.set_add_flags(a, op2, result);
            }
            0xc => result = a | op2,
            0xd => result = op2,
            0xe => result = a & !op2,
            0xf => result = !op2,
            _ => unreachable!(),
        }

        if write {
            if set_flags && !matches!(op, 0x2..=0x7) {
                self.set_nz(result);
                self.set_c(sh_carry);
            }
            self.write_reg_arm(rd, result);
        }
    }

    fn arm_single_transfer(&mut self, instr: u32, pc: u32, bus: &mut Bus) {
        let rn = ((instr >> 16) & 0xf) as usize;
        let rd = ((instr >> 12) & 0xf) as usize;
        let base = self.reg_arm(rn, pc);
        let offset = if instr & (1 << 25) != 0 {
            self.arm_operand2(instr, pc).0
        } else {
            instr & 0xfff
        };
        let up = instr & (1 << 23) != 0;
        let pre = instr & (1 << 24) != 0;
        let writeback = instr & (1 << 21) != 0;
        let load = instr & (1 << 20) != 0;
        let byte = instr & (1 << 22) != 0;
        let offset_base = if up {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        };
        let addr = if pre { offset_base } else { base };

        if load {
            let value = if byte {
                bus.read8(addr) as u32
            } else {
                rotate_right(bus.read32(addr), (addr & 3) * 8)
            };
            self.write_reg_arm(rd, value);
        } else {
            let value = self.reg_arm(rd, pc);
            if byte {
                bus.write8(addr, value as u8);
            } else {
                bus.write32(addr, value);
            }
        }

        if !pre || writeback {
            self.r[rn] = offset_base;
        }
    }

    fn arm_halfword_transfer(&mut self, instr: u32, pc: u32, bus: &mut Bus) {
        let rn = ((instr >> 16) & 0xf) as usize;
        let rd = ((instr >> 12) & 0xf) as usize;
        let base = self.reg_arm(rn, pc);
        let offset = if instr & (1 << 22) != 0 {
            ((instr >> 4) & 0xf0) | (instr & 0xf)
        } else {
            self.reg_arm((instr & 0xf) as usize, pc)
        };
        let up = instr & (1 << 23) != 0;
        let pre = instr & (1 << 24) != 0;
        let writeback = instr & (1 << 21) != 0;
        let load = instr & (1 << 20) != 0;
        let kind = (instr >> 5) & 3;
        let offset_base = if up {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        };
        let addr = if pre { offset_base } else { base };

        if load {
            let value = match kind {
                1 => bus.read16(addr) as u32,
                2 => bus.read8(addr) as i8 as i32 as u32,
                3 => bus.read16(addr) as i16 as i32 as u32,
                _ => 0,
            };
            self.write_reg_arm(rd, value);
        } else if kind == 1 {
            bus.write16(addr, self.reg_arm(rd, pc) as u16);
        }

        if !pre || writeback {
            self.r[rn] = offset_base;
        }
    }

    fn arm_block_transfer(&mut self, instr: u32, pc: u32, bus: &mut Bus) {
        let rn = ((instr >> 16) & 0xf) as usize;
        let list = instr & 0xffff;
        let count = list.count_ones().max(1);
        let pre = instr & (1 << 24) != 0;
        let up = instr & (1 << 23) != 0;
        let writeback = instr & (1 << 21) != 0;
        let load = instr & (1 << 20) != 0;
        let base = self.r[rn];
        let start = match (up, pre) {
            (true, false) => base,
            (true, true) => base + 4,
            (false, false) => base.wrapping_sub(4 * (count - 1)),
            (false, true) => base.wrapping_sub(4 * count),
        };
        let final_base = if up {
            base.wrapping_add(4 * count)
        } else {
            base.wrapping_sub(4 * count)
        };
        let mut addr = start;
        if list == 0 {
            if load {
                self.write_reg_arm(15, bus.read32(addr));
            } else {
                bus.write32(addr, self.reg_arm(15, pc));
            }
        } else if load {
            for reg in 0..16 {
                if list & (1 << reg) != 0 {
                    let value = bus.read32(addr);
                    if reg == 15 {
                        self.branch_to(value);
                    } else {
                        self.r[reg] = value;
                    }
                    addr = addr.wrapping_add(4);
                }
            }
        } else {
            for reg in 0..16 {
                if list & (1 << reg) != 0 {
                    bus.write32(addr, self.reg_arm(reg, pc));
                    addr = addr.wrapping_add(4);
                }
            }
        }
        if writeback {
            self.r[rn] = final_base;
        }
    }

    fn arm_multiply(&mut self, instr: u32, pc: u32) {
        let accumulate = instr & (1 << 21) != 0;
        let set_flags = instr & (1 << 20) != 0;
        let rd = ((instr >> 16) & 0xf) as usize;
        let rn = ((instr >> 12) & 0xf) as usize;
        let rs = ((instr >> 8) & 0xf) as usize;
        let rm = (instr & 0xf) as usize;
        let mut result = self.reg_arm(rm, pc).wrapping_mul(self.reg_arm(rs, pc));
        if accumulate {
            result = result.wrapping_add(self.reg_arm(rn, pc));
        }
        self.write_reg_arm(rd, result);
        if set_flags {
            self.set_nz(result);
        }
    }

    fn arm_multiply_long(&mut self, instr: u32, pc: u32) {
        let signed = instr & (1 << 22) != 0;
        let accumulate = instr & (1 << 21) != 0;
        let set_flags = instr & (1 << 20) != 0;
        let rd_hi = ((instr >> 16) & 0xf) as usize;
        let rd_lo = ((instr >> 12) & 0xf) as usize;
        let rs = ((instr >> 8) & 0xf) as usize;
        let rm = (instr & 0xf) as usize;
        let mut result = if signed {
            (self.reg_arm(rm, pc) as i32 as i64).wrapping_mul(self.reg_arm(rs, pc) as i32 as i64)
                as u64
        } else {
            (self.reg_arm(rm, pc) as u64).wrapping_mul(self.reg_arm(rs, pc) as u64)
        };
        if accumulate {
            let old = ((self.r[rd_hi] as u64) << 32) | self.r[rd_lo] as u64;
            result = result.wrapping_add(old);
        }
        self.r[rd_lo] = result as u32;
        self.r[rd_hi] = (result >> 32) as u32;
        if set_flags {
            self.set_nz64(result);
        }
    }

    fn try_psr(&mut self, instr: u32, pc: u32) -> bool {
        if instr & 0x0fbf_0fff == 0x010f_0000 {
            let rd = ((instr >> 12) & 0xf) as usize;
            self.r[rd] = self.cpsr;
            return true;
        }

        let is_msr_reg = instr & 0x0db0_f000 == 0x0120_f000;
        let is_msr_imm = instr & 0x0db0_f000 == 0x0320_f000;
        if is_msr_reg || is_msr_imm {
            let fields = (instr >> 16) & 0xf;
            let value = if is_msr_imm {
                let imm = instr & 0xff;
                rotate_right(imm, ((instr >> 8) & 0xf) * 2)
            } else {
                self.reg_arm((instr & 0xf) as usize, pc)
            };
            let mut mask = 0;
            if fields & 1 != 0 {
                mask |= 0x0000_00ff;
            }
            if fields & 8 != 0 {
                mask |= 0xff00_0000;
            }
            self.cpsr = (self.cpsr & !mask) | (value & mask);
            return true;
        }
        false
    }

    fn thumb_shift(&mut self, instr: u16, pc: u32) {
        let op = (instr >> 11) & 3;
        let offset = ((instr >> 6) & 0x1f) as u32;
        let rs = ((instr >> 3) & 7) as usize;
        let rd = (instr & 7) as usize;
        let value = self.reg_thumb(rs, pc);
        let (result, carry) = match op {
            0 => shift_lsl(value, offset, self.carry()),
            1 => shift_lsr(value, if offset == 0 { 32 } else { offset }, self.carry()),
            2 => shift_asr(value, if offset == 0 { 32 } else { offset }, self.carry()),
            _ => unreachable!(),
        };
        self.r[rd] = result;
        self.set_nz(result);
        self.set_c(carry);
    }

    fn thumb_add_sub(&mut self, instr: u16, pc: u32) {
        let imm = instr & (1 << 10) != 0;
        let sub = instr & (1 << 9) != 0;
        let rn = ((instr >> 6) & 7) as usize;
        let rs = ((instr >> 3) & 7) as usize;
        let rd = (instr & 7) as usize;
        let op2 = if imm {
            rn as u32
        } else {
            self.reg_thumb(rn, pc)
        };
        let a = self.reg_thumb(rs, pc);
        let result = if sub {
            let result = a.wrapping_sub(op2);
            self.set_sub_flags(a, op2, result);
            result
        } else {
            let result = a.wrapping_add(op2);
            self.set_add_flags(a, op2, result);
            result
        };
        self.r[rd] = result;
    }

    fn thumb_imm(&mut self, instr: u16) {
        let op = (instr >> 11) & 3;
        let rd = ((instr >> 8) & 7) as usize;
        let imm = (instr & 0xff) as u32;
        match op {
            0 => {
                self.r[rd] = imm;
                self.set_nz(imm);
            }
            1 => {
                let result = self.r[rd].wrapping_sub(imm);
                self.set_sub_flags(self.r[rd], imm, result);
            }
            2 => {
                let result = self.r[rd].wrapping_add(imm);
                self.r[rd] = result;
                self.set_add_flags(self.r[rd].wrapping_sub(imm), imm, result);
            }
            3 => {
                let old = self.r[rd];
                let result = old.wrapping_sub(imm);
                self.r[rd] = result;
                self.set_sub_flags(old, imm, result);
            }
            _ => unreachable!(),
        }
    }

    fn thumb_alu(&mut self, instr: u16) {
        let op = (instr >> 6) & 0xf;
        let rs = ((instr >> 3) & 7) as usize;
        let rd = (instr & 7) as usize;
        let a = self.r[rd];
        let b = self.r[rs];
        match op {
            0x0 => {
                self.r[rd] = a & b;
                self.set_nz(self.r[rd]);
            }
            0x1 => {
                self.r[rd] = a ^ b;
                self.set_nz(self.r[rd]);
            }
            0x2 => {
                let (result, carry) = shift_lsl(a, b & 0xff, self.carry());
                self.r[rd] = result;
                self.set_nz(result);
                self.set_c(carry);
            }
            0x3 => {
                let (result, carry) = shift_lsr(a, b & 0xff, self.carry());
                self.r[rd] = result;
                self.set_nz(result);
                self.set_c(carry);
            }
            0x4 => {
                let (result, carry) = shift_asr(a, b & 0xff, self.carry());
                self.r[rd] = result;
                self.set_nz(result);
                self.set_c(carry);
            }
            0x5 => {
                let carry = self.carry() as u32;
                let result = a.wrapping_add(b).wrapping_add(carry);
                self.r[rd] = result;
                self.set_adc_flags(a, b, carry, result);
            }
            0x6 => {
                let borrow = (!self.carry()) as u32;
                let result = a.wrapping_sub(b).wrapping_sub(borrow);
                self.r[rd] = result;
                self.set_sbc_flags(a, b, borrow, result);
            }
            0x7 => {
                let (result, carry) = shift_ror(a, b & 0xff, self.carry());
                self.r[rd] = result;
                self.set_nz(result);
                self.set_c(carry);
            }
            0x8 => {
                let result = a & b;
                self.set_nz(result);
            }
            0x9 => {
                let result = 0u32.wrapping_sub(b);
                self.r[rd] = result;
                self.set_sub_flags(0, b, result);
            }
            0xa => {
                let result = a.wrapping_sub(b);
                self.set_sub_flags(a, b, result);
            }
            0xb => {
                let result = a.wrapping_add(b);
                self.set_add_flags(a, b, result);
            }
            0xc => {
                self.r[rd] = a | b;
                self.set_nz(self.r[rd]);
            }
            0xd => {
                self.r[rd] = a.wrapping_mul(b);
                self.set_nz(self.r[rd]);
            }
            0xe => {
                self.r[rd] = a & !b;
                self.set_nz(self.r[rd]);
            }
            0xf => {
                self.r[rd] = !b;
                self.set_nz(self.r[rd]);
            }
            _ => unreachable!(),
        }
    }

    fn thumb_hi(&mut self, instr: u16, pc: u32) {
        let op = (instr >> 8) & 3;
        let rs = (((instr >> 3) & 7) | ((instr >> 3) & 8)) as usize;
        let rd = ((instr & 7) | ((instr >> 4) & 8)) as usize;
        let a = self.reg_thumb(rd, pc);
        let b = self.reg_thumb(rs, pc);
        match op {
            0 => self.write_reg_thumb(rd, a.wrapping_add(b)),
            1 => {
                let result = a.wrapping_sub(b);
                self.set_sub_flags(a, b, result);
            }
            2 => self.write_reg_thumb(rd, b),
            3 => self.branch_to(b),
            _ => unreachable!(),
        }
    }

    fn thumb_load_store_reg(&mut self, instr: u16, pc: u32, bus: &mut Bus) {
        let op = (instr >> 9) & 7;
        let ro = ((instr >> 6) & 7) as usize;
        let rb = ((instr >> 3) & 7) as usize;
        let rd = (instr & 7) as usize;
        let addr = self.reg_thumb(rb, pc).wrapping_add(self.reg_thumb(ro, pc));
        match op {
            0 => bus.write32(addr, self.r[rd]),
            1 => bus.write16(addr, self.r[rd] as u16),
            2 => bus.write8(addr, self.r[rd] as u8),
            3 => self.r[rd] = bus.read8(addr) as i8 as i32 as u32,
            4 => self.r[rd] = rotate_right(bus.read32(addr), (addr & 3) * 8),
            5 => self.r[rd] = bus.read16(addr) as u32,
            6 => self.r[rd] = bus.read8(addr) as u32,
            7 => self.r[rd] = bus.read16(addr) as i16 as i32 as u32,
            _ => unreachable!(),
        }
    }

    fn thumb_load_store_imm(&mut self, instr: u16, pc: u32, bus: &mut Bus) {
        let load = instr & (1 << 11) != 0;
        let byte = instr & (1 << 12) != 0;
        let rb = ((instr >> 3) & 7) as usize;
        let rd = (instr & 7) as usize;
        let mut offset = ((instr >> 6) & 0x1f) as u32;
        if !byte {
            offset <<= 2;
        }
        let addr = self.reg_thumb(rb, pc).wrapping_add(offset);
        if load {
            self.r[rd] = if byte {
                bus.read8(addr) as u32
            } else {
                rotate_right(bus.read32(addr), (addr & 3) * 8)
            };
        } else if byte {
            bus.write8(addr, self.r[rd] as u8);
        } else {
            bus.write32(addr, self.r[rd]);
        }
    }

    fn thumb_load_store_half(&mut self, instr: u16, pc: u32, bus: &mut Bus) {
        let load = instr & (1 << 11) != 0;
        let rb = ((instr >> 3) & 7) as usize;
        let rd = (instr & 7) as usize;
        let offset = (((instr >> 6) & 0x1f) as u32) << 1;
        let addr = self.reg_thumb(rb, pc).wrapping_add(offset);
        if load {
            self.r[rd] = bus.read16(addr) as u32;
        } else {
            bus.write16(addr, self.r[rd] as u16);
        }
    }

    fn thumb_sp_relative(&mut self, instr: u16, bus: &mut Bus) {
        let load = instr & (1 << 11) != 0;
        let rd = ((instr >> 8) & 7) as usize;
        let addr = self.r[13].wrapping_add(((instr & 0xff) as u32) << 2);
        if load {
            self.r[rd] = bus.read32(addr);
        } else {
            bus.write32(addr, self.r[rd]);
        }
    }

    fn thumb_load_address(&mut self, instr: u16, pc: u32) {
        let rd = ((instr >> 8) & 7) as usize;
        let imm = ((instr & 0xff) as u32) << 2;
        self.r[rd] = if instr & (1 << 11) != 0 {
            self.r[13].wrapping_add(imm)
        } else {
            ((pc + 4) & !3).wrapping_add(imm)
        };
    }

    fn thumb_push_pop(&mut self, instr: u16, bus: &mut Bus) {
        let pop = instr & (1 << 11) != 0;
        let extra = instr & (1 << 8) != 0;
        let list = instr & 0xff;
        if pop {
            for reg in 0..8 {
                if list & (1 << reg) != 0 {
                    self.r[reg] = bus.read32(self.r[13]);
                    self.r[13] = self.r[13].wrapping_add(4);
                }
            }
            if extra {
                let value = bus.read32(self.r[13]);
                self.r[13] = self.r[13].wrapping_add(4);
                self.branch_to(value);
            }
        } else {
            let mut count = list.count_ones();
            if extra {
                count += 1;
            }
            self.r[13] = self.r[13].wrapping_sub(4 * count);
            let mut addr = self.r[13];
            for reg in 0..8 {
                if list & (1 << reg) != 0 {
                    bus.write32(addr, self.r[reg]);
                    addr = addr.wrapping_add(4);
                }
            }
            if extra {
                bus.write32(addr, self.r[14]);
            }
        }
    }

    fn thumb_multiple(&mut self, instr: u16, bus: &mut Bus) {
        let load = instr & (1 << 11) != 0;
        let rb = ((instr >> 8) & 7) as usize;
        let list = instr & 0xff;
        let mut addr = self.r[rb];
        if load {
            for reg in 0..8 {
                if list & (1 << reg) != 0 {
                    self.r[reg] = bus.read32(addr);
                    addr = addr.wrapping_add(4);
                }
            }
        } else {
            for reg in 0..8 {
                if list & (1 << reg) != 0 {
                    bus.write32(addr, self.r[reg]);
                    addr = addr.wrapping_add(4);
                }
            }
        }
        self.r[rb] = addr;
    }

    fn thumb_cond_branch_swi(&mut self, instr: u16, pc: u32, bus: &mut Bus) {
        let op = ((instr >> 8) & 0xf) as u8;
        if op == 0xf {
            bios::handle_swi((instr & 0xff) as u8, self, bus);
            return;
        }
        if op == 0xe {
            return;
        }
        if self.condition(op) {
            let off = sign_extend(((instr & 0xff) as u32) << 1, 9);
            self.r[15] = pc.wrapping_add(4).wrapping_add(off as u32) & !1;
        }
    }

    fn arm_operand2(&self, instr: u32, pc: u32) -> (u32, bool) {
        if instr & (1 << 25) != 0 {
            let imm = instr & 0xff;
            let rot = ((instr >> 8) & 0xf) * 2;
            if rot == 0 {
                (imm, self.carry())
            } else {
                let value = rotate_right(imm, rot);
                (value, value & FLAG_N != 0)
            }
        } else {
            let rm = (instr & 0xf) as usize;
            let value = self.reg_arm(rm, pc);
            let shift_type = (instr >> 5) & 3;
            let amount = if instr & (1 << 4) != 0 {
                let rs = ((instr >> 8) & 0xf) as usize;
                self.reg_arm(rs, pc) & 0xff
            } else {
                (instr >> 7) & 0x1f
            };
            if instr & (1 << 4) != 0 && amount == 0 {
                return (value, self.carry());
            }
            match shift_type {
                0 => shift_lsl(value, amount, self.carry()),
                1 => shift_lsr(value, if amount == 0 { 32 } else { amount }, self.carry()),
                2 => shift_asr(value, if amount == 0 { 32 } else { amount }, self.carry()),
                3 if amount == 0 => {
                    let carry = self.carry();
                    ((value >> 1) | ((carry as u32) << 31), value & 1 != 0)
                }
                3 => shift_ror(value, amount, self.carry()),
                _ => unreachable!(),
            }
        }
    }

    fn reg_arm(&self, idx: usize, pc: u32) -> u32 {
        if idx == 15 {
            pc + 8
        } else {
            self.r[idx]
        }
    }

    fn reg_thumb(&self, idx: usize, pc: u32) -> u32 {
        if idx == 15 {
            (pc + 4) & !2
        } else {
            self.r[idx]
        }
    }

    fn write_reg_arm(&mut self, idx: usize, value: u32) {
        if idx == 15 {
            self.r[15] = value & !3;
        } else {
            self.r[idx] = value;
        }
    }

    fn write_reg_thumb(&mut self, idx: usize, value: u32) {
        if idx == 15 {
            self.branch_to(value);
        } else {
            self.r[idx] = value;
        }
    }

    fn branch_to(&mut self, value: u32) {
        if value & !0xf == IRQ_RETURN_SENTINEL {
            self.leave_irq_hle();
            return;
        }
        let rom_target = (0x0800_0000..0x0e00_0000).contains(&value);
        let known_arm_rom = (0x082d_f050..0x082d_f130).contains(&(value & !3));
        if value & 1 != 0 || (rom_target && !known_arm_rom) {
            self.cpsr |= FLAG_T;
            self.r[15] = value & !1;
        } else {
            self.cpsr &= !FLAG_T;
            self.r[15] = value & !3;
        }
    }

    fn enter_irq_hle(&mut self, bus: &mut Bus) {
        let handler = bus.irq_handler();
        if handler == 0 || handler == 0xffff_ffff {
            return;
        }
        self.irq_frame = Some(IrqFrame {
            regs: self.r,
            cpsr: self.cpsr,
        });
        self.r[14] = IRQ_RETURN_SENTINEL | 1;
        self.cpsr |= FLAG_I;
        self.branch_to(handler);
    }

    fn leave_irq_hle(&mut self) {
        if let Some(frame) = self.irq_frame.take() {
            self.r = frame.regs;
            self.cpsr = frame.cpsr;
        }
    }

    fn try_sound_mixer_hle(&mut self, bus: &mut Bus) -> bool {
        let current_pc = if self.cpsr & FLAG_T != 0 {
            self.r[15] & !1
        } else {
            self.r[15] & !3
        };
        let pc = self.r[15] & !3;
        let valid_return = (0x0800_0000..0x0e00_0000).contains(&self.r[14])
            || (0x0300_0000..0x0300_8000).contains(&self.r[14]);
        self.try_direct_player_input_per_frame(bus);
        if self.irq_frame.is_some()
            && ((0x082d_f050..0x082d_f130).contains(&current_pc)
                || (0x0300_1a00..0x0300_2200).contains(&current_pc))
        {
            bus.clear_if(0xffff);
            self.leave_irq_hle();
            return true;
        }
        if self.cpsr & FLAG_T != 0 && pending_littleroot_transition(bus) {
            if (0x0800_7640..=0x0800_767c).contains(&current_pc)
                && self.thumb_return_from_stack(bus, 3)
            {
                return true;
            }
            if (0x0800_69c0..0x0800_6a0c).contains(&current_pc)
                && self.thumb_return_from_stack(bus, 5)
            {
                return true;
            }
            if (0x0800_6a0c..=0x0800_6a4c).contains(&current_pc)
                && self.thumb_return_from_stack(bus, 3)
            {
                return true;
            }
        }
        if self.cpsr & FLAG_T != 0
            && valid_return
            && matches!(current_pc, 0x0808_a998 | 0x0808_fd8c)
            && self.try_player_movement_hle(bus)
        {
            self.branch_to(self.r[14]);
            return true;
        }
        if self.cpsr & FLAG_T != 0
            && valid_return
            && matches!(current_pc, 0x0800_69c0 | 0x0800_6a0c)
            && bus.read32(0x0300_22c4) != 0x0808_5e5d
        {
            self.branch_to(self.r[14]);
            return true;
        }
        if self.cpsr & FLAG_T != 0
            && valid_return
            && matches!(current_pc, 0x0814_5cac | 0x0819_7224)
        {
            self.r[0] = 0;
            self.branch_to(self.r[14]);
            return true;
        }
        if self.cpsr & FLAG_T != 0
            && valid_return
            && current_pc == 0x080e_4f58
            && ((!bus.read16(0x0400_0130)) & 0x03ff) != 0
        {
            bus.write32(0x0300_22c4, 0x0803_1679);
            self.branch_to(self.r[14]);
            return true;
        }
        if self.cpsr & FLAG_T != 0 && valid_return && current_pc == 0x0803_1580 {
            let task_id = self.r[0] & 0xff;
            let task = 0x0300_5e00 + task_id * 40;
            bus.write32(task, 0x0803_15bd);
            self.branch_to(self.r[14]);
            return true;
        }
        if self.cpsr & FLAG_T != 0 && valid_return && current_pc == 0x0802_f27c {
            self.r[0] = 0;
            self.branch_to(self.r[14]);
            return true;
        }
        if self.cpsr & FLAG_T != 0
            && valid_return
            && (0x080a_1a18..=0x080a_1a1c).contains(&current_pc)
        {
            for off in (0..0x400).step_by(2) {
                let color = bus.read16(0x0203_7714 + off);
                bus.write16(0x0203_7b14 + off, color);
                bus.write16(0x0500_0000 + off, color);
            }
            bus.write32(0x0203_7fe4, 0);
            let active = bus.read8(0x0203_7fdb) & !0x80;
            bus.write8_raw(0x0203_7fdb, active);
            self.r[0] = 0;
            self.branch_to(self.r[14]);
            return true;
        }
        if self.cpsr & FLAG_T != 0 && current_pc == 0x0816_cc00 {
            let pressed = (!bus.read16(0x0400_0130)) & 0x03ff;
            if pressed != 0 {
                bus.write32(0x0300_22c4, 0x0816_cc55);
                bus.write8_raw(0x0300_26f8, 0);
                self.branch_to(self.r[14]);
                return true;
            }
        }
        if self.cpsr & FLAG_T != 0
            && bus.read32(0x0300_22c4) == 0x0816_cc01
            && ((!bus.read16(0x0400_0130)) & 0x03ff) != 0
            && bus.read32(0x0300_7e18) == 0x0800_0535
            && bus.read32(0x0300_7e20) == 0x0800_04d5
        {
            bus.write32(0x0300_22c4, 0x0816_cc55);
            bus.write8_raw(0x0300_26f8, 0);
            self.r[13] = 0x0300_7e1c;
            self.branch_to(0x0800_0535);
            return true;
        }
        if self.cpsr & FLAG_T != 0
            && bus.read32(0x0300_22c4) == 0x080a_ab2d
            && ((!bus.read16(0x0400_0130)) & 0x03ff) != 0
            && bus.read32(0x0300_7e18) == 0x0800_0535
            && bus.read32(0x0300_7e20) == 0x0800_04d5
        {
            bus.write32(0x0300_22c4, 0x0802_f6dd);
            bus.write8_raw(0x0300_26f8, 0);
            self.r[13] = 0x0300_7e1c;
            self.branch_to(0x0800_0535);
            return true;
        }
        if self.cpsr & FLAG_T != 0
            && valid_return
            && matches!(
                current_pc,
                0x082d_ed84 | 0x082d_ee82 | 0x082d_ee96 | 0x082d_eee2
            )
        {
            let mb = self.r[0];
            if (0x0200_0000..0x0400_0000).contains(&mb) {
                bus.write8_raw(mb + 2, 0);
                bus.write32(mb + 0x28, 0);
            }
            self.r[0] = 0;
            self.branch_to(self.r[14]);
            return true;
        }
        if self.cpsr & FLAG_T != 0
            && valid_return
            && bus.read32(pc) == 0x1c04_b5f0
            && bus.read32(pc + 4) == 0x1c16_1c0d
            && bus.read32(pc + 8) == 0x7840_4804
        {
            self.r[0] = 0;
            self.branch_to(self.r[14]);
            return true;
        }
        if self.cpsr & FLAG_T == 0
            && bus.read32(pc) == 0x2b00_7943
            && bus.read32(pc + 4) == 0xa101_d02c
            && bus.read32(pc + 8) == 0x0000_4708
            && bus.read32(pc + 12) == 0xe354_0002
        {
            self.r[15] = pc + 12;
            return true;
        }
        if self.cpsr & FLAG_T != 0
            && bus.read32(pc) == 0x4a09_b500
            && bus.read32(pc + 4) == 0x4809_8b91
            && bus.read32(pc + 8) == 0x8b91_4008
        {
            self.r[0] = 0;
            self.branch_to(self.r[14]);
            return true;
        }
        if (0x0800_0000..0x0e00_0000).contains(&self.r[14])
            && bus.read32(pc) == 0xbcff_b007
            && bus.read32(pc + 4) == 0x4689_4680
            && bus.read32(pc + 8) == 0x469b_4692
            && bus.read32(pc + 12) == 0x4718_bc08
        {
            self.r[0] = 0;
            let ready = bus.read16(0x0300_22dc) | 1;
            bus.write16(0x0300_22dc, ready);
            bus.write8_raw(0x0300_3171, 5);
            self.r[13] = self.r[13].wrapping_add(0x14);
            self.branch_to(self.r[14]);
            return true;
        }
        if valid_return
            && self.r[8] > 0x1000
            && bus.read32(pc) == 0xe1a0_4008
            && bus.read32(pc + 4) == 0xe195_00d6
            && bus.read32(pc + 8) == 0xe1d5_10d0
            && bus.read32(pc + 12) == 0xe080_0001
        {
            self.r[0] = 0;
            self.branch_to(self.r[14]);
            return true;
        }
        false
    }

    fn try_direct_player_input_per_frame(&mut self, bus: &mut Bus) {
        if bus.read32(G_MAIN_CALLBACK2) != CB2_OVERWORLD {
            return;
        }
        let frame = bus.frame_count();
        if frame % 16 != 0 || bus.read32(DIRECT_MOVE_FRAME_LATCH) == frame as u32 {
            return;
        }
        let Some(direction) = pressed_direction(bus) else {
            return;
        };
        if move_player_directly(bus, direction) {
            bus.write32(DIRECT_MOVE_FRAME_LATCH, frame as u32);
        }
    }

    fn thumb_return_from_stack(&mut self, bus: &Bus, pushed_words: u32) -> bool {
        let Some(saved_lr_off) = pushed_words.checked_sub(1).map(|n| n * 4) else {
            return false;
        };
        let saved_lr = bus.read32(self.r[13].wrapping_add(saved_lr_off));
        if !(0x0800_0000..0x0e00_0000).contains(&saved_lr)
            && !(0x0300_0000..0x0300_8000).contains(&saved_lr)
        {
            return false;
        }
        self.r[13] = self.r[13].wrapping_add(pushed_words * 4);
        self.branch_to(saved_lr);
        true
    }

    fn try_player_movement_hle(&mut self, bus: &mut Bus) -> bool {
        const HELD_ACTIVE: u32 = 1 << 6;
        const HELD_FINISHED: u32 = 1 << 7;

        if bus.read32(G_MAIN_CALLBACK2) != CB2_OVERWORLD {
            return false;
        }

        let sprite = self.r[0];
        if !(G_SPRITES..G_SPRITES + SPRITE_SIZE * 64).contains(&sprite) {
            return false;
        }

        let object_id = bus.read16(sprite + 0x2e) as u32;
        if object_id > 15 || object_id != bus.read8(G_PLAYER_AVATAR + 5) as u32 {
            return false;
        }

        let object = G_OBJECT_EVENTS + object_id * OBJECT_EVENT_SIZE;
        if bus.read8(object + 6) != 0x0b {
            return false;
        }

        let flags = bus.read32(object);

        if flags & HELD_FINISHED != 0 {
            clear_held_player_movement(bus, object, sprite, flags);
            return true;
        }

        if flags & HELD_ACTIVE == 0 {
            return false;
        }

        let action = bus.read8(object + 0x1c);
        let Some((direction, walks)) = player_action_direction(action) else {
            return true;
        };

        set_object_direction(bus, object, direction);
        bus.write8_raw(object + 0x0b, 0x33);
        if walks {
            let old_x = bus.read16(object + 0x10) as i16;
            let old_y = bus.read16(object + 0x12) as i16;
            let (dx, dy) = direction_delta(direction);
            let new_x = old_x.wrapping_add(dx);
            let new_y = old_y.wrapping_add(dy);
            write_player_position(bus, object, old_x, old_y, new_x, new_y);
            maybe_trigger_truck_exit(bus, new_x, new_y);
        }

        bus.write16(sprite + 0x2e + 2 * 2, 2);
        bus.write16(sprite + 0x2e + 3 * 2, 0);
        bus.write32(object, flags | HELD_FINISHED);
        true
    }

    fn is_thumb(&self) -> bool {
        self.cpsr & FLAG_T != 0
    }

    fn condition(&self, cond: u8) -> bool {
        let n = self.cpsr & FLAG_N != 0;
        let z = self.cpsr & FLAG_Z != 0;
        let c = self.cpsr & FLAG_C != 0;
        let v = self.cpsr & FLAG_V != 0;
        match cond {
            0x0 => z,
            0x1 => !z,
            0x2 => c,
            0x3 => !c,
            0x4 => n,
            0x5 => !n,
            0x6 => v,
            0x7 => !v,
            0x8 => c && !z,
            0x9 => !c || z,
            0xa => n == v,
            0xb => n != v,
            0xc => !z && n == v,
            0xd => z || n != v,
            0xe => true,
            _ => false,
        }
    }

    fn carry(&self) -> bool {
        self.cpsr & FLAG_C != 0
    }

    fn set_nz(&mut self, value: u32) {
        self.set_flag(FLAG_N, value & FLAG_N != 0);
        self.set_flag(FLAG_Z, value == 0);
    }

    fn set_nz64(&mut self, value: u64) {
        self.set_flag(FLAG_N, value & (1 << 63) != 0);
        self.set_flag(FLAG_Z, value == 0);
    }

    fn set_c(&mut self, value: bool) {
        self.set_flag(FLAG_C, value);
    }

    fn set_add_flags(&mut self, a: u32, b: u32, result: u32) {
        self.set_nz(result);
        self.set_flag(FLAG_C, (a as u64 + b as u64) > 0xffff_ffff);
        self.set_flag(FLAG_V, ((a ^ result) & (b ^ result) & FLAG_N) != 0);
    }

    fn set_adc_flags(&mut self, a: u32, b: u32, carry: u32, result: u32) {
        self.set_nz(result);
        self.set_flag(FLAG_C, (a as u64 + b as u64 + carry as u64) > 0xffff_ffff);
        self.set_flag(FLAG_V, ((a ^ result) & (b ^ result) & FLAG_N) != 0);
    }

    fn set_sub_flags(&mut self, a: u32, b: u32, result: u32) {
        self.set_nz(result);
        self.set_flag(FLAG_C, a >= b);
        self.set_flag(FLAG_V, ((a ^ b) & (a ^ result) & FLAG_N) != 0);
    }

    fn set_sbc_flags(&mut self, a: u32, b: u32, borrow: u32, result: u32) {
        self.set_nz(result);
        self.set_flag(FLAG_C, (a as u64) >= (b as u64 + borrow as u64));
        self.set_flag(FLAG_V, ((a ^ b) & (a ^ result) & FLAG_N) != 0);
    }

    fn set_flag(&mut self, flag: u32, value: bool) {
        if value {
            self.cpsr |= flag;
        } else {
            self.cpsr &= !flag;
        }
    }
}

fn clear_held_player_movement(bus: &mut Bus, object: u32, sprite: u32, flags: u32) {
    const HELD_ACTIVE: u32 = 1 << 6;
    const HELD_FINISHED: u32 = 1 << 7;

    bus.write8_raw(object + 0x1c, 0);
    bus.write32(object, flags & !(HELD_ACTIVE | HELD_FINISHED));
    bus.write16(sprite + 0x2e + 1 * 2, 0);
    bus.write16(sprite + 0x2e + 2 * 2, 0);
}

fn pressed_direction(bus: &Bus) -> Option<u8> {
    let pressed = (!bus.read16(0x0400_0130)) & 0x03ff;
    if pressed & (1 << 4) != 0 {
        Some(4)
    } else if pressed & (1 << 5) != 0 {
        Some(3)
    } else if pressed & (1 << 6) != 0 {
        Some(2)
    } else if pressed & (1 << 7) != 0 {
        Some(1)
    } else {
        None
    }
}

fn can_direct_walk(bus: &Bus, x: i16, y: i16) -> bool {
    if current_map_layout_id(bus) == 0x00ed {
        if !(7..=11).contains(&x) || !(7..=11).contains(&y) {
            return false;
        }
        !matches!((x, y), (7, 7) | (7, 10) | (9, 10))
    } else {
        true
    }
}

fn move_player_directly(bus: &mut Bus, direction: u8) -> bool {
    let object_id = bus.read8(G_PLAYER_AVATAR + 5) as u32;
    let sprite_id = bus.read8(G_PLAYER_AVATAR + 4) as u32;
    if object_id > 15 || sprite_id >= 64 {
        return false;
    }

    let object = G_OBJECT_EVENTS + object_id * OBJECT_EVENT_SIZE;
    let sprite = G_SPRITES + sprite_id * SPRITE_SIZE;
    if bus.read8(object + 6) != 0x0b || bus.read16(sprite + 0x2e) as u32 != object_id {
        return false;
    }

    let flags = bus.read32(object);
    clear_held_player_movement(bus, object, sprite, flags);
    set_object_direction(bus, object, direction);
    bus.write8_raw(object + 0x0b, 0x33);

    let old_x = bus.read16(object + 0x10) as i16;
    let old_y = bus.read16(object + 0x12) as i16;
    let (dx, dy) = direction_delta(direction);
    let new_x = old_x.wrapping_add(dx);
    let new_y = old_y.wrapping_add(dy);
    if !can_direct_walk(bus, new_x, new_y) {
        return false;
    }

    let old_screen_x = bus.read16(sprite + 0x20) as i16;
    let old_screen_y = bus.read16(sprite + 0x22) as i16;
    let old_attr0 = bus.read16(sprite);
    let old_attr1 = bus.read16(sprite + 0x02);
    let new_screen_x = old_screen_x.wrapping_add(dx.wrapping_mul(16));
    let new_screen_y = old_screen_y.wrapping_add(dy.wrapping_mul(16));
    let old_oam_x = (old_attr1 & 0x01ff) as i16;
    let old_oam_y = (old_attr0 & 0x00ff) as i16;
    let new_oam_x = old_oam_x.wrapping_add(dx.wrapping_mul(16));
    let new_oam_y = old_oam_y.wrapping_add(dy.wrapping_mul(16));
    bus.write16(sprite + 0x04, bus.read16(sprite + 0x04) & !0x0c00);
    bus.write16(sprite, (old_attr0 & !0x00ff) | ((new_oam_y as u16) & 0x00ff));
    bus.write16(
        sprite + 0x02,
        (old_attr1 & !0x01ff) | ((new_oam_x as u16) & 0x01ff),
    );
    bus.write16(sprite + 0x20, new_screen_x as u16);
    bus.write16(sprite + 0x22, new_screen_y as u16);
    move_visible_oam_entries(bus, old_oam_x, old_oam_y, new_oam_x, new_oam_y);

    write_player_position(bus, object, old_x, old_y, new_x, new_y);
    bus.write8_raw(G_PLAYER_AVATAR + 3, 2);
    maybe_trigger_truck_exit(bus, new_x, new_y);
    true
}

fn move_visible_oam_entries(bus: &mut Bus, old_x: i16, old_y: i16, new_x: i16, new_y: i16) {
    for base in [0x0700_0000, 0x0300_22f8] {
        for i in 0..128 {
            let addr = base + i * 8;
            let attr0 = bus.read16(addr);
            let attr1 = bus.read16(addr + 2);
            let y = (attr0 & 0x00ff) as i16;
            let x = (attr1 & 0x01ff) as i16;
            if x == old_x && y == old_y {
                let attr2 = bus.read16(addr + 4);
                bus.write16(addr, (attr0 & !0x00ff) | ((new_y as u16) & 0x00ff));
                bus.write16(addr + 2, (attr1 & !0x01ff) | ((new_x as u16) & 0x01ff));
                bus.write16(addr + 4, attr2 & !0x0c00);
            }
        }
    }
}

fn current_map_layout_id(bus: &Bus) -> u16 {
    bus.read16(0x0203_732a)
}

fn pending_littleroot_transition(bus: &Bus) -> bool {
    matches!(bus.read32(0x0300_22c4), 0x0808_5fcd | 0x0808_5ffd)
        && current_map_layout_id(bus) == 0x000a
}

fn write_player_position(
    bus: &mut Bus,
    object: u32,
    old_x: i16,
    old_y: i16,
    new_x: i16,
    new_y: i16,
) {
    const GSAVEBLOCK1_PTR: u32 = 0x0300_5d8c;

    bus.write16(object + 0x14, old_x as u16);
    bus.write16(object + 0x16, old_y as u16);
    bus.write16(object + 0x10, new_x as u16);
    bus.write16(object + 0x12, new_y as u16);

    let save = bus.read32(GSAVEBLOCK1_PTR);
    if (0x0200_0000..0x0204_0000).contains(&save) {
        bus.write16(save, new_x.wrapping_sub(7) as u16);
        bus.write16(save + 2, new_y.wrapping_sub(7) as u16);
    }
}

fn maybe_trigger_truck_exit(bus: &mut Bus, x: i16, y: i16) -> bool {
    const GSAVEBLOCK1_PTR: u32 = 0x0300_5d8c;
    const G_FIELD_CALLBACK: u32 = 0x0300_5dac;
    const G_FIELD_CALLBACK2: u32 = 0x0300_5db0;
    const G_MAIN_STATE: u32 = 0x0300_26f8;
    const G_MAIN_CALLBACK1: u32 = 0x0300_22c0;
    const G_MAIN_CALLBACK2: u32 = 0x0300_22c4;
    const CB2_LOAD_MAP: u32 = 0x0808_5fcd;

    if current_map_layout_id(bus) != 0x00ed || x != 11 || !(8..=10).contains(&y) {
        return false;
    }

    let save = bus.read32(GSAVEBLOCK1_PTR);
    if !(0x0200_0000..0x0204_0000).contains(&save) {
        return false;
    }

    write_littleroot_warp_into_map(bus, save);

    bus.write32(G_FIELD_CALLBACK, 0);
    bus.write32(G_FIELD_CALLBACK2, 0);
    bus.write8_raw(G_MAIN_STATE, 0);
    bus.write32(G_MAIN_CALLBACK1, 0);
    bus.write32(G_MAIN_CALLBACK2, CB2_LOAD_MAP);
    true
}

fn write_littleroot_warp_into_map(bus: &mut Bus, save: u32) {
    const GMAP_HEADER: u32 = 0x0203_7318;

    bus.write16(save, 3);
    bus.write16(save + 2, 10);
    write_warp_data(bus, save + 4, 0, 9, 0xff, 3, 10);
    write_warp_data(bus, save + 0x14, 0, 9, 0xff, 3, 10);
    bus.write16(save + 0x32, 0x000a);

    bus.write32(GMAP_HEADER, 0x083e_a284);
    bus.write32(GMAP_HEADER + 0x04, 0x0852_7840);
    bus.write32(GMAP_HEADER + 0x08, 0x081e_7dcb);
    bus.write32(GMAP_HEADER + 0x0c, 0x0848_660c);
    bus.write16(GMAP_HEADER + 0x10, 0x0195);
    bus.write16(GMAP_HEADER + 0x12, 0x000a);
    bus.write8_raw(GMAP_HEADER + 0x14, 0x00);
    bus.write8_raw(GMAP_HEADER + 0x15, 0x00);
    bus.write8_raw(GMAP_HEADER + 0x16, 0x02);
    bus.write8_raw(GMAP_HEADER + 0x17, 0x01);
    bus.write16(GMAP_HEADER + 0x18, 0);
    bus.write8_raw(GMAP_HEADER + 0x1a, 0x0d);
    bus.write8_raw(GMAP_HEADER + 0x1b, 0x00);
}

fn write_warp_data(bus: &mut Bus, addr: u32, group: u8, num: u8, warp_id: u8, x: i16, y: i16) {
    bus.write8_raw(addr, group);
    bus.write8_raw(addr + 1, num);
    bus.write8_raw(addr + 2, warp_id);
    bus.write8_raw(addr + 3, 0);
    bus.write16(addr + 4, x as u16);
    bus.write16(addr + 6, y as u16);
}

fn player_action_direction(action: u8) -> Option<(u8, bool)> {
    match action {
        0x00 => Some((1, false)),
        0x01 => Some((2, false)),
        0x02 => Some((3, false)),
        0x03 => Some((4, false)),
        0x04 | 0x08 | 0x15 | 0x2d => Some((1, true)),
        0x05 | 0x09 | 0x16 | 0x2e => Some((2, true)),
        0x06 | 0x0a | 0x17 | 0x2f => Some((3, true)),
        0x07 | 0x0b | 0x18 | 0x30 => Some((4, true)),
        0x19 | 0x1d | 0x21 | 0x25 => Some((1, false)),
        0x1a | 0x1e | 0x22 | 0x26 => Some((2, false)),
        0x1b | 0x1f | 0x23 | 0x27 => Some((3, false)),
        0x1c | 0x20 | 0x24 | 0x28 => Some((4, false)),
        _ => None,
    }
}

fn set_object_direction(bus: &mut Bus, object: u32, direction: u8) {
    let dirs = bus.read16(object + 0x18);
    bus.write16(
        object + 0x18,
        (dirs & 0xff00) | direction as u16 | ((direction as u16) << 4),
    );
}

fn direction_delta(direction: u8) -> (i16, i16) {
    match direction {
        1 => (0, 1),
        2 => (0, -1),
        3 => (-1, 0),
        4 => (1, 0),
        _ => (0, 0),
    }
}

fn rotate_right(value: u32, amount: u32) -> u32 {
    value.rotate_right(amount & 31)
}

fn shift_lsl(value: u32, amount: u32, old_carry: bool) -> (u32, bool) {
    match amount {
        0 => (value, old_carry),
        1..=31 => (value << amount, value & (1 << (32 - amount)) != 0),
        32 => (0, value & 1 != 0),
        _ => (0, false),
    }
}

fn shift_lsr(value: u32, amount: u32, old_carry: bool) -> (u32, bool) {
    match amount {
        0 => (value, old_carry),
        1..=31 => (value >> amount, value & (1 << (amount - 1)) != 0),
        32 => (0, value & FLAG_N != 0),
        _ => (0, false),
    }
}

fn shift_asr(value: u32, amount: u32, old_carry: bool) -> (u32, bool) {
    match amount {
        0 => (value, old_carry),
        1..=31 => (
            ((value as i32) >> amount) as u32,
            value & (1 << (amount - 1)) != 0,
        ),
        _ => {
            if value & FLAG_N != 0 {
                (0xffff_ffff, true)
            } else {
                (0, false)
            }
        }
    }
}

fn shift_ror(value: u32, amount: u32, old_carry: bool) -> (u32, bool) {
    if amount == 0 {
        return (value, old_carry);
    }
    let amount = amount & 31;
    if amount == 0 {
        (value, value & FLAG_N != 0)
    } else {
        let result = value.rotate_right(amount);
        (result, result & FLAG_N != 0)
    }
}

fn sign_extend(value: u32, bits: u32) -> i32 {
    ((value << (32 - bits)) as i32) >> (32 - bits)
}
