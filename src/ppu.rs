use crate::{
    nesbus::CpuBus,
    util::{get_flag_u16, get_flag_u8, set_flag_u16, set_flag_u8},
};

use self::pixel_buffer::PixelBuffer;

const DOTS: u16 = 341;
const LINES: u16 = 262;

pub mod pixel_buffer;

pub struct Ppu {
    meta: Meta,
    control: Control,
    mask: Mask,
    v: V,
    t: V,
    dot: [u16; 2],

    data_latch: u8,
    oam_addr: u8,
    oam: Box<[u8; 256]>,
    palette: Box<[u8; 32]>,

    shifters: Shifters,
    sprites: Box<Sprites>,

    pixels: Box<PixelBuffer>,
}
impl Ppu {
    pub fn init() -> Self {
        Self {
            meta: Meta::init(),
            control: Control::init(),
            mask: Mask::init(),
            v: V::init(),
            t: V::init(),
            dot: [0; 2],

            data_latch: 0,
            oam_addr: 0,
            oam: Box::new([0; 256]),
            palette: Box::new([0; 32]),

            shifters: Shifters::init(),
            sprites: Box::new(Sprites::init()),

            pixels: Box::new(PixelBuffer::new()),
        }
    }

    pub fn cycle(&mut self, bus: &mut PpuBus, cpu: &mut CpuBus) {
        self.common_cycle(cpu, bus);
        self.handle_cpu(bus, cpu);
    }
    pub fn cycle_alone(&mut self, bus: &mut PpuBus, cpu: &mut CpuBus) {
        self.common_cycle(cpu, bus);
    }

    fn common_cycle(&mut self, cpu: &mut CpuBus, bus: &mut PpuBus) {
        self.update_data_latch(bus); // The order is important here
        self.perform_memop(bus);

        self.render(bus);

        self.decide_vblank(cpu);
        self.tick_counter();
    }
    fn update_data_latch(&mut self, bus: &mut PpuBus) {
        if !self.meta.data_latch_update_pending() {
            return;
        };
        if self.meta.read_pending() {
            return;
        };

        self.data_latch = bus.data();
        self.meta.set_data_latch_update_pending(false);
    }
    fn perform_memop(&mut self, bus: &mut PpuBus) {
        bus.set_read_enable(self.meta.read_pending());
        bus.set_write_enable(self.meta.write_pending());

        self.meta.set_read_pending(false);
        self.meta.set_write_pending(false);
    }
    fn decide_vblank(&mut self, cpu: &mut CpuBus) {
        let start = [1, 241];
        let end = [1, 261];

        if self.dot == start {
            self.meta.set_vblank(true);
        } else if self.dot == end {
            self.meta.set_vblank(false);
            self.meta.set_sprite_zero_hit(false);
            self.meta.set_sprite_overflow(false);
        }

        cpu.set_nmi(self.meta.vblank() && self.control.nmi_enable());
    }
    fn tick_counter(&mut self) {
        let last = if self.meta.odd_frame() {
            [DOTS - 2, LINES - 1]
        } else {
            [DOTS - 1, LINES - 1]
        };
        if self.dot == last {
            self.dot = [0, 0];
            self.meta.set_odd_frame(!self.meta.odd_frame());
        } else {
            self.dot[0] += 1;
            if self.dot[0] == DOTS {
                self.dot[0] = 0;
                self.dot[1] += 1;
            }
        }
    }

    fn render(&mut self, bus: &mut PpuBus) {
        if !self.mask.render_enabled() {
            return;
        };

        match self.dot[1] {
            0..=239 => self.visible_scanline(false, bus),
            261 => self.visible_scanline(true, bus),
            _ => (),
        }
    }
    fn visible_scanline(&mut self, prerender: bool, bus: &mut PpuBus) {
        match self.dot[0] {
            0 => (),
            1..=256 => {
                let x = self.dot[0] - 1;
                let step = (x % 8) as u8;

                if x != 0 {
                    self.shifters.shift();
                }
                if step == 0 && x != 0 {
                    self.shifters.shift_in_tile(bus.data());
                    self.v.increment_x();
                }

                self.fetch_background(step, bus);
                if !prerender {
                    self.produce_pixel();
                }

                if x == 255 {
                    self.v.increment_y();
                }
            }
            257..=320 => {
                if self.dot[0] == 257 {
                    self.v.copy_horizontal_bits(self.t);
                    self.evaluate_sprites();
                }
                if (280..=304).contains(&self.dot[0]) && prerender {
                    self.v.copy_vertical_bits(self.t)
                }

                self.fetch_sprites(bus);
            }
            321..=336 => {
                if self.dot[0] == 321 {
                    self.fetch_sprites(bus); // Final sprite pattern data is only now available
                }
                self.prefetch_tiles(bus);
            }
            337 => self.prefetch_tiles(bus), // Final pattern data is only now available
            _ => (),
        }
    }

