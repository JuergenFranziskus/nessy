use self::renderer::{color_to_rgb, Renderer};

const CYCLES_PER_LINE: u32 = 341;
const LINES_PER_FRAME: u32 = 262;
const CYCLES_PER_FRAME: u32 = CYCLES_PER_LINE * LINES_PER_FRAME;
const NMI_START: u32 = 82182;
const NMI_END: u32 = 89002;

pub mod renderer;

#[allow(dead_code)]
pub struct Ppu {
    master_cycle: u8,
    last_m2: bool,

    control: Control,
    mask: Mask,
    status: Status,

    oam_memory: [u8; 256],
    oam_address: u8,
    scroll: Scroll,
    address: u16,

    render_cycle: u32,
    odd_frame: bool,

    scheduled_mem_op: Option<MemOp>,

    fetch: Fetch,
    graphics: Graphics,
    framebuffer: [[u8; 3]; 256 * 240],
    out: OutPins,
}
impl Ppu {
    pub fn new() -> Self {
        Self {
            master_cycle: 0,
            last_m2: false,

            control: Control::init(),
            mask: Mask::init(),
            status: Status::init(),

            oam_memory: [0; 256],
            oam_address: 0,
            scroll: Scroll::init(),
            address: 0,

            render_cycle: 0,
            odd_frame: false,

            scheduled_mem_op: None,

            fetch: Fetch::init(),
            graphics: Graphics::init(),
            framebuffer: [[0; 3]; 256 * 240],
            out: OutPins::init(),
        }
    }

    pub fn master_cycle(&mut self, pins: InPins) {
        if self.should_cycle_ppu() {
            self.ppu_cycle(pins);
        }

        self.service_cpu(pins);
        self.tick_counter();
        self.last_m2 = pins.cpu_m2;
    }
    fn should_cycle_ppu(&self) -> bool {
        self.master_cycle == 0
    }
    fn tick_counter(&mut self) {
        self.master_cycle += 1;
        self.master_cycle %= 4;
    }

    fn ppu_cycle(&mut self, pins: InPins) {
        self.out.ale = false;
        self.out.read_enable = false;
        self.out.write_enable = false;

        self.decide_nmi();
        self.perform_mem_op();
        self.fetch_graphics(pins);
        self.render();
        self.tick_pixel();
    }
    fn service_cpu(&mut self, pins: InPins) {
        let m2_edge = self.last_m2 != pins.cpu_m2;
        if !m2_edge || !pins.cpu_m2 {
            return;
        }

        self.out.cross_data_busses = false;
        self.out.cpu_data = None;
        let in_range = (0x2000..0x4000).contains(&pins.cpu_address);
        if !in_range {
            return;
        }

        let address = pins.cpu_address as usize % 8;
        match address {
            0 => {
                if pins.cpu_read {
                    return;
                }

                self.control.reconfig(pins.cpu_data);
                let high_x = pins.cpu_data & 1 != 0;
                let high_y = pins.cpu_data & 2 != 0;
                self.scroll.write_high(high_x, high_y);
            }
            1 => {
                if pins.cpu_read {
                    return;
                }

                self.mask.reconfig(pins.cpu_data);
            }
            2 => {
                self.out.cpu_data = Some(self.status.get_read());
                self.address = 0;
                self.scroll = Scroll::init();
            }
            3 => {
                if pins.cpu_read {
                    return;
                }
                self.oam_address = pins.cpu_data;
            }
            4 => {
                let address = self.oam_address as usize;
                if pins.cpu_read {
                    self.out.cpu_data = Some(self.oam_memory[address]);
                } else {
                    self.oam_memory[address] = pins.cpu_data;
                    self.oam_address = self.oam_address.wrapping_add(1);
                }
            }
            5 => {
                if pins.cpu_read {
                    return;
                }

                self.scroll.write_low(pins.cpu_data);
            }
            6 => {
                if pins.cpu_read {
                    return;
                }

                self.address <<= 8;
                self.address |= pins.cpu_data as u16;
                self.address %= 0x4000;
            }
            7 => {
                if pins.cpu_read {
                    self.schedule_read(self.address);
                    self.out.cross_data_busses = true;
                } else {
                    self.schedule_write(self.address, pins.cpu_data);
                }

                self.address += self.control.vram_address_increment;
                self.address %= 0x4000;
            }
            _ => unreachable!("#{address} is not a valid ppu register or is not yet implemented"),
        }
    }
    fn perform_mem_op(&mut self) {
        let Some(op) = self.scheduled_mem_op.take() else { return };

        let address_high = op.address & !0xFF;
        self.out.mem_address_data = address_high | op.data.unwrap_or(0) as u16;
        self.out.read_enable = op.data.is_none();
        self.out.write_enable = op.data.is_some();
    }
    fn decide_nmi(&mut self) {
        if self.render_cycle == NMI_START {
            self.status.vblank = true;
        } else if self.render_cycle == NMI_END {
            self.status.vblank = false;
        }

        self.out.nmi = self.control.generate_nmi && self.status.vblank;
    }
    fn tick_pixel(&mut self) {
        let final_cycle = if self.odd_frame {
            CYCLES_PER_FRAME - 1
        } else {
            CYCLES_PER_FRAME - 2
        };

        if self.render_cycle == final_cycle {
            self.render_cycle = 0;
            self.odd_frame = !self.odd_frame;
        } else {
            self.render_cycle += 1;
        }
    }

