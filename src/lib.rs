pub mod bios;
pub mod bus;
pub mod cartridge;
pub mod cpu;
pub mod edu;
pub mod gba;
pub mod ppu;
pub mod web;

pub const SCREEN_WIDTH: usize = 240;
pub const SCREEN_HEIGHT: usize = 160;
pub const FRAME_CYCLES: u32 = 280_896;
