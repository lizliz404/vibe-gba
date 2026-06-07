use crate::{SCREEN_HEIGHT, SCREEN_WIDTH};
use serde::{Deserialize, Serialize};

const LINE_CYCLES: u32 = 1_232;
const HBLANK_START: u32 = 960;
const VISIBLE_LINES: u16 = 160;
const TOTAL_LINES: u16 = 228;

#[derive(Default)]
pub struct PpuEvents {
    pub hblank: bool,
    pub vblank: bool,
    pub frame: bool,
}

#[derive(Clone, Copy)]
struct Pixel {
    color: u16,
    priority: u8,
    order: u8,
    mask: u16,
    semi: bool,
}

#[derive(Deserialize, Serialize)]
pub struct Ppu {
    pub frame: Vec<u32>,
    line: u16,
    line_cycles: u32,
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            frame: vec![0; SCREEN_WIDTH * SCREEN_HEIGHT],
            line: 0,
            line_cycles: 0,
        }
    }

    pub fn step(
        &mut self,
        mut cycles: u32,
        io: &mut [u8; 0x400],
        vram: &[u8],
        pram: &[u8],
        oam: &[u8],
    ) -> PpuEvents {
        let mut events = PpuEvents::default();
        while cycles > 0 {
            if self.line < VISIBLE_LINES
                && self.line_cycles < HBLANK_START
                && self.line_cycles + cycles >= HBLANK_START
            {
                let used = HBLANK_START - self.line_cycles;
                self.line_cycles = HBLANK_START;
                cycles -= used;
                set_dispstat_bit(io, 1, true);
                events.hblank = true;
                if read16(io, 0x004) & (1 << 4) != 0 {
                    request_irq(io, 1 << 1);
                }
                continue;
            }

            if self.line_cycles + cycles >= LINE_CYCLES {
                let used = LINE_CYCLES - self.line_cycles;
                cycles -= used;
                self.line_cycles = 0;
                set_dispstat_bit(io, 1, false);
                self.line += 1;

                if self.line == VISIBLE_LINES {
                    set_dispstat_bit(io, 0, true);
                    self.render_frame(io, vram, pram, oam);
                    events.vblank = true;
                    events.frame = true;
                    if read16(io, 0x004) & (1 << 3) != 0 || read16(io, 0x200) & 1 != 0 {
                        request_irq(io, 1 << 0);
                    }
                } else if self.line >= TOTAL_LINES {
                    self.line = 0;
                    set_dispstat_bit(io, 0, false);
                }
                write16(io, 0x006, self.line);
                self.check_vcounter(io);
                continue;
            }

            self.line_cycles += cycles;
            cycles = 0;
        }
        events
    }

    fn check_vcounter(&self, io: &mut [u8; 0x400]) {
        let dispstat = read16(io, 0x004);
        let lyc = dispstat >> 8;
        let match_now = self.line == lyc;
        set_dispstat_bit(io, 2, match_now);
        if match_now && dispstat & (1 << 5) != 0 {
            request_irq(io, 1 << 2);
        }
    }

    fn render_frame(&mut self, io: &[u8; 0x400], vram: &[u8], pram: &[u8], oam: &[u8]) {
        let dispcnt = read16(io, 0x000);
        if dispcnt & (1 << 7) != 0 {
            self.frame.fill(0x00ff_ffff);
            return;
        }

        match dispcnt & 7 {
            3 => self.render_mode3(vram),
            4 => self.render_mode4(dispcnt, vram, pram),
            5 => self.render_mode5(dispcnt, vram),
            _ => self.render_layers(dispcnt, io, vram, pram, oam),
        }
    }

    fn render_mode3(&mut self, vram: &[u8]) {
        for y in 0..SCREEN_HEIGHT {
            for x in 0..SCREEN_WIDTH {
                let off = (y * SCREEN_WIDTH + x) * 2;
                self.frame[y * SCREEN_WIDTH + x] = bgr555_to_rgb(read_mem16(vram, off));
            }
        }
    }

    fn render_mode4(&mut self, dispcnt: u16, vram: &[u8], pram: &[u8]) {
        let page = if dispcnt & (1 << 4) != 0 { 0x0a000 } else { 0 };
        for y in 0..SCREEN_HEIGHT {
            for x in 0..SCREEN_WIDTH {
                let index = vram.get(page + y * SCREEN_WIDTH + x).copied().unwrap_or(0) as usize;
                self.frame[y * SCREEN_WIDTH + x] = bgr555_to_rgb(read_mem16(pram, index * 2));
            }
        }
    }

    fn render_mode5(&mut self, dispcnt: u16, vram: &[u8]) {
        let page = if dispcnt & (1 << 4) != 0 { 0x0a000 } else { 0 };
        let backdrop = 0;
        self.frame.fill(backdrop);
        for y in 0..128 {
            for x in 0..160 {
                let off = page + (y * 160 + x) * 2;
                self.frame[y * SCREEN_WIDTH + x] = bgr555_to_rgb(read_mem16(vram, off));
            }
        }
    }

    fn render_layers(
        &mut self,
        dispcnt: u16,
        io: &[u8; 0x400],
        vram: &[u8],
        pram: &[u8],
        oam: &[u8],
    ) {
        let mode = dispcnt & 7;
        let backdrop = read_mem16(pram, 0);
        let obj_window = if dispcnt & (1 << 15) != 0 && dispcnt & (1 << 12) != 0 {
            Some(build_obj_window_mask(dispcnt, vram, oam))
        } else {
            None
        };
        let obj_layer = if dispcnt & (1 << 12) != 0 {
            Some(build_obj_layer(dispcnt, vram, pram, oam))
        } else {
            None
        };
        for y in 0..SCREEN_HEIGHT {
            for x in 0..SCREEN_WIDTH {
                let window = window_mask_at(dispcnt, io, x, y, obj_window.as_deref());
                let mut first = Pixel {
                    color: backdrop,
                    priority: 4,
                    order: 255,
                    mask: 1 << 5,
                    semi: false,
                };
                let mut second = None;

                for bg in 0..4 {
                    if dispcnt & (1 << (8 + bg)) == 0 || window.layers & (1 << bg) == 0 {
                        continue;
                    }
                    let pixel = match (mode, bg) {
                        (0, _) | (1, 0 | 1) => text_bg_pixel(bg, x, y, io, vram, pram),
                        (1, 2) | (2, 2 | 3) => affine_bg_pixel(bg, x, y, io, vram, pram),
                        _ => None,
                    };
                    if let Some(pixel) = pixel {
                        insert_pixel(&mut first, &mut second, pixel);
                    }
                }

                if dispcnt & (1 << 12) != 0 && window.layers & (1 << 4) != 0 {
                    if let Some(pixel) = obj_layer
                        .as_ref()
                        .and_then(|layer| layer[y * SCREEN_WIDTH + x])
                    {
                        insert_pixel(&mut first, &mut second, pixel);
                    }
                }

                let color = apply_blend(first, second, io, window.blend);
                self.frame[y * SCREEN_WIDTH + x] = bgr555_to_rgb(color);
            }
        }
    }
}