    fn schedule_mem_op(&mut self, address: u16, data: Option<u8>) {
        self.scheduled_mem_op = Some(MemOp { address, data });
        self.out.mem_address_data = address;
        self.out.ale = true;
    }
    fn schedule_read(&mut self, address: u16) {
        self.schedule_mem_op(address, None);
    }
    fn schedule_write(&mut self, address: u16, data: u8) {
        self.schedule_mem_op(address, Some(data));
    }

    fn fetch_graphics(&mut self, pins: InPins) {
        if self.mask.render_disabled() {
            return;
        }

        let scanline = self.render_cycle / CYCLES_PER_LINE;
        let line_cycle = self.render_cycle % CYCLES_PER_LINE;

        if scanline == LINES_PER_FRAME - 1 && line_cycle == 0 {
            self.fetch.start_fetch();
        }

        if !self.fetch.fetching {
            return;
        }

        match self.fetch.awaiting {
            Awaiting::None => (),
            Awaiting::Name => {
                self.graphics.nametable[self.fetch.nametable as usize] = pins.mem_data;
                self.fetch.nametable += 1;
            }
            Awaiting::Pattern => {
                self.graphics.pattern_table[self.fetch.pattern_table as usize] = pins.mem_data;
                self.fetch.pattern_table += 1;
            }
            Awaiting::Palette => {
                let palette_i = self.fetch.palette as usize / 3;
                let color_i = self.fetch.palette as usize % 3;
                self.graphics.palette[palette_i][color_i] = pins.mem_data;
                self.fetch.palette += 1;
            }
            Awaiting::Background => {
                self.graphics.background = pins.mem_data;
                self.fetch.background = true;
            }
        }
        self.fetch.awaiting = Awaiting::None;

        match line_cycle {
            0 => (),
            1..=256 => {
                let name_done = self.fetch.nametable >= 4096;
                let pattern_done = self.fetch.pattern_table >= 8192;
                let background_done = self.fetch.background;
                let palette_done = self.fetch.palette >= 24;

                if pattern_done && name_done && background_done && palette_done {
                    self.fetch.fetching = false;
                    return;
                }

                let turn = match self.fetch.cycle % 8 {
                    0 | 1 if !name_done => Awaiting::Name,
                    0 | 1 => Awaiting::Pattern,
                    2 | 3 if !pattern_done => Awaiting::Pattern,
                    2 | 3 => unreachable!(),
                    4 | 5 if !background_done => Awaiting::Background,
                    4 | 5 if !name_done => Awaiting::Name,
                    4 | 5 => Awaiting::Pattern,
                    6 | 7 if !palette_done => Awaiting::Palette,
                    6 | 7 => Awaiting::Pattern,
                    _ => unreachable!(),
                };
                let step = self.fetch.cycle % 2;

                let address = match turn {
                    Awaiting::None => {
                        dbg!(name_done, pattern_done, background_done, palette_done);
                        panic!()
                    }
                    Awaiting::Name => self.fetch.nametable + 0x2000,
                    Awaiting::Pattern => self.fetch.pattern_table,
                    Awaiting::Background => 0x3F00,
                    Awaiting::Palette => {
                        let palette = self.fetch.palette / 3;
                        let color = self.fetch.palette % 3;
                        let offset = palette * 4 + color;
                        0x3F01 + offset
                    }
                };

                match step {
                    0 => self.schedule_read(address),
                    1 => self.fetch.awaiting = turn,
                    _ => unreachable!(),
                }

                self.fetch.cycle += 1;
            }
            337 => self.schedule_read(0x2000),
            338 => (),
            339 => self.schedule_read(0x2000),
            _ => (),
        }
    }
    fn render(&mut self) {
        if self.render_cycle != NMI_START {
            return;
        }
        if self.mask.render_disabled() {
            self.framebuffer = [color_to_rgb(self.graphics.background); 256 * 240];
            return;
        }

        let renderer = Renderer::new(
            &mut self.framebuffer,
            &self.graphics.pattern_table,
            &self.graphics.nametable,
            self.control.background_pattern_table,
            self.graphics.background,
            self.graphics.palette,
            self.scroll,
        );
        renderer.render();
    }

    pub fn out(&self) -> OutPins {
        self.out
    }

    pub fn framebuffer(&self) -> &[[u8; 3]; 256 * 240] {
        &self.framebuffer
    }
}

struct MemOp {
    address: u16,
    data: Option<u8>,
}