    fn evaluate_sprites(&mut self) {
        self.sprites.eval_index = 0;
        self.sprites.fetch_index = 0;
        for i in (0..256).step_by(4) {
            self.evaluate_sprite(i);
        }
        while self.sprites.eval_index < 8 {
            self.sprites.sprites[self.sprites.eval_index as usize] = Sprite::default();
            self.sprites.eval_index += 1;
        }
    }
    fn evaluate_sprite(&mut self, sprite: usize) {
        if self.sprites.eval_index >= 8 {
            self.meta.set_sprite_overflow(true); // Wrongly correct implementation, real hardware has bug. Important?
            return;
        }

        let bytes = &self.oam[sprite..sprite + 4];
        let dot = self.dot();
        let y = bytes[0] as u16;
        let ver_range = y..(y + 8);
        if !ver_range.contains(&dot[1]) {
            return;
        };
        let x = bytes[3];
        let tile = bytes[1];
        let flags = bytes[2];

        let palette = flags & 0b11;
        let priority = flags & (1 << 5) == 0;
        let hor_flip = flags & (1 << 6) != 0;
        let ver_flip = flags & (1 << 7) != 0;

        let y_offset = (dot[1] - y) as u8;
        let y_offset = if ver_flip { 7 - y_offset } else { y_offset };

        self.sprites.sprites[self.sprites.eval_index as usize] = Sprite {
            present: true,
            x,
            sprite_zero: sprite == 0,
            priority,
            tile,
            y_offset,
            hor_flip,
            pattern: [0; 2],
            palette,
        };
        self.sprites.eval_index += 1;
    }
    fn fetch_sprites(&mut self, bus: &mut PpuBus) {
        let step = (self.dot[0] - 257) as u8 % 8;

        match step {
            0 => {
                if self.dot[0] != 257 {
                    self.sprites.fetch_high_pattern(bus.data());
                    self.sprites.next_fetch();
                }
                if self.dot[0] != 321 {
                    self.read(self.v.tile_address(), bus)
                }
            }
            1 => (),
            2 => self.read(self.v.attribute_address(), bus),
            3 => (),
            4 => self.read(
                self.sprites
                    .pattern_low_address(self.control.sprite_table()),
                bus,
            ),
            5 => (),
            6 => {
                self.sprites.fetch_low_pattern(bus.data());
                self.read(
                    self.sprites
                        .pattern_high_address(self.control.sprite_table()),
                    bus,
                );
            }
            7 => (),
            _ => (),
        }
    }