fn insert_pixel(first: &mut Pixel, second: &mut Option<Pixel>, pixel: Pixel) {
    let pixel_key = (pixel.priority, pixel.order);
    let first_key = (first.priority, first.order);
    if pixel_key < first_key {
        *second = Some(*first);
        *first = pixel;
    } else if second
        .map(|old| pixel_key < (old.priority, old.order))
        .unwrap_or(true)
    {
        *second = Some(pixel);
    }
}

#[derive(Clone, Copy)]
struct WindowMask {
    layers: u8,
    blend: bool,
}

fn window_mask_at(
    dispcnt: u16,
    io: &[u8; 0x400],
    x: usize,
    y: usize,
    obj_window: Option<&[bool]>,
) -> WindowMask {
    if dispcnt & 0xe000 == 0 {
        return window_mask_from_bits(0x3f);
    }

    let winin = read16(io, 0x048);
    let winout = read16(io, 0x04a);
    let bits = if dispcnt & (1 << 13) != 0 && inside_window(io, 0x040, 0x044, x, y) {
        winin & 0x003f
    } else if dispcnt & (1 << 14) != 0 && inside_window(io, 0x042, 0x046, x, y) {
        (winin >> 8) & 0x003f
    } else if dispcnt & (1 << 15) != 0
        && obj_window
            .and_then(|mask| mask.get(y * SCREEN_WIDTH + x))
            .copied()
            .unwrap_or(false)
    {
        (winout >> 8) & 0x003f
    } else {
        winout & 0x003f
    };
    window_mask_from_bits(bits)
}