#[derive(Copy, Clone, Debug)]
pub struct InPins {
    pub cpu_m2: bool,
    pub cpu_read: bool,
    pub cpu_address: u16,
    pub cpu_data: u8,
    pub mem_data: u8,
}
impl InPins {
    pub fn init() -> Self {
        Self {
            cpu_m2: false,
            cpu_read: true,
            cpu_address: 0,
            cpu_data: 0,

            mem_data: 0,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct OutPins {
    pub nmi: bool,
    pub cpu_data: Option<u8>,
    pub mem_address_data: u16,
    pub ale: bool,
    pub read_enable: bool,
    pub write_enable: bool,
    pub cross_data_busses: bool,
}
impl OutPins {
    pub fn init() -> Self {
        Self {
            nmi: false,
            cpu_data: None,
            mem_address_data: 0,
            ale: false,
            read_enable: false,
            write_enable: false,
            cross_data_busses: false,
        }
    }
}

struct Control {
    vram_address_increment: u16,
    sprite_pattern_table: bool,
    background_pattern_table: bool,
    wide_sprites: bool,
    generate_nmi: bool,
}
impl Control {
    fn init() -> Self {
        Self {
            vram_address_increment: 1,
            sprite_pattern_table: false,
            background_pattern_table: false,
            wide_sprites: false,
            generate_nmi: false,
        }
    }

    fn reconfig(&mut self, data: u8) {
        self.vram_address_increment = if data & 4 != 0 { 32 } else { 1 };
        self.sprite_pattern_table = data & 8 != 0;
        self.background_pattern_table = data & 16 != 0;
        self.wide_sprites = data & 32 != 0;
        self.generate_nmi = data & 128 != 0;
    }
}

struct Mask {
    greyscale: bool,
    show_leftmost_background: bool,
    show_leftmost_sprites: bool,
    show_background: bool,
    show_sprites: bool,
    emphasize_red: bool,
    emphasize_green: bool,
    emphasize_blue: bool,
}
impl Mask {
    fn init() -> Self {
        Self {
            greyscale: false,
            show_leftmost_background: true,
            show_leftmost_sprites: true,
            show_background: false,
            show_sprites: false,
            emphasize_red: false,
            emphasize_green: false,
            emphasize_blue: false,
        }
    }

    fn reconfig(&mut self, data: u8) {
        self.greyscale = data & 1 != 0;
        self.show_leftmost_background = data & 2 != 0;
        self.show_leftmost_sprites = data & 4 != 0;
        self.show_background = data & 8 != 0;
        self.show_sprites = data & 16 != 0;
        self.emphasize_red = data & 32 != 0;
        self.emphasize_green = data & 64 != 0;
        self.emphasize_blue = data & 128 != 0;
    }

    fn render_disabled(&self) -> bool {
        !self.show_background && !self.show_sprites
    }
}

struct Status {
    sprite_overflow: bool,
    sprite_zero_hit: bool,
    vblank: bool,
}
impl Status {
    fn init() -> Self {
        Self {
            sprite_overflow: false,
            sprite_zero_hit: false,
            vblank: false,
        }
    }

    fn get_read(&mut self) -> u8 {
        let overflow = (self.sprite_overflow as u8) << 5;
        let hit = (self.sprite_zero_hit as u8) << 6;
        let blank = (self.vblank as u8) << 7;
        self.vblank = false;

        overflow | hit | blank
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct Scroll {
    x: (u8, bool),
    y: (u8, bool),
}
impl Scroll {
    pub fn init() -> Self {
        Self {
            x: (0, false),
            y: (0, false),
        }
    }

    pub fn write_low(&mut self, value: u8) {
        self.x.0 = self.y.0;
        self.y.0 = value;
    }
    pub fn write_high(&mut self, x: bool, y: bool) {
        self.x.1 = x;
        self.y.1 = y;
    }
}

struct Fetch {
    fetching: bool,
    awaiting: Awaiting,
    cycle: u16,
    nametable: u16,
    pattern_table: u16,
    palette: u16,
    background: bool,
}
impl Fetch {
    fn init() -> Self {
        Self {
            awaiting: Awaiting::None,
            fetching: false,
            cycle: 0,
            nametable: 0,
            pattern_table: 0,
            palette: 0,
            background: false,
        }
    }

    fn start_fetch(&mut self) {
        self.cycle = 0;
        self.fetching = true;
        self.nametable = 0;
        self.pattern_table = 0;
        self.palette = 0;
        self.background = false;
    }
}

enum Awaiting {
    None,
    Name,
    Pattern,
    Palette,
    Background,
}

struct Graphics {
    nametable: Box<[u8; 4096]>,
    pattern_table: Box<[u8; 8192]>,
    background: u8,
    palette: [[u8; 3]; 8],
}
impl Graphics {
    fn init() -> Self {
        Self {
            nametable: Box::new([0; 4096]),
            pattern_table: Box::new([0; 8192]),
            background: 0,
            palette: [[0; 3]; 8],
        }
    }
}
