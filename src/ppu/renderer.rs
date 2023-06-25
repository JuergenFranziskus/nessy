use super::Scroll;

pub struct Renderer<'a> {
    framebuffer: &'a mut [[u8; 3]; 256 * 240],
    background_transparent: Box<[bool; 256 * 240]>,
    sprite_priority: Box<[u8; 256 * 240]>,
    pattern_table: &'a [u8; 8192],
    nametable: &'a [u8; 4096],

    oam: &'a [u8; 256],
    sprites_use_right_field: bool,
    wide_sprites: bool,

    background_uses_right_field: bool,
    background_color: u8,
    palettes: [[u8; 3]; 8],

    sprite_0_hit: bool,
    sprite_overflow: bool,

    scroll: [u16; 2],
}
impl<'a> Renderer<'a> {
    pub(super) fn new(
        framebuffer: &'a mut [[u8; 3]; 256 * 240],
        pattern_table: &'a [u8; 8192],
        nametable: &'a [u8; 4096],
        oam: &'a [u8; 256],
        sprites_use_right_field: bool,
        wide_sprites: bool,
        background_uses_right_field: bool,
        background_color: u8,
        palettes: [[u8; 3]; 8],

        scroll: Scroll,
    ) -> Self {
        let scroll_x = scroll.x.0 as u16 | (scroll.x.1 as u16) << 8;
        let scroll_y = scroll.y.0 as u16 | (scroll.y.1 as u16) << 8;

        Self {
            framebuffer,
            background_transparent: Box::new([true; 256 * 240]),
            sprite_priority: Box::new([0; 256 * 240]),
            pattern_table,
            nametable,

            background_uses_right_field,
            background_color,
            palettes,

            sprite_0_hit: false,
            sprite_overflow: false,

            scroll: [scroll_x, scroll_y],
            oam,
            sprites_use_right_field,
            wide_sprites,
        }
    }

    pub fn render(mut self) -> (bool, bool) {
        self.render_background();
        self.render_sprites();
        (self.sprite_0_hit, self.sprite_overflow)
    }

    fn render_background(&mut self) {
        for y in 0..240 {
            for x in 0..256 {
                let i = (x + 256 * y) as usize;
                let (color, transpi) = self.background_color(x, y);
                let pixel = color_to_rgb(color);
                self.framebuffer[i] = pixel;
                self.background_transparent[i] = transpi;
            }
        }
    }
    fn background_color(&self, x: u16, y: u16) -> (u8, bool) {
        let x = (x + self.scroll[0]) % 512;
        let y = (y + self.scroll[1]) % 480;
        let name_i = nametable_index(x, y);
        let name_byte = self.nametable[name_i];
        let fine_y = y as usize % 8;
        let fine_x = 7 - (x as usize % 8);

        let pixel =
            self.pattern(fine_x, fine_y, name_byte, self.background_uses_right_field) as usize;
        let attribute_i = attribute_index(x, y);
        let attribute = self.nametable[attribute_i];
        let palette = palette(x, y, attribute) as usize;
        if pixel == 0 {
            (self.background_color, true)
        } else {
            (self.palettes[palette][pixel - 1], false)
        }
    }