fn window_mask_from_bits(bits: u16) -> WindowMask {
    WindowMask {
        layers: (bits & 0x1f) as u8,
        blend: bits & (1 << 5) != 0,
    }
}

fn inside_window(io: &[u8; 0x400], hoff: usize, voff: usize, x: usize, y: usize) -> bool {
    let h = read16(io, hoff);
    let v = read16(io, voff);
    let left = (h >> 8) as usize;
    let right = (h & 0xff) as usize;
    let top = (v >> 8) as usize;
    let bottom = (v & 0xff) as usize;
    in_window_range(x, left, right, SCREEN_WIDTH) && in_window_range(y, top, bottom, SCREEN_HEIGHT)
}

fn in_window_range(value: usize, start: usize, end: usize, limit: usize) -> bool {
    let start = start.min(limit);
    let end = end.min(limit);
    start < end && value >= start && value < end
}

fn apply_blend(top: Pixel, second: Option<Pixel>, io: &[u8; 0x400], blend_enabled: bool) -> u16 {
    if !blend_enabled {
        return top.color;
    }

    let bldcnt = read16(io, 0x050);
    let effect = (bldcnt >> 6) & 3;
    let target1 = bldcnt & 0x3f;
    let target2 = (bldcnt >> 8) & 0x3f;

    if top.semi {
        if let Some(second) = second.filter(|p| target2 & p.mask != 0) {
            let eva = (read16(io, 0x052) & 0x1f).min(16) as u32;
            let evb = ((read16(io, 0x052) >> 8) & 0x1f).min(16) as u32;
            return alpha_blend(top.color, second.color, eva, evb);
        }
    }

    if target1 & top.mask == 0 {
        return top.color;
    }

    match effect {
        1 => {
            if let Some(second) = second.filter(|p| target2 & p.mask != 0) {
                let eva = (read16(io, 0x052) & 0x1f).min(16) as u32;
                let evb = ((read16(io, 0x052) >> 8) & 0x1f).min(16) as u32;
                alpha_blend(top.color, second.color, eva, evb)
            } else {
                top.color
            }
        }
        2 => brighten(top.color, (read16(io, 0x054) & 0x1f).min(16) as u32),
        3 => darken(top.color, (read16(io, 0x054) & 0x1f).min(16) as u32),
        _ => top.color,
    }
}