    fn prefetch_tiles(&mut self, bus: &mut PpuBus) {
        let step = (self.dot[0] - 321) as u8 % 8;

        self.shifters.shift();
        if step == 0 && self.dot[0] != 321 {
            self.shifters.shift_in_tile(bus.data());
            self.v.increment_x();
        }

        if self.dot[0] == 337 {
            return;
        };
        self.fetch_background(step, bus);
    }
    fn fetch_background(&mut self, step: u8, bus: &mut PpuBus) {
        match step {
            0 => self.read(self.v.tile_address(), bus),
            1 => (),
            2 => {
                self.shifters.next_name = bus.data();
                self.read(self.v.attribute_address(), bus);
            }
            3 => (),
            4 => {
                let attribute = self.v.extract_attribute(bus.data());
                self.shifters.next_attribute = attribute;
                let addr = self
                    .shifters
                    .pattern_address(self.control.background_table(), self.v.fine_y());
                self.read(addr, bus);
            }
            5 => (),
            6 => {
                self.shifters.next_pattern_low = bus.data();
                let addr = self
                    .shifters
                    .pattern_address(self.control.background_table(), self.v.fine_y());
                self.read(addr + 8, bus);
            }
            7 => (),
            8.. => unreachable!(),
        }
    }
    fn produce_pixel(&mut self) {
        let x = self.dot()[0] as usize - 1;
        let y = self.dot()[1] as usize;

        let bg_pattern = self.shifters.pattern(self.meta.x());
        let bg_palette = self.shifters.palette(self.meta.x());
        let bg_opague =
            bg_pattern != 0 && self.mask.background() && (x >= 8 || self.mask.left_background());
        let bg_color = self.background_color(bg_palette, bg_pattern);

        let (sp_pattern, sp_palette, sp_zero, sp_priority) = self.generate_sprite_pixel();
        let sp_opague =
            sp_pattern != 0 && self.mask.sprites() && (x >= 8 || self.mask.left_sprites());
        let sp_color = self.sprite_color(sp_palette, sp_pattern);

        let universal_bg = self.palette[0];
        let (color, hit) = match (bg_opague, sp_opague) {
            (false, false) => (universal_bg, false),
            (true, false) => (bg_color, false),
            (false, true) => (sp_color, false),
            (true, true) => {
                let color = if sp_priority { sp_color } else { bg_color };
                (color, true)
            }
        };

        if hit && sp_zero {
            self.meta.set_sprite_zero_hit(true);
        }

        self.pixels.set_color(x, y, color);
    }
    fn generate_sprite_pixel(&self) -> (u8, u8, bool, bool) {
        for sprite in &self.sprites.sprites {
            if !sprite.present {
                continue;
            };
            let x = self.dot[0] - 1;
            let sp_x = sprite.x as u16;
            let hor_range = sp_x..sp_x + 8;
            if !hor_range.contains(&x) {
                continue;
            };
            let offset = (x - sp_x) as u8;
            let offset = if !sprite.hor_flip { 7 - offset } else { offset };
            let pattern_low = if sprite.pattern[0] & (1 << offset) != 0 {
                1
            } else {
                0
            };
            let pattern_high = if sprite.pattern[1] & (1 << offset) != 0 {
                2
            } else {
                0
            };
            let pattern = pattern_low | pattern_high;
            if pattern == 0 {
                continue;
            };
            let palette = sprite.palette;
            let zero = sprite.sprite_zero;
            let priority = sprite.priority;
            return (pattern, palette, zero, priority);
        }

        (0, 0, false, false)
    }
    fn background_color(&self, palette: u8, pattern: u8) -> u8 {
        let index = (palette << 2) | pattern;
        self.palette[index as usize]
    }
    fn sprite_color(&self, palette: u8, pattern: u8) -> u8 {
        let index = 16 | (palette << 2) | pattern;
        self.palette[index as usize]
    }

