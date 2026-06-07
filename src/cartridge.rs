use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub fn load_rom_file(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    if bytes.starts_with(b"PK\x03\x04") {
        return load_zip(bytes);
    }
    Ok(bytes)
}

fn load_zip(bytes: Vec<u8>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let cursor = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(cursor)?;
    let index = find_gba_entry(&mut zip).ok_or("zip does not contain a .gba entry")?;
    let mut file = zip.by_index(index)?;
    let mut rom = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut rom)?;
    Ok(rom)
}

fn find_gba_entry<R: Read + Seek>(zip: &mut zip::ZipArchive<R>) -> Option<usize> {
    let mut fallback = None;
    for i in 0..zip.len() {
        let file = zip.by_index(i).ok()?;
        if file.is_dir() {
            continue;
        }
        if fallback.is_none() {
            fallback = Some(i);
        }
        if file.name().to_ascii_lowercase().ends_with(".gba") {
            return Some(i);
        }
    }
    fallback
}

#[derive(Deserialize, Serialize)]
pub struct Flash {
    data: Vec<u8>,
    path: Option<PathBuf>,
    dirty: bool,
    state: FlashState,
    id_mode: bool,
    bank: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum FlashState {
    Ready,
    GotAa,
    GotAa55,
    Program,
    Erase1,
    EraseGotAa,
    EraseGotAa55,
    BankSelect,
}

impl Flash {
    pub fn new(path: Option<PathBuf>) -> Self {
        let mut data = vec![0xff; 128 * 1024];
        if let Some(path) = path.as_ref() {
            if let Ok(existing) = std::fs::read(path) {
                let len = existing.len().min(data.len());
                data[..len].copy_from_slice(&existing[..len]);
            }
        }
        Self {
            data,
            path,
            dirty: false,
            state: FlashState::Ready,
            id_mode: false,
            bank: 0,
        }
    }

    pub fn read(&self, addr: u32) -> u8 {
        let off = (addr as usize) & 0xffff;
        if self.id_mode {
            return match off {
                0 => 0xc2,
                1 => 0x09,
                _ => 0xff,
            };
        }
        let index = self.bank * 0x10000 + off;
        self.data[index % self.data.len()]
    }

    pub fn write(&mut self, addr: u32, value: u8) {
        let off = (addr as usize) & 0xffff;
        match self.state {
            FlashState::Ready => match (off, value) {
                (0x5555, 0xaa) => self.state = FlashState::GotAa,
                (_, 0xf0) => self.id_mode = false,
                _ => {}
            },
            FlashState::GotAa => {
                self.state = if off == 0x2aaa && value == 0x55 {
                    FlashState::GotAa55
                } else {
                    FlashState::Ready
                };
            }
            FlashState::GotAa55 => {
                self.state = FlashState::Ready;
                match value {
                    0x90 if off == 0x5555 => self.id_mode = true,
                    0xf0 if off == 0x5555 => self.id_mode = false,
                    0xa0 if off == 0x5555 => self.state = FlashState::Program,
                    0x80 if off == 0x5555 => self.state = FlashState::Erase1,
                    0xb0 if off == 0x5555 => self.state = FlashState::BankSelect,
                    _ => {}
                }
            }
            FlashState::Program => {
                self.program_byte(off, value);
                self.state = FlashState::Ready;
            }
            FlashState::Erase1 => {
                self.state = if off == 0x5555 && value == 0xaa {
                    FlashState::EraseGotAa
                } else {
                    FlashState::Ready
                };
            }
            FlashState::EraseGotAa => {
                self.state = if off == 0x2aaa && value == 0x55 {
                    FlashState::EraseGotAa55
                } else {
                    FlashState::Ready
                };
            }
            FlashState::EraseGotAa55 => {
                if off == 0x5555 && value == 0x10 {
                    self.data.fill(0xff);
                    self.dirty = true;
                } else if value == 0x30 {
                    let base = self.bank * 0x10000 + (off & !0x0fff);
                    for byte in &mut self.data[base..base + 0x1000] {
                        *byte = 0xff;
                    }
                    self.dirty = true;
                }
                self.state = FlashState::Ready;
            }
            FlashState::BankSelect => {
                self.bank = (value as usize) & 1;
                self.state = FlashState::Ready;
            }
        }
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        if let Some(path) = self.path.as_ref() {
            std::fs::write(path, &self.data)?;
        }
        self.dirty = false;
        Ok(())
    }

    pub fn flush_if_dirty(&mut self) -> std::io::Result<()> {
        self.flush()
    }

    fn program_byte(&mut self, off: usize, value: u8) {
        let index = self.bank * 0x10000 + off;
        let len = self.data.len();
        self.data[index % len] &= value;
        self.dirty = true;
    }
}