fn text_bg_pixel(
    bg: usize,
    x: usize,
    y: usize,
    io: &[u8; 0x400],
    vram: &[u8],
    pram: &[u8],
) -> Option<Pixel> {
    let cnt = read16(io, 0x008 + bg * 2);
    let char_base = (((cnt >> 2) & 3) as usize) * 0x4000;
    let screen_base = (((cnt >> 8) & 0x1f) as usize) * 0x800;
    let color_256 = cnt & (1 << 7) != 0;
    let size = (cnt >> 14) & 3;
    let (width, height) = match size {
        0 => (256, 256),
        1 => (512, 256),
        2 => (256, 512),
        _ => (512, 512),
    };
    let hofs = (read16(io, 0x010 + bg * 4) & 0x01ff) as usize;
    let vofs = (read16(io, 0x012 + bg * 4) & 0x01ff) as usize;
    let sx = (x + hofs) % width;
    let sy = (y + vofs) % height;
    let tile_x = sx / 8;
    let tile_y = sy / 8;
    let page = match (size, tile_x >= 32, tile_y >= 32) {
        (1, true, _) => 1,
        (2, _, true) => 1,
        (3, true, false) => 1,
        (3, false, true) => 2,
        (3, true, true) => 3,
        _ => 0,
    };
    let local_x = tile_x % 32;
    let local_y = tile_y % 32;
    let entry_off = screen_base + page * 0x800 + (local_y * 32 + local_x) * 2;
    let entry = read_mem16(vram, entry_off);
    let mut px = sx % 8;
    let mut py = sy % 8;
    if entry & (1 << 10) != 0 {
        px = 7 - px;
    }
    if entry & (1 << 11) != 0 {
        py = 7 - py;
    }
    let tile = (entry & 0x03ff) as usize;
    let color = if color_256 {
        let off = char_base + tile * 64 + py * 8 + px;
        vram.get(off).copied().unwrap_or(0) as usize
    } else {
        let off = char_base + tile * 32 + py * 4 + px / 2;
        let byte = vram.get(off).copied().unwrap_or(0);
        let nibble = if px & 1 == 0 { byte & 0x0f } else { byte >> 4 };
        ((entry >> 12) as usize) * 16 + nibble as usize
    };
    if color & 0x0f == 0 && !color_256 || color == 0 {
        return None;
    }
    Some(Pixel {
        color: read_mem16(pram, color * 2),
        priority: (cnt & 3) as u8,
        order: 128 + bg as u8,
        mask: 1 << bg,
        semi: false,
    })
}

fn affine_bg_pixel(
    bg: usize,
    x: usize,
    y: usize,
    io: &[u8; 0x400],
    vram: &[u8],
    pram: &[u8],
) -> Option<Pixel> {
    let cnt = read16(io, 0x008 + bg * 2);
    let char_base = (((cnt >> 2) & 3) as usize) * 0x4000;
    let screen_base = (((cnt >> 8) & 0x1f) as usize) * 0x800;
    let wrap = cnt & (1 << 13) != 0;
    let size = 128usize << ((cnt >> 14) & 3);
    let reg = if bg == 2 { 0x020 } else { 0x030 };
    let pa = read16(io, reg) as i16 as i32;
    let pb = read16(io, reg + 2) as i16 as i32;
    let pc = read16(io, reg + 4) as i16 as i32;
    let pd = read16(io, reg + 6) as i16 as i32;
    let refx = sign_extend_28(read32(io, reg + 8));
    let refy = sign_extend_28(read32(io, reg + 12));
    let sx = (refx + pa * x as i32 + pb * y as i32) >> 8;
    let sy = (refy + pc * x as i32 + pd * y as i32) >> 8;
    if !wrap && (sx < 0 || sy < 0 || sx >= size as i32 || sy >= size as i32) {
        return None;
    }
    let sx = sx.rem_euclid(size as i32) as usize;
    let sy = sy.rem_euclid(size as i32) as usize;
    let tiles_per_row = size / 8;
    let entry_off = screen_base + (sy / 8 * tiles_per_row + sx / 8);
    let tile = vram.get(entry_off).copied().unwrap_or(0) as usize;
    let color = vram
        .get(char_base + tile * 64 + (sy % 8) * 8 + (sx % 8))
        .copied()
        .unwrap_or(0) as usize;
    if color == 0 {
        return None;
    }
    Some(Pixel {
        color: read_mem16(pram, color * 2),
        priority: (cnt & 3) as u8,
        order: 128 + bg as u8,
        mask: 1 << bg,
        semi: false,
    })
}

