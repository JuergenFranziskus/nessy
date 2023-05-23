use super::Scroll;

pub struct Renderer<'a> {
    framebuffer: &'a mut [[u8; 3]; 256 * 240],
    pattern_table: &'a [u8; 8192],
    nametable: &'a [u8; 4096],

    background_uses_right_field: bool,
    background_color: u8,
    palettes: [[u8; 3]; 8],

    scroll: [u16; 2],
}
impl<'a> Renderer<'a> {
    pub(super) fn new(
        framebuffer: &'a mut [[u8; 3]; 256 * 240],
        pattern_table: &'a [u8; 8192],
        nametable: &'a [u8; 4096],
        background_uses_right_field: bool,
        background_color: u8,
        palettes: [[u8; 3]; 8],

        scroll: Scroll,
    ) -> Self {
        let scroll_x = scroll.x.0 as u16 | (scroll.x.1 as u16) << 8;
        let scroll_y = scroll.y.0 as u16 | (scroll.y.1 as u16) << 8;

        Self {
            framebuffer,
            pattern_table,
            nametable,

            background_uses_right_field,
            background_color,
            palettes,

            scroll: [scroll_x, scroll_y],
        }
    }

    pub fn render(self) {
        for x in 0..256 {
            for y in 0..240 {
                let i = (x + 256 * y) as usize;
                let color = self.pixel_color(x, y);
                let pixel = color_to_rgb(color);
                //eprintln!("{color} = ({red} {green} {blue})");
                self.framebuffer[i] = pixel;
            }
        }
        //eprintln!("Finished rendering");
    }
    fn pixel_color(&self, x: u16, y: u16) -> u8 {
        let x = (x + self.scroll[0]) % 512;
        let y = (y + self.scroll[1]) % 480;
        let name_i = nametable_index(x, y);
        let name_byte = self.nametable[name_i];
        let pattern_index = pattern_index(name_byte, self.background_uses_right_field);
        let fine_y = y as usize % 8;
        let fine_x = 7 - (x as usize % 8);
        let mask = 1 << (fine_x as u8);
        let pattern_low = self.pattern_table[pattern_index + fine_y];
        let pattern_high = self.pattern_table[pattern_index + fine_y + 8];
        let low_bit = if pattern_low & mask != 0 { 1 } else { 0 };
        let high_bit = if pattern_high & mask != 0 { 2 } else { 0 };
        let pixel = low_bit | high_bit;
        let attribute_i = attribute_index(x, y);
        let attribute = self.nametable[attribute_i];
        let palette = palette(x, y, attribute) as usize;
        let color = if pixel == 0 {
            self.background_color
        } else {
            self.palettes[palette][pixel - 1]
        };

        color % 64
        // name_byte % 64
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
    let base = base_nametable(x, y) + 0xC30;

    let x = (x % 256) as usize / 64;
    let y = (y % 240) as usize / 64;
    let offset = x + 64 * y;

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