    fn handle_cpu(&mut self, bus: &mut PpuBus, cpu: &mut CpuBus) {
        if !(0x2000..0x4000).contains(&cpu.address()) {
            return;
        };
        let addr = cpu.address() % 8;
        let data = cpu.data();

        match addr {
            0 => {
                if cpu.read() {
                    return;
                };
                let nametable = data & 0b11;
                self.t.set_nametable(nametable);
                self.control.0 = data;
            }
            1 => {
                if cpu.read() {
                    return;
                };
                self.mask.0 = data;
            }
            2 => {
                if !cpu.read() {
                    return;
                };
                cpu.set_data(self.meta.status_bits());
                self.meta.set_w(false);
                self.meta.set_vblank(false);
            }
            3 => {
                if cpu.read() {
                    return;
                };
                self.oam_addr = data;
            }
            4 => {
                if cpu.read() {
                    cpu.set_data(self.oam[self.oam_addr as usize]);
                } else {
                    self.oam[self.oam_addr as usize] = data;
                    self.oam_addr = self.oam_addr.wrapping_add(1);
                }
            }
            5 => {
                if cpu.read() {
                    return;
                };
                if !self.meta.w() {
                    self.meta.set_x(data & 0b111);
                    self.t.set_coarse_x(data >> 3);
                    self.meta.set_w(true);
                } else {
                    self.t.set_fine_y(data & 0b111);
                    self.t.set_coarse_y(data >> 3);
                    self.meta.set_w(false);
                }
            }
            6 => {
                if cpu.read() {
                    return;
                };

                if !self.meta.w() {
                    let data = data & 0b111111;
                    self.t.0 &= 0xFF;
                    self.t.0 |= (data as u16) << 8;
                    self.meta.set_w(true);
                } else {
                    self.t.0 &= !0xFF;
                    self.t.0 |= data as u16;
                    self.v = self.t;
                    self.meta.set_w(false);
                }
            }
            7 => {
                let v = self.v.0;
                let palette = is_palette_address(v);
                let palette_index = normalize_palette_address(v);

                if cpu.read() {
                    self.read(v, bus);
                    self.meta.set_data_latch_update_pending(true);
                    if palette {
                        cpu.set_data(self.palette[palette_index]);
                    } else {
                        cpu.set_data(self.data_latch);
                    }
                } else {
                    if palette {
                        self.palette[palette_index] = cpu.data();
                    } else {
                        self.write(v, cpu.data(), bus);
                    }
                }
                self.increment_v();
            }
            8.. => unreachable!(),
        }
    }

    fn read(&mut self, addr: u16, bus: &mut PpuBus) {
        self.meta.set_read_pending(true);
        bus.set_address(addr);
    }
    fn write(&mut self, addr: u16, val: u8, bus: &mut PpuBus) {
        self.meta.set_write_pending(true);
        bus.set_address(addr);
        bus.set_data(val);
    }
    fn increment_v(&mut self) {
        self.v.0 += self.control.inc_amount();
        self.v.0 %= 0x4000;
    }