fn build_obj_layer(dispcnt: u16, vram: &[u8], pram: &[u8], oam: &[u8]) -> Vec<Option<Pixel>> {
    let mut layer = vec![None; SCREEN_WIDTH * SCREEN_HEIGHT];
    for obj in 0..128 {
        let base = obj * 8;
        let attr0 = read_mem16(oam, base);
        let attr1 = read_mem16(oam, base + 2);
        let attr2 = read_mem16(oam, base + 4);
        let obj_mode = (attr0 >> 10) & 3;
        if obj_mode >= 2 {
            continue;
        }
        let Some((x0, x1, y0, y1)) = obj_bounds(attr0, attr1) else {
            continue;
        };
        for py in y0..y1 {
            for px in x0..x1 {
                let Some(color) =
                    obj_color_index_at(obj, px, py, dispcnt, attr0, attr1, attr2, vram, oam)
                else {
                    continue;
                };
                let palette_index = if attr0 & (1 << 13) != 0 {
                    0x100 + color as usize
                } else {
                    0x100 + ((attr2 >> 12) as usize) * 16 + color as usize
                };
                insert_obj_layer_pixel(
                    &mut layer[py * SCREEN_WIDTH + px],
                    Pixel {
                        color: read_mem16(pram, palette_index * 2),
                        priority: ((attr2 >> 10) & 3) as u8,
                        order: obj as u8,
                        mask: 1 << 4,
                        semi: obj_mode == 1,
                    },
                );
            }
        }
    }
    layer
}

fn insert_obj_layer_pixel(slot: &mut Option<Pixel>, pixel: Pixel) {
    if slot
        .map(|old| (pixel.priority, pixel.order) < (old.priority, old.order))
        .unwrap_or(true)
    {
        *slot = Some(pixel);
    }
}

fn build_obj_window_mask(dispcnt: u16, vram: &[u8], oam: &[u8]) -> Vec<bool> {
    let mut mask = vec![false; SCREEN_WIDTH * SCREEN_HEIGHT];
    for obj in 0..128 {
        let base = obj * 8;
        let attr0 = read_mem16(oam, base);
        if (attr0 >> 10) & 3 != 2 {
            continue;
        }
        let attr1 = read_mem16(oam, base + 2);
        let attr2 = read_mem16(oam, base + 4);
        let Some((x0, x1, y0, y1)) = obj_bounds(attr0, attr1) else {
            continue;
        };
        for py in y0..y1 {
            for px in x0..x1 {
                if obj_color_index_at(obj, px, py, dispcnt, attr0, attr1, attr2, vram, oam)
                    .is_some()
                {
                    mask[py * SCREEN_WIDTH + px] = true;
                }
            }
        }
    }
    mask
}

fn obj_color_index_at(
    obj: usize,
    x: usize,
    y: usize,
    dispcnt: u16,
    attr0: u16,
    attr1: u16,
    attr2: u16,
    vram: &[u8],
    oam: &[u8],
) -> Option<u8> {
    if attr0 & (1 << 9) != 0 && attr0 & (1 << 8) == 0 {
        return None;
    }
    let (w, h) = obj_size(attr0 >> 14, attr1 >> 14);
    let affine = attr0 & (1 << 8) != 0;
    let draw_w = if affine && attr0 & (1 << 9) != 0 {
        w * 2
    } else {
        w
    };
    let draw_h = if affine && attr0 & (1 << 9) != 0 {
        h * 2
    } else {
        h
    };
    let ox = signed_obj_x(attr1 & 0x01ff);
    let oy = signed_obj_y(attr0 & 0x00ff);
    let px = x as i32 - ox;
    let py = y as i32 - oy;
    if px < 0 || py < 0 || px >= draw_w as i32 || py >= draw_h as i32 {
        return None;
    }

    let (mut tx, mut ty) = if affine {
        affine_obj_coords(
            obj,
            attr1,
            px,
            py,
            w as i32,
            h as i32,
            draw_w as i32,
            draw_h as i32,
            oam,
        )
    } else {
        (px, py)
    };
    if tx < 0 || ty < 0 || tx >= w as i32 || ty >= h as i32 {
        return None;
    }
    if !affine {
        if attr1 & (1 << 12) != 0 {
            tx = w as i32 - 1 - tx;
        }
        if attr1 & (1 << 13) != 0 {
            ty = h as i32 - 1 - ty;
        }
    }

    let color = obj_tile_color(
        (attr2 & 0x03ff) as usize,
        tx as usize,
        ty as usize,
        w,
        attr0 & (1 << 13) != 0,
        dispcnt & (1 << 6) != 0,
        vram,
    );
    (color != 0).then_some(color)
}

