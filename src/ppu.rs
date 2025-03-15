use crate::apu::Bus as CpuBus;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Bus {
    pub addr: u16,
    pub data: u8,
    flags: u8,
}
impl Bus {
    pub fn new() -> Self {
        Self {
            addr: 0,
            data: 0,
            flags: 0,
        }
    }

    pub fn io(&mut self) {
        self.set_rd(false);
        self.set_wr(false);
    }
    pub fn read(&mut self, addr: u16) {
        self.addr = addr;
        self.set_rd(true);
        self.set_wr(false);
    }
    pub fn write(&mut self, addr: u16, data: u8) {
        self.addr = addr;
        self.data = data;
        self.set_rd(false);
        self.set_wr(true);
    }

    pub fn rd(self) -> bool {
        self.flags & Self::RD != 0
    }
    pub fn wr(self) -> bool {
        self.flags & Self::WR != 0
    }

    pub fn set_rd(&mut self, to: bool) {
        self.flags &= !Self::RD;
        if to {
            self.flags |= Self::RD;
        }
    }
    pub fn set_wr(&mut self, to: bool) {
        self.flags &= !Self::WR;
        if to {
            self.flags |= Self::WR;
        }
    }

    const RD: u8 = 1;
    const WR: u8 = 2;
}

const DOTS_PER_LINE: u32 = 341;
const LINES_PER_FRAME: u32 = 262;
const DOTS_PER_FRAME: u32 = DOTS_PER_LINE * LINES_PER_FRAME;

const SET_VBLANK: u32 = 241 * DOTS_PER_LINE + 1;
const CLEAR_VBLANK: u32 = 261 * DOTS_PER_LINE + 1;
const LAST_EVEN_DOT: u32 = DOTS_PER_FRAME - 1;
const LAST_ODD_DOT: u32 = DOTS_PER_FRAME - 2;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Ppu {
    dot: u32,
    odd_frame: bool,

    ctrl: Ctrl,
    mask: Mask,
    sprite_overflow: bool,
    sprite_0_hit: bool,
    vblank: bool,
    oam_addr: u8,
    data: u8,

    t: u16,
    v: u16,
    w: bool,
    x: u8,

    oam: [u8; 256],
    palette: [u8; 32],
    mem: Mem,

    shifters: Shifters,

    pixel: u8,
    pixel_coord: [u32; 2],
}
impl Ppu {
    pub fn start() -> Ppu {
        Self {
            dot: 0,
            odd_frame: false,

            ctrl: Ctrl(0),
            mask: Mask(0),
            sprite_overflow: false,
            sprite_0_hit: false,
            vblank: false,
            data: 0,
            oam_addr: 0,

            t: 0,
            v: 0,
            w: false,
            x: 0,

            oam: [0; 256],
            palette: [0; 32],
            mem: Mem::Idle,

            shifters: Shifters::new(),

            pixel: 0,
            pixel_coord: [0; 2],
        }
    }

    pub fn output(&self) -> (u8, u32, u32) {
        (self.pixel, self.pixel_coord[0], self.pixel_coord[1])
    }
    pub fn is_vblank(&self) -> bool {
        self.vblank
    }