    pub fn dot(&self) -> [u16; 2] {
        self.dot
    }
    pub fn is_vblank(&self) -> bool {
        self.meta.vblank()
    }
    pub fn palette(&self) -> &[u8] {
        &*self.palette
    }
    pub fn pixels(&self) -> &PixelBuffer {
        &self.pixels
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PpuBus {
    address: u16,
    data: u8,
    flags: u8,
}
impl PpuBus {
    pub fn init() -> Self {
        Self {
            address: 0,
            data: 0,
            flags: 0,
        }
    }

    fn get_flag(self, flag: u8) -> bool {
        get_flag_u8(self.flags, flag)
    }
    fn set_flag(&mut self, flag: u8, val: bool) {
        set_flag_u8(&mut self.flags, flag, val);
    }

    pub fn address(self) -> u16 {
        self.address
    }
    pub fn set_address(&mut self, addr: u16) {
        self.address = addr;
    }
    pub fn data(self) -> u8 {
        self.data
    }
    pub fn set_data(&mut self, data: u8) {
        self.data = data;
    }

    pub fn read_enable(self) -> bool {
        self.get_flag(Self::READ_ENABLE)
    }
    pub fn set_read_enable(&mut self, enable: bool) {
        self.set_flag(Self::READ_ENABLE, enable)
    }
    pub fn write_enable(self) -> bool {
        self.get_flag(Self::WRITE_ENABLE)
    }
    pub fn set_write_enable(&mut self, enable: bool) {
        self.set_flag(Self::WRITE_ENABLE, enable)
    }
    pub fn nmi(self) -> bool {
        self.get_flag(Self::NMI)
    }
    pub fn set_nmi(&mut self, nmi: bool) {
        self.set_flag(Self::NMI, nmi)
    }

    const READ_ENABLE: u8 = 0;
    const WRITE_ENABLE: u8 = 1;
    const NMI: u8 = 2;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
struct Meta(u16);
impl Meta {
    fn init() -> Self {
        Self(0)
    }

    fn get_flag(self, flag: u16) -> bool {
        get_flag_u16(self.0, flag)
    }
    fn set_flag(&mut self, flag: u16, value: bool) {
        set_flag_u16(&mut self.0, flag, value);
    }

    pub fn x(self) -> u8 {
        (self.0 >> Self::X) as u8 & 0b111
    }
    pub fn set_x(&mut self, x: u8) {
        let x = x as u16 & 0b111;
        let mask = 0b111 << Self::X;
        self.0 &= !mask;
        self.0 |= x << Self::X;
    }
    pub fn w(self) -> bool {
        self.get_flag(Self::W)
    }
    pub fn set_w(&mut self, w: bool) {
        self.set_flag(Self::W, w);
    }
    pub fn odd_frame(self) -> bool {
        self.get_flag(Self::ODD_FRAME)
    }
    pub fn set_odd_frame(&mut self, odd_frame: bool) {
        self.set_flag(Self::ODD_FRAME, odd_frame);
    }

    pub fn vblank(self) -> bool {
        self.get_flag(Self::VBLANK)
    }

    pub fn set_sprite_zero_hit(&mut self, hit: bool) {
        self.set_flag(Self::SPRITE_ZERO_HIT, hit);
    }

    pub fn set_vblank(&mut self, blank: bool) {
        self.set_flag(Self::VBLANK, blank)
    }

    pub fn status_bits(self) -> u8 {
        assert_eq!(Self::SPRITE_OVERFLOW, 5);
        assert_eq!(Self::SPRITE_ZERO_HIT, 6);
        assert_eq!(Self::VBLANK, 7);
        self.0 as u8 & (0b111 << 5)
    }

    pub fn read_pending(self) -> bool {
        self.get_flag(Self::READ_PENDING)
    }
    pub fn write_pending(self) -> bool {
        self.get_flag(Self::WRITE_PENDING)
    }
    pub fn data_latch_update_pending(self) -> bool {
        self.get_flag(Self::DATA_LATCH_UPDATE_PENDING)
    }

    pub fn set_read_pending(&mut self, pending: bool) {
        self.set_flag(Self::READ_PENDING, pending)
    }
    pub fn set_write_pending(&mut self, pending: bool) {
        self.set_flag(Self::WRITE_PENDING, pending)
    }
    pub fn set_data_latch_update_pending(&mut self, pending: bool) {
        self.set_flag(Self::DATA_LATCH_UPDATE_PENDING, pending)
    }

    const X: u16 = 0;
    const W: u16 = 3;
    const ODD_FRAME: u16 = 4;
    const SPRITE_OVERFLOW: u16 = 5;
    const SPRITE_ZERO_HIT: u16 = 6;
    const VBLANK: u16 = 7;
    const READ_PENDING: u16 = 8;
    const WRITE_PENDING: u16 = 9;
    const DATA_LATCH_UPDATE_PENDING: u16 = 10;

    pub fn set_sprite_overflow(&mut self, overflow: bool) {
        self.set_flag(Self::SPRITE_OVERFLOW, overflow);
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
struct Control(u8);
impl Control {
    pub fn init() -> Self {
        Self(0)
    }

    pub fn background_table(self) -> bool {
        get_flag_u8(self.0, Self::BACKGROUND_TABLE)
    }
    pub fn increment(self) -> bool {
        get_flag_u8(self.0, Self::INCREMENT)
    }
    pub fn nmi_enable(self) -> bool {
        get_flag_u8(self.0, Self::NMI_ENABLE)
    }

    pub fn inc_amount(self) -> u16 {
        if self.increment() {
            32
        } else {
            1
        }
    }

    const INCREMENT: u8 = 2;
    const SPRITE_TABLE: u8 = 3;
    const BACKGROUND_TABLE: u8 = 4;
    const NMI_ENABLE: u8 = 7;

    pub fn sprite_table(&self) -> bool {
        get_flag_u8(self.0, Self::SPRITE_TABLE)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
struct Mask(u8);
impl Mask {
    pub fn init() -> Self {
        Self(0)
    }

    fn background(self) -> bool {
        get_flag_u8(self.0, Self::BACKGROUND)
    }
    fn left_background(self) -> bool {
        get_flag_u8(self.0, Self::LEFT_BACKGROUND)
    }
    fn left_sprites(self) -> bool {
        get_flag_u8(self.0, Self::LEFT_SPRITES)
    }
    fn sprites(self) -> bool {
        get_flag_u8(self.0, Self::SPRITES)
    }
    fn render_enabled(self) -> bool {
        self.background() || self.sprites()
    }

    const LEFT_BACKGROUND: u8 = 1;
    const LEFT_SPRITES: u8 = 2;
    const BACKGROUND: u8 = 3;
    const SPRITES: u8 = 4;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
struct V(u16);
impl V {
    fn init() -> Self {
        Self(0)
    }

    pub fn coarse_x(self) -> u8 {
        (self.0 >> Self::COARSE_X) as u8 & 0b11111
    }
    pub fn coarse_y(self) -> u8 {
        (self.0 >> Self::COARSE_Y) as u8 & 0b11111
    }
    pub fn fine_y(self) -> u8 {
        (self.0 >> Self::FINE_Y) as u8 & 0b111
    }
    pub fn set_coarse_x(&mut self, coarse_x: u8) {
        let coarse_x = (coarse_x as u16) << Self::COARSE_X;
        let mask = 0b11111 << Self::COARSE_X;
        self.0 &= !mask;
        self.0 |= coarse_x & mask;
    }
    pub fn set_coarse_y(&mut self, coarse_y: u8) {
        let coarse_y = (coarse_y as u16) << Self::COARSE_Y;
        let mask = 0b11111 << Self::COARSE_Y;
        self.0 &= !mask;
        self.0 |= coarse_y & mask;
    }
    pub fn set_nametable(&mut self, nametable: u8) {
        let nametable = (nametable as u16) << Self::NAMETABLE;
        let mask = 0b11 << Self::NAMETABLE;
        self.0 &= !mask;
        self.0 |= nametable & mask;
    }
    pub fn set_fine_y(&mut self, fine_y: u8) {
        let fine_y = (fine_y as u16) << Self::FINE_Y;
        let mask = 0b111 << Self::FINE_Y;
        self.0 &= !mask;
        self.0 |= fine_y & mask;
    }

    pub fn tile_address(self) -> u16 {
        0x2000 | (self.0 & 0x0FFF)
    }
    pub fn attribute_address(self) -> u16 {
        let v = self.0;
        0x23C0 | (v & 0x0C00) | ((v >> 4) & 0x38) | ((v >> 2) & 0x07)
    }
    pub fn extract_attribute(self, byte: u8) -> [bool; 2] {
        let x = self.coarse_x() % 4;
        let y = self.coarse_y() % 4;
        let right = x >= 2;
        let down = y >= 2;

        let shift = match (right, down) {
            (false, false) => 0,
            (true, false) => 2,
            (false, true) => 4,
            (true, true) => 6,
        };
        let bits = (byte >> shift) & 0b11;

        let low = bits & 1 != 0;
        let high = bits & 2 != 0;

        [low, high]
    }

    pub fn increment_x(&mut self) {
        if self.coarse_x() == 31 {
            self.set_coarse_x(0);
            self.0 ^= 0x400; // Switch horizontal nametable
        } else {
            self.0 += 1; // Increment coarse x
        }
    }
    pub fn increment_y(&mut self) {
        if self.fine_y() < 7 {
            self.set_fine_y(self.fine_y() + 1);
        } else {
            self.set_fine_y(0);
            if self.coarse_y() == 29 {
                self.set_coarse_y(0);
                self.0 ^= 0x800; // Switch vertical nametable
            } else if self.coarse_y() == 31 {
                self.set_coarse_y(0);
            } else {
                self.set_coarse_y(self.coarse_y() + 1);
            }
        }
    }

    const COARSE_X: u16 = 0;
    const COARSE_Y: u16 = 5;
    const NAMETABLE: u16 = 10;
    const FINE_Y: u16 = 12;

    fn copy_horizontal_bits(&mut self, t: V) {
        let mask = (0b11111 << Self::COARSE_X) | (1 << Self::NAMETABLE);
        self.0 &= !mask;
        self.0 |= t.0 & mask;
    }

    fn copy_vertical_bits(&mut self, t: V) {
        let mask = (0b11111 << Self::COARSE_X) | (1 << Self::NAMETABLE);
        self.0 &= mask;
        self.0 |= t.0 & !mask;
    }
}

fn is_palette_address(addr: u16) -> bool {
    (0x3F00..0x4000).contains(&addr)
}
fn normalize_palette_address(addr: u16) -> usize {
    let addr = addr as usize % 0x20;
    match addr {
        0x10 => 0x00,
        0x14 => 0x04,
        0x18 => 0x08,
        0x1C => 0x0C,
        _ => addr,
    }
}

struct Shifters {
    pattern: [u16; 2],
    palette: [u8; 2],
    attribute: [bool; 2],

    next_name: u8,
    next_attribute: [bool; 2],
    next_pattern_low: u8,
}
impl Shifters {
    fn init() -> Self {
        Self {
            pattern: [0; 2],
            palette: [0; 2],
            attribute: [false; 2],

            next_name: 0,
            next_attribute: [false; 2],
            next_pattern_low: 0,
        }
    }

    fn pattern_address(&self, table: bool, fine_y: u8) -> u16 {
        let fine_y = fine_y as u16;
        let offset = self.next_name as u16 * 16;
        let base = if table { 0x1000 } else { 0 };
        base + offset + fine_y
    }

    fn pattern(&self, fine_x: u8) -> u8 {
        let fine_x = fine_x as u16;
        let mask = 1 << (15 - fine_x);
        let low = if self.pattern[0] & mask != 0 { 1 } else { 0 };
        let high = if self.pattern[1] & mask != 0 { 2 } else { 0 };
        low | high
    }
    fn palette(&self, fine_x: u8) -> u8 {
        let mask = 1 << (7 - fine_x);
        let low = if self.palette[0] & mask != 0 { 1 } else { 0 };
        let high = if self.palette[1] & mask != 0 { 2 } else { 0 };
        low | high
    }

    fn shift(&mut self) {
        self.pattern[0] = self.pattern[0].wrapping_shl(1);
        self.pattern[1] = self.pattern[1].wrapping_shl(1);
        self.palette[0] = self.palette[0].wrapping_shl(1);
        self.palette[1] = self.palette[1].wrapping_shl(1);
        self.palette[0] |= self.attribute[0] as u8;
        self.palette[1] |= self.attribute[1] as u8;
    }
    fn shift_in_tile(&mut self, pattern_high: u8) {
        self.pattern[0] |= self.next_pattern_low as u16;
        self.pattern[1] |= pattern_high as u16;
        self.attribute = self.next_attribute;
    }
}

struct Sprites {
    sprites: [Sprite; 8],
    fetch_index: u8,
    eval_index: u8,
}
impl Sprites {
    fn init() -> Sprites {
        Sprites {
            sprites: Default::default(),
            fetch_index: 0,
            eval_index: 0,
        }
    }

    fn pattern_low_address(&self, table: bool) -> u16 {
        let i = self.fetch_index as usize;
        let tile = self.sprites[i].tile as u16;
        let offset = tile * 16;
        let base = if table { 0x1000 } else { 0 };
        base + offset + self.sprites[i].y_offset as u16
    }
    fn pattern_high_address(&self, table: bool) -> u16 {
        self.pattern_low_address(table) + 8
    }

    fn fetch_low_pattern(&mut self, pattern: u8) {
        let i = self.fetch_index as usize;
        let sprite = &mut self.sprites[i];
        sprite.pattern[0] = if sprite.present { pattern } else { 0 };
    }
    fn fetch_high_pattern(&mut self, pattern: u8) {
        let i = self.fetch_index as usize;
        let sprite = &mut self.sprites[i];
        sprite.pattern[1] = if sprite.present { pattern } else { 0 };
    }
    fn next_fetch(&mut self) {
        self.fetch_index += 1;
    }
}

struct Sprite {
    present: bool,
    x: u8,
    sprite_zero: bool,
    priority: bool,
    tile: u8,
    y_offset: u8,
    hor_flip: bool,
    pattern: [u8; 2],
    palette: u8,
}
impl Default for Sprite {
    fn default() -> Self {
        Self {
            present: false,
            x: 0,
            sprite_zero: false,
            priority: false,
            tile: 0xFF,
            y_offset: 0,
            hor_flip: false,
            pattern: [0; 2],
            palette: 0,
        }
    }
}