fn obj_bounds(attr0: u16, attr1: u16) -> Option<(usize, usize, usize, usize)> {
    if attr0 & (1 << 9) != 0 && attr0 & (1 << 8) == 0 {
        return None;
    }
    let (w, h) = obj_size(attr0 >> 14, attr1 >> 14);
    let affine = attr0 & (1 << 8) != 0;
    let draw_w = if affine && attr0 & (1 << 9) != 0 {
        w * 2
    } else {
        w
    } as i32;
    let draw_h = if affine && attr0 & (1 << 9) != 0 {
        h * 2
    } else {
        h
    } as i32;
    let ox = signed_obj_x(attr1 & 0x01ff);
    let oy = signed_obj_y(attr0 & 0x00ff);
    let x0 = ox.max(0) as usize;
    let y0 = oy.max(0) as usize;
    let x1 = (ox + draw_w).min(SCREEN_WIDTH as i32).max(0) as usize;
    let y1 = (oy + draw_h).min(SCREEN_HEIGHT as i32).max(0) as usize;
    (x0 < x1 && y0 < y1).then_some((x0, x1, y0, y1))
}

fn affine_obj_coords(
    obj: usize,
    attr1: u16,
    px: i32,
    py: i32,
    w: i32,
    h: i32,
    draw_w: i32,
    draw_h: i32,
    oam: &[u8],
) -> (i32, i32) {
    let index = ((attr1 >> 9) & 0x1f) as usize;
    let pa = read_mem16(oam, index * 32 + 6) as i16 as i32;
    let pb = read_mem16(oam, index * 32 + 14) as i16 as i32;
    let pc = read_mem16(oam, index * 32 + 22) as i16 as i32;
    let pd = read_mem16(oam, index * 32 + 30) as i16 as i32;
    let _ = obj;
    let cx = draw_w / 2;
    let cy = draw_h / 2;
    let ux = px - cx;
    let uy = py - cy;
    (
        ((pa * ux + pb * uy) >> 8) + w / 2,
        ((pc * ux + pd * uy) >> 8) + h / 2,
    )
}

fn obj_tile_color(
    tile: usize,
    x: usize,
    y: usize,
    width: usize,
    color_256: bool,
    one_dimensional: bool,
    vram: &[u8],
) -> u8 {
    let tiles_x = width / 8;
    let tile_x = x / 8;
    let tile_y = y / 8;
    let px = x % 8;
    let py = y % 8;
    let base = 0x10000;
    if color_256 {
        let stride = if one_dimensional { tiles_x * 2 } else { 32 };
        let tile_no = tile + tile_y * stride + tile_x * 2;
        vram.get(base + tile_no * 32 + py * 8 + px)
            .copied()
            .unwrap_or(0)
    } else {
        let stride = if one_dimensional { tiles_x } else { 32 };
        let tile_no = tile + tile_y * stride + tile_x;
        let byte = vram
            .get(base + tile_no * 32 + py * 4 + px / 2)
            .copied()
            .unwrap_or(0);
        if px & 1 == 0 {
            byte & 0x0f
        } else {
            byte >> 4
        }
    }
}