    pub fn clock(&mut self, bus: &mut Bus, cpu: &mut CpuBus, cpu_clock: bool) {
        self.do_mem_access(bus);
        if cpu_clock {
            self.handle_cpu(cpu);
        }
        self.render(bus);
        self.tick_dot();
        self.set_nmi(cpu);
    }
    fn do_mem_access(&mut self, bus: &mut Bus) {
        let mem = std::mem::replace(&mut self.mem, Mem::Idle);
        match mem {
            Mem::Idle => bus.io(),
            Mem::Read(addr, d) => {
                bus.read(addr);
                if d {
                    self.mem = Mem::UpdatePpuData
                }
            }
            Mem::UpdatePpuData => self.data = bus.data,
            Mem::Write(addr, data) => bus.write(addr, data),
        }
    }
    fn handle_cpu(&mut self, cpu: &mut CpuBus) {
        let addr = cpu.addr as usize;
        if addr < 0x2000 || addr >= 0x4000 {
            return;
        };
        let reg = addr % 8;
        let rw = cpu.rw();

        match reg {
            0 if !rw => {
                self.ctrl.0 = cpu.data & 0xFC;
                self.t = set_nametable_select(self.t, cpu.data);
            }
            1 if !rw => self.mask.0 = cpu.data,
            2 if rw => {
                let o = if self.sprite_overflow { 32 } else { 0 };
                let s = if self.sprite_0_hit { 64 } else { 0 };
                let v = if self.vblank { 128 } else { 0 };
                cpu.data &= 0x1F;
                cpu.data |= o | s | v;

                self.vblank = false;
                self.w = false;
            }
            3 if !rw => self.oam_addr = cpu.data,
            4 => {
                if rw {
                    cpu.data = self.oam[self.oam_addr as usize];
                } else {
                    self.oam[self.oam_addr as usize] = cpu.data;
                }
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            5 if !rw => {
                if !self.w {
                    self.w = true;

                    let data = cpu.data;
                    self.x = data & 0x7;
                    self.t = set_coarse_x(self.t, data >> 3);
                } else {
                    self.w = false;

                    let data = cpu.data;
                    self.t = set_coarse_y(self.t, data >> 3);
                    self.t = set_fine_y(self.t, data & 0x7);
                }
            }
            6 if !rw => {
                if !self.w {
                    self.w = true;

                    let data = cpu.data;
                    self.t &= 0xFF;
                    self.t |= (data as u16 & 0x3F) << 8;
                } else {
                    self.w = false;

                    let data = cpu.data as u16;
                    self.t &= 0xFF00;
                    self.t |= data;
                    self.v = self.t;
                }
            }
            7 => {
                if self.v >= 0x3F00 && self.v < 0x4000 {
                    let offset = self.v % 32;
                    let offset = mirror_palette_offset(offset as u8) as usize;
                    if cpu.rw() {
                        cpu.data = self.palette[offset];
                        self.read_ppu_data(self.v);
                    } else {
                        self.palette[offset] = cpu.data;
                    }
                } else {
                    if cpu.rw() {
                        cpu.data = self.data;
                        self.read_ppu_data(self.v);
                    } else {
                        self.write(self.v, cpu.data);
                    }
                }

                self.increment_v();
            }
            8.. => unreachable!(),
            _ => (),
        }
    }
    fn tick_dot(&mut self) {
        let render = self.mask.enable_bg() || self.mask.enable_sp();
        let last_dot = if render && self.odd_frame {
            LAST_ODD_DOT
        } else {
            LAST_EVEN_DOT
        };

        if self.dot == last_dot {
            self.dot = 0;
            self.odd_frame = !self.odd_frame;
        } else {
            if self.dot == SET_VBLANK {
                self.vblank = true;
            } else if self.dot == CLEAR_VBLANK {
                self.vblank = false;
                self.sprite_0_hit = false;
                self.sprite_overflow = false;
            }

            self.dot += 1;
        }
    }
    fn set_nmi(&self, cpu: &mut CpuBus) {
        cpu.set_nmi(self.ctrl.v() && self.vblank);
    }

    fn render(&mut self, bus: &mut Bus) {
        if !self.mask.enable_bg() && !self.mask.enable_sp() {
            return;
        };

        let x = self.dot % DOTS_PER_LINE;
        let y = self.dot / DOTS_PER_LINE;

        match y {
            0..240 => self.visible_scanline(x, y, false, bus),
            240..261 => (),
            261 => self.visible_scanline(x, y, true, bus),
            LINES_PER_FRAME.. => unreachable!(),
        }
    }
    fn visible_scanline(&mut self, x: u32, y: u32, prerender: bool, bus: &mut Bus) {
        match x {
            0 => (),
            1..257 => {
                let step = x - 1;
                self.fetch_background(step, bus);
                if !prerender {
                    self.produce_pixel(x - 1, y);
                    self.shifters.shift_next_pixel();
                }
            }
            257 => {
                self.v = inc_vert(self.v);
                self.v = copy_hori(self.v, self.t);
            }
            258..280 => (),
            280..305 => {
                if prerender {
                    self.v = copy_vert(self.v, self.t);
                }
            }
            305..321 => (),
            321..337 => self.fetch_background(x - 321, bus),
            337 => {
                self.latch_hi_pattern(bus);
                self.fetch_name();
            }
            338 => (),
            339 => self.fetch_name(),
            340 => (),
            DOTS_PER_LINE.. => unreachable!(),
        }
    }
    fn fetch_background(&mut self, step: u32, bus: &mut Bus) {
        match step % 8 {
            0 => {
                if step != 0 {
                    self.latch_hi_pattern(bus);
                }

                self.fetch_name();
            }
            1 => (),
            2 => {
                self.shifters.name = bus.data;
                self.fetch_attr();
            }
            3 => (),
            4 => {
                self.shifters.next_palette = bus.data;
                self.fetch_pattern_lo();
            }
            5 => (),
            6 => {
                self.shifters.next_pattern[0] = bus.data;
                self.fetch_pattern_hi();
            }
            7 => (),
            8.. => unreachable!(),
        }
    }
    fn latch_hi_pattern(&mut self, bus: &mut Bus) {
        self.shifters.next_pattern[1] = bus.data;
        self.shifters
            .shift_next_tile(coarse_x(self.v), coarse_y(self.v));
        self.v = inc_hori(self.v);
    }
    fn fetch_name(&mut self) {
        self.read(name_addr(self.v))
    }
    fn fetch_attr(&mut self) {
        self.read(attr_addr(self.v));
    }
    fn fetch_pattern_lo(&mut self) {
        self.read(self.pattern_addr());
    }
    fn fetch_pattern_hi(&mut self) {
        self.read(self.pattern_addr() + 8);
    }
    fn pattern_addr(&self) -> u16 {
        let name = self.shifters.name;
        let base = name as u16 * 16;
        let fine_y = fine_y(self.v) as u16;
        let h = if self.ctrl.b() { 0x1000 } else { 0 };
        base | fine_y | h
    }

    fn produce_pixel(&mut self, x: u32, y: u32) {
        let (bg, _bg_transpi) = self.produce_background();

        let pixel = bg;
        self.pixel = pixel;
        self.pixel_coord = [x, y];
        //println!("{x} {y}: {pixel}");
    }
    fn produce_background(&self) -> (u8, bool) {
        let fine_x = self.x;
        let pattern_lo = self.shifters.pattern[0] >> fine_x & 1 != 0;
        let pattern_hi = self.shifters.pattern[1] >> fine_x & 1 != 0;
        let pattern = if pattern_lo { 1 } else { 0 } | if pattern_hi { 2 } else { 0 };

        if !pattern_lo && !pattern_hi {
            (self.palette[0], true)
        } else {
            let palette_lo = self.shifters.palette[0] >> fine_x & 1 != 0;
            let palette_hi = self.shifters.palette[1] >> fine_x & 1 != 0;
            let palette = if palette_lo { 1 } else { 0 } | if palette_hi { 2 } else { 0 };

            let color_idx = pattern | palette << 2;
            let color_idx = mirror_palette_offset(color_idx);
            let color = self.palette[color_idx as usize];

            (color, false)
        }
    }

    fn increment_v(&mut self) {
        let by = if self.ctrl.i() { 32 } else { 1 };
        self.v += by;
        self.v &= 0x3FFF;
    }
    fn read(&mut self, addr: u16) {
        self.mem = Mem::Read(addr, false);
    }
    fn read_ppu_data(&mut self, addr: u16) {
        self.mem = Mem::Read(addr, true);
    }
    fn write(&mut self, addr: u16, data: u8) {
        self.mem = Mem::Write(addr, data);
    }
}

fn coarse_x(v: u16) -> u8 {
    (v & 0x1F) as u8
}
fn coarse_y(v: u16) -> u8 {
    (v >> 5 & 0x1F) as u8
}
fn fine_y(v: u16) -> u8 {
    (v >> 12 & 0x7) as u8
}
fn set_nametable_select(v: u16, to: u8) -> u16 {
    let to = to as u16 & 0x3;
    let mask = 0b11_00000_00000;
    (v & !mask) | to << 10
}
fn set_coarse_x(v: u16, to: u8) -> u16 {
    let to = to as u16 & 0x1F;
    let mask = 0x1F;
    (v & !mask) | to
}
fn set_coarse_y(v: u16, to: u8) -> u16 {
    let to = to as u16 & 0x1F;
    let mask = 0x1F << 5;
    (v & !mask) | to << 5
}
fn set_fine_y(v: u16, to: u8) -> u16 {
    let to = to as u16 & 0x7;
    let mask = 0x7 << 12;
    (v & !mask) | to << 12
}
fn name_addr(v: u16) -> u16 {
    0x2000 | (v & 0x0FFF)
}
fn attr_addr(v: u16) -> u16 {
    0x23C0 | (v & 0x0C00) | ((v >> 4) & 0x38) | ((v >> 2) & 0x07)
}
fn inc_hori(mut v: u16) -> u16 {
    if v & 0x001F == 31 {
        v &= !0x001F;
        v ^= 0x0400;
    } else {
        v += 1;
    }

    v
}
fn inc_vert(mut v: u16) -> u16 {
    if v & 0x7000 != 0x7000 {
        v += 0x1000;
    } else {
        v &= !0x7000;
        let mut y = (v & 0x03E0) >> 5;
        if y == 29 {
            y = 0;
            v ^= 0x0800;
        } else if y == 31 {
            y = 0;
        } else {
            y += 1;
        }
        v = (v & !0x03E0) | (y << 5);
    }

    v
}
fn copy_hori(v: u16, t: u16) -> u16 {
    let mask = 0b100_00011111;
    (v & !mask) | (t & mask)
}
fn copy_vert(v: u16, t: u16) -> u16 {
    let mask = 0b1111011_11100000;
    (v & !mask) | (t & mask)
}

fn mirror_palette_offset(offset: u8) -> u8 {
    if offset == 0x10 || offset == 0x14 || offset == 0x18 || offset == 0x1C {
        offset - 0x10
    } else {
        offset
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct Ctrl(u8);
impl Ctrl {
    fn i(self) -> bool {
        self.0 & Self::I != 0
    }
    fn s(self) -> bool {
        self.0 & Self::S != 0
    }
    fn b(self) -> bool {
        self.0 & Self::B != 0
    }
    fn h(self) -> bool {
        self.0 & Self::H != 0
    }
    fn p(self) -> bool {
        self.0 & Self::P != 0
    }
    fn v(self) -> bool {
        self.0 & Self::V != 0
    }

    const I: u8 = 4;
    const S: u8 = 8;
    const B: u8 = 16;
    const H: u8 = 32;
    const P: u8 = 64;
    const V: u8 = 128;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct Mask(u8);
impl Mask {
    fn greyscale(self) -> bool {
        self.0 & Self::GREYSCALE != 0
    }
    fn left_bg(self) -> bool {
        self.0 & Self::LEFT_BG != 0
    }
    fn left_sp(self) -> bool {
        self.0 & Self::LEFT_SP != 0
    }
    fn enable_bg(self) -> bool {
        self.0 & Self::ENABLE_BG != 0
    }
    fn enable_sp(self) -> bool {
        self.0 & Self::ENABLE_SP != 0
    }
    fn emph_red(self) -> bool {
        self.0 & Self::EMPH_RED != 0
    }
    fn emph_green(self) -> bool {
        self.0 & Self::EMPH_GREEN != 0
    }
    fn emph_blue(self) -> bool {
        self.0 & Self::EMPH_BLUE != 0
    }

    const GREYSCALE: u8 = 1;
    const LEFT_BG: u8 = 2;
    const LEFT_SP: u8 = 4;
    const ENABLE_BG: u8 = 8;
    const ENABLE_SP: u8 = 16;
    const EMPH_RED: u8 = 32;
    const EMPH_GREEN: u8 = 64;
    const EMPH_BLUE: u8 = 128;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Mem {
    Idle,
    Read(u16, bool),
    UpdatePpuData,
    Write(u16, u8),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct Shifters {
    name: u8,
    pattern: [u16; 2],
    next_pattern: [u8; 2],
    palette: [u8; 2],
    curr_palette: [bool; 2],
    next_palette: u8,
}
impl Shifters {
    fn new() -> Self {
        Self {
            name: 0,
            pattern: [0; 2],
            next_pattern: [0; 2],
            palette: [0; 2],
            curr_palette: [false; 2],
            next_palette: 0,
        }
    }

    fn shift_next_tile(&mut self, coarse_x: u8, coarse_y: u8) {
        for (p, n) in self.pattern.iter_mut().zip(self.next_pattern) {
            *p &= 0xFF;
            *p |= (n.reverse_bits() as u16) << 8;
        }
        let cx_1 = if coarse_x & 2 != 0 { 1 } else { 0 };
        let cy_1 = if coarse_y & 2 != 0 { 2 } else { 0 };
        let shift = (cx_1 | cy_1) * 2;
        let palette = (self.next_palette >> shift) & 0x3;
        let palette_lo = palette & 1 != 0;
        let palette_hi = palette & 2 != 0;
        self.curr_palette = [palette_lo, palette_hi];
    }
    fn shift_next_pixel(&mut self) {
        for p in &mut self.pattern {
            *p >>= 1;
            *p |= 0x8000;
        }

        for (p, c) in self.palette.iter_mut().zip(self.curr_palette) {
            *p >>= 1;
            *p |= if c { 128 } else { 0 };
        }
    }
}
