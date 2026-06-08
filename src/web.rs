use crate::gba::{Button, Gba};
use crate::{SCREEN_HEIGHT, SCREEN_WIDTH};

pub struct WebGba {
    gba: Gba,
    rgba_framebuffer: Vec<u8>,
}

impl WebGba {
    pub fn new(rom: Vec<u8>) -> Self {
        Self {
            gba: Gba::new(rom, None, false),
            rgba_framebuffer: vec![0; SCREEN_WIDTH * SCREEN_HEIGHT * 4],
        }
    }

    pub fn set_button(&mut self, button: WebButton, pressed: bool) {
        self.gba.set_button(button.into(), pressed);
    }

    pub fn run_frame(&mut self) -> &[u8] {
        self.gba.run_frame();
        write_rgba(self.gba.framebuffer(), &mut self.rgba_framebuffer);
        &self.rgba_framebuffer
    }

    pub fn debug_summary(&self) -> String {
        self.gba.debug_summary()
    }
}

#[derive(Clone, Copy)]
pub enum WebButton {
    A,
    B,
    Select,
    Start,
    Right,
    Left,
    Up,
    Down,
    R,
    L,
}

impl From<WebButton> for Button {
    fn from(button: WebButton) -> Self {
        match button {
            WebButton::A => Button::A,
            WebButton::B => Button::B,
            WebButton::Select => Button::Select,
            WebButton::Start => Button::Start,
            WebButton::Right => Button::Right,
            WebButton::Left => Button::Left,
            WebButton::Up => Button::Up,
            WebButton::Down => Button::Down,
            WebButton::R => Button::R,
            WebButton::L => Button::L,
        }
    }
}

fn write_rgba(framebuffer: &[u32], out: &mut [u8]) {
    for (pixel, rgba) in framebuffer.iter().zip(out.chunks_exact_mut(4)) {
        rgba[0] = ((pixel >> 16) & 0xff) as u8;
        rgba[1] = ((pixel >> 8) & 0xff) as u8;
        rgba[2] = (pixel & 0xff) as u8;
        rgba[3] = 0xff;
    }
}

#[cfg(test)]
mod tests {
    use super::write_rgba;

    #[test]
    fn converts_internal_framebuffer_to_canvas_rgba() {
        let framebuffer = [0x0012_3456, 0x00ab_cdef];
        let mut out = [0; 8];

        write_rgba(&framebuffer, &mut out);

        assert_eq!(out, [0x12, 0x34, 0x56, 0xff, 0xab, 0xcd, 0xef, 0xff]);
    }
}
