use crate::nes::NesBus;

const CYCLES_PER_LINE: u32 = 341;
const LINES_PER_FRAME: u32 = 262;
const CYCLES_PER_FRAME: u32 = CYCLES_PER_LINE * LINES_PER_FRAME;
const NMI_START: u32 = 82182;
const NMI_END: u32 = 89002;

#[allow(dead_code)]
pub struct Ppu {
    last_m2: bool,

    control: Control,
    mask: Mask,
    status: Status,

    palette_memory: Box<[u8; 0x20]>,
    oam_memory: Box<[u8; 256]>,
    oam_address: u8,
    scroll: Scroll,

    ppudata_address: u16,
    ppudata_latch: u8,
    ppudata_update: PpuDataUpdate,

    render_cycle: u32,
    odd_frame: bool,

    scheduled_mem_op: Option<MemOp>,
    framebuffer: Box<[[u8; 3]; 256 * 240]>,
}
impl Ppu {
    pub fn new() -> Self {
        Self {
            last_m2: false,

            control: Control::init(),
            mask: Mask::init(),
            status: Status::init(),

            palette_memory: Box::new([0; 0x20]),
            oam_memory: Box::new([0; 256]),
            oam_address: 0,
            scroll: Scroll::init(),

            ppudata_address: 0,
            ppudata_latch: 0,
            ppudata_update: PpuDataUpdate::UpToDate,

            render_cycle: 0,
            odd_frame: false,

            scheduled_mem_op: None,
            framebuffer: Box::new([[0; 3]; 256 * 240]),
        }
    }

    pub fn master_cycle(&mut self, bus: &mut NesBus, cycle: u64) {
        if self.should_cycle_ppu(cycle) {
            self.ppu_cycle(bus);
        }

        self.service_cpu(bus);
        self.last_m2 = bus.cpu_m2;
    }
    fn should_cycle_ppu(&self, cycle: u64) -> bool {
        cycle % 4 == 0
    }

    fn ppu_cycle(&mut self, bus: &mut NesBus) {
        bus.ppu_read_enable = false;
        bus.ppu_write_enable = false;

        self.decide_nmi(bus);
        self.perform_mem_op(bus);
        self.update_ppudata_latch(bus);
        self.tick_pixel();
    }
    fn service_cpu(&mut self, bus: &mut NesBus) {
        let m2_edge = self.last_m2 != bus.cpu_m2;
        if !m2_edge || !bus.cpu_m2 {
            return;
        }

        let in_range = (0x2000..0x4000).contains(&bus.cpu_address);
        if !in_range {
            return;
        }

        let address = bus.cpu_address as usize % 8;
        match address {
            0 => {
                if bus.cpu_read {
                    return;
                }

                self.control.reconfig(bus.cpu_data);
                let high_x = bus.cpu_data & 1 != 0;
                let high_y = bus.cpu_data & 2 != 0;
                self.scroll.write_high(high_x, high_y);
            }
            1 => {
                if bus.cpu_read {
                    return;
                }

                self.mask.reconfig(bus.cpu_data);
            }
            2 => {
                if !bus.cpu_read {
                    return;
                }

                bus.cpu_data = self.status.get_read();
                self.ppudata_address = 0;
                self.scroll = Scroll::init();
            }
            3 => {
                if bus.cpu_read {
                    return;
                }
                self.oam_address = bus.cpu_data;
            }
            4 => {
                let address = self.oam_address as usize;
                if bus.everyone_reads_cpu_bus() {
                    bus.cpu_data = self.oam_memory[address];
                } else {
                    self.oam_memory[address] = bus.cpu_data;
                    self.oam_address = self.oam_address.wrapping_add(1);
                }
            }
            5 => {
                if bus.cpu_read {
                    return;
                }

                self.scroll.write_low(bus.cpu_data);
            }
            6 => {
                if bus.cpu_read {
                    return;
                }

                self.ppudata_address <<= 8;
                self.ppudata_address |= bus.cpu_data as u16;
                self.ppudata_address %= 0x4000;
            }
            7 => {
                let address = self.ppudata_address as usize;
                let palette_range = (0x3F00..).contains(&address);
                let palette_address = address.wrapping_sub(0x3F00) % 0x20;
                if bus.cpu_read && palette_range {
                    bus.cpu_data = self.read_palette_memory(palette_address);
                    self.increment_ppudata_address();
                } else if palette_range {
                    self.write_palette_memory(palette_address, bus.cpu_data);
                    self.increment_ppudata_address();
                } else if bus.cpu_read {
                    bus.cpu_data = self.ppudata_latch;
                    self.ppudata_update = PpuDataUpdate::Scheduled;
                } else {
                    self.schedule_write(self.ppudata_address, bus.cpu_data);
                    self.increment_ppudata_address();
                }
            }
            _ => unreachable!("#{address} is not a valid ppu register or is not yet implemented"),
        }
    }
    fn perform_mem_op(&mut self, bus: &mut NesBus) {
        let Some(op) = self.scheduled_mem_op.take() else { return };

        bus.ppu_address = op.address;
        bus.ppu_read_enable = op.data.is_none();
        bus.ppu_write_enable = op.data.is_some();
        if let Some(data) = op.data {
            bus.ppu_data = data;
        }
    }
    fn update_ppudata_latch(&mut self, bus: &mut NesBus) {
        use PpuDataUpdate::*;
        match self.ppudata_update {
            UpToDate => (),
            Scheduled => {
                self.schedule_read(self.ppudata_address);
                self.ppudata_update = AwaitingData;
            }
            AwaitingData => self.ppudata_update = StillWaiting,
            StillWaiting => {
                self.ppudata_latch = bus.ppu_data;
                self.ppudata_update = UpToDate;
                self.increment_ppudata_address();
            }
        }
    }
    fn decide_nmi(&mut self, bus: &mut NesBus) {
        if self.render_cycle == NMI_START {
            self.status.vblank = true;
        } else if self.render_cycle == NMI_END {
            self.status.vblank = false;
        }

        bus.ppu_nmi = self.control.generate_nmi && self.status.vblank;
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
    }
    fn schedule_read(&mut self, address: u16) {
        self.schedule_mem_op(address, None);
    }
    fn schedule_write(&mut self, address: u16, data: u8) {
        self.schedule_mem_op(address, Some(data));
    }

    fn read_palette_memory(&self, address: usize) -> u8 {
        let address = Self::normalize_palette_address(address);
        self.palette_memory[address]
    }
    fn write_palette_memory(&mut self, address: usize, data: u8) {
        let address = Self::normalize_palette_address(address);
        self.palette_memory[address] = data;
    }
    fn normalize_palette_address(address: usize) -> usize {
        match address {
            0x10 => 0x0,
            0x14 => 0x4,
            0x18 => 0x8,
            0x1C => 0xC,
            _ => address,
        }
    }

    fn increment_ppudata_address(&mut self) {
        self.ppudata_address += self.control.vram_address_increment;
        self.ppudata_address %= 0x4000;
    }

    pub fn framebuffer(&self) -> &[[u8; 3]; 256 * 240] {
        &self.framebuffer
    }
}

struct MemOp {
    address: u16,
    data: Option<u8>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum PpuDataUpdate {
    UpToDate,
    Scheduled,
    AwaitingData,
    StillWaiting,
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