fn obj_size(shape: u16, size: u16) -> (usize, usize) {
    match (shape & 3, size & 3) {
        (0, 0) => (8, 8),
        (0, 1) => (16, 16),
        (0, 2) => (32, 32),
        (0, 3) => (64, 64),
        (1, 0) => (16, 8),
        (1, 1) => (32, 8),
        (1, 2) => (32, 16),
        (1, 3) => (64, 32),
        (2, 0) => (8, 16),
        (2, 1) => (8, 32),
        (2, 2) => (16, 32),
        (2, 3) => (32, 64),
        _ => (8, 8),
    }
}

fn signed_obj_x(raw: u16) -> i32 {
    let value = raw as i32;
    if value >= 256 {
        value - 512
    } else {
        value
    }
}

fn signed_obj_y(raw: u16) -> i32 {
    let value = raw as i32;
    if value >= 160 {
        value - 256
    } else {
        value
    }
}

fn alpha_blend(a: u16, b: u16, eva: u32, evb: u32) -> u16 {
    let ar = (a & 0x1f) as u32;
    let ag = ((a >> 5) & 0x1f) as u32;
    let ab = ((a >> 10) & 0x1f) as u32;
    let br = (b & 0x1f) as u32;
    let bg = ((b >> 5) & 0x1f) as u32;
    let bb = ((b >> 10) & 0x1f) as u32;
    let r = ((ar * eva + br * evb) >> 4).min(31);
    let g = ((ag * eva + bg * evb) >> 4).min(31);
    let bl = ((ab * eva + bb * evb) >> 4).min(31);
    (r | (g << 5) | (bl << 10)) as u16
}

fn brighten(c: u16, evy: u32) -> u16 {
    let r = (c & 0x1f) as u32;
    let g = ((c >> 5) & 0x1f) as u32;
    let b = ((c >> 10) & 0x1f) as u32;
    let r = r + ((31 - r) * evy >> 4);
    let g = g + ((31 - g) * evy >> 4);
    let b = b + ((31 - b) * evy >> 4);
    (r | (g << 5) | (b << 10)) as u16
}

fn darken(c: u16, evy: u32) -> u16 {
    let r = (c & 0x1f) as u32;
    let g = ((c >> 5) & 0x1f) as u32;
    let b = ((c >> 10) & 0x1f) as u32;
    let r = r - (r * evy >> 4);
    let g = g - (g * evy >> 4);
    let b = b - (b * evy >> 4);
    (r | (g << 5) | (b << 10)) as u16
}

fn bgr555_to_rgb(color: u16) -> u32 {
    let r = (color & 0x1f) as u32;
    let g = ((color >> 5) & 0x1f) as u32;
    let b = ((color >> 10) & 0x1f) as u32;
    let r = (r << 3) | (r >> 2);
    let g = (g << 3) | (g >> 2);
    let b = (b << 3) | (b >> 2);
    (r << 16) | (g << 8) | b
}

fn sign_extend_28(value: u32) -> i32 {
    ((value << 4) as i32) >> 4
}

fn set_dispstat_bit(io: &mut [u8; 0x400], bit: u16, on: bool) {
    let mut value = read16(io, 0x004);
    if on {
        value |= 1 << bit;
    } else {
        value &= !(1 << bit);
    }
    write16(io, 0x004, value);
}

fn request_irq(io: &mut [u8; 0x400], bit: u16) {
    let flags = read16(io, 0x202) | bit;
    write16(io, 0x202, flags);
}

fn read16(io: &[u8; 0x400], off: usize) -> u16 {
    u16::from_le_bytes([io[off], io[off + 1]])
}

fn read32(io: &[u8; 0x400], off: usize) -> u32 {
    u32::from_le_bytes([io[off], io[off + 1], io[off + 2], io[off + 3]])
}

fn write16(io: &mut [u8; 0x400], off: usize, value: u16) {
    let bytes = value.to_le_bytes();
    io[off] = bytes[0];
    io[off + 1] = bytes[1];
}

fn read_mem16(mem: &[u8], off: usize) -> u16 {
    if off + 1 >= mem.len() {
        return 0;
    }
    u16::from_le_bytes([mem[off], mem[off + 1]])
}