    fn render_sprites(&mut self) {
        for (i, sprite) in self.oam.chunks_exact(4).enumerate() {
            let y = sprite[0].wrapping_add(1) as usize;
            let tile = sprite[1];
            let attribute = sprite[2];
            let x = sprite[3] as usize;

            let palette = (attribute as usize & 0b11) + 4;
            let priority = attribute & 32 == 0;
            let hflip = attribute & 64 == 0;
            let vflip = attribute & 128 == 0;

            if self.wide_sprites {
                self.render_wide_sprite(x, y, tile, palette, priority, hflip, vflip);
            } else {
                self.render_normal_sprite(i as u8, x, y, tile, palette, priority, hflip, vflip);
            }
        }
    }
    fn render_normal_sprite(
        &mut self,
        sprite_id: u8,
        x: usize,
        y: usize,
        tile: u8,
        palette: usize,
        priority: bool,
        hflip: bool,
        vflip: bool,
    ) {
        for fine_y in 0..8 {
            for fine_x in 0..8 {
                let screen_x = x + fine_x;
                let screen_y = y + fine_y;
                if screen_x >= 256 || screen_y >= 240 {
                    continue;
                }

                let tile_x = if hflip { 7 - fine_x } else { fine_x };
                let tile_y = if vflip { fine_y } else { 7 - fine_y };

                let screen_i = screen_x + screen_y * 256;
                let pattern =
                    self.pattern(tile_x, tile_y, tile, self.sprites_use_right_field) as usize;
                if pattern == 0 {
                    continue;
                }
                let transpi = self.background_transparent[screen_i];
                self.sprite_0_hit |= sprite_id == 0 && !transpi;
                let color = self.palettes[palette][pattern - 1];

                let req_priority = self.sprite_priority[screen_i];
                let show = (transpi || priority) && sprite_id >= req_priority;
                if show {
                    let rgb = color_to_rgb(color);
                    self.framebuffer[screen_i] = rgb;
                    self.sprite_priority[screen_i] = sprite_id;
                }
            }
        }
    }
    fn render_wide_sprite(
        &mut self,
        _x: usize,
        _y: usize,
        _tile: u8,
        _palette: usize,
        _priority: bool,
        _hflip: bool,
        _vflip: bool,
    ) {
        todo!()
    }

    fn pattern(&self, fine_x: usize, fine_y: usize, name_byte: u8, right_field: bool) -> u8 {
        let pattern_index = pattern_index(name_byte, right_field);
        let mask = 1 << (fine_x as u8);
        let pattern_low = self.pattern_table[pattern_index + fine_y];
        let pattern_high = self.pattern_table[pattern_index + fine_y + 8];
        let low_bit = if pattern_low & mask != 0 { 1 } else { 0 };
        let high_bit = if pattern_high & mask != 0 { 2 } else { 0 };
        let pixel = low_bit | high_bit;
        pixel
    }
}

fn base_nametable(x: u16, y: u16) -> usize {
    let base = if x < 256 && y < 240 {
        0
    } else if y < 240 {
        1024
    } else if x < 256 {
        2 * 1024
    } else {
        3 * 1024
    };

    base
}
fn nametable_index(x: u16, y: u16) -> usize {
    let base = base_nametable(x, y);

    let x = (x % 256) as usize / 8;
    let y = (y % 240) as usize / 8;
    let offset = x + 32 * y;

    base + offset
}
fn attribute_index(x: u16, y: u16) -> usize {
    let base = base_nametable(x, y) + 960;

    let x = (x % 256) as usize / 32;
    let y = (y % 240) as usize / 32;
    let offset = x + 8 * y;

    base + offset
}
fn pattern_index(name: u8, right_field: bool) -> usize {
    let addend = if right_field { 0x1000 } else { 0 };
    let offset = (name as usize) * 16;

    addend + offset
}
fn palette(x: u16, y: u16, attribute: u8) -> u8 {
    let x = x % 32 / 16;
    let y = y % 32 / 16;

    match (x, y) {
        (0, 0) => attribute & 0b11,
        (1, 0) => (attribute >> 2) & 0b11,
        (0, 1) => (attribute >> 4) & 0b11,
        (1, 1) => (attribute >> 6) & 0b11,
        _ => unreachable!(),
    }
}
pub fn color_to_rgb(color: u8) -> [u8; 3] {
    let first_byte = color as usize * 3;
    [
        PALETTE_BYTES[first_byte + 0],
        PALETTE_BYTES[first_byte + 1],
        PALETTE_BYTES[first_byte + 2],
    ]
}

static PALETTE_BYTES: &[u8] = include_bytes!("ntscpalette.pal");
