const PIXELS_PER_LINE: u16 = 341;
const LINES_PER_FRAME: u16 = 262;
const NMI_START: [u16; 2] = [1, 241];
const NMI_END: [u16; 2] = [1, 261];

#[allow(dead_code)]
pub struct Ppu {
    base_nametable_address: u16,
    vram_address_increment: u16,
    sprite_pattern_table: u16,
    background_pattern_table: u16,
    wide_sprites: bool,
    generate_nmi: bool,
    greyscale: bool,
    show_leftmost_background: bool,
    show_leftmost_sprites: bool,
    show_background: bool,
    show_sprites: bool,
    emphasize_red: bool,
    emphasize_green: bool,
    emphasize_blue: bool,

    sprite_overflow: bool,
    sprite_zero_hit: bool,
    vblank: bool,

    oam_memory: [u8; 256],
    oam_address: u8,
    scroll: [u8; 2],
    address: u16,

    pixel: [u16; 2],
    odd_frame: bool,

    cpu_service_pending: bool,
    scheduled_mem_op: Option<MemOp>,

    out: OutPins,
}
impl Ppu {
    pub fn new() -> Self {
        Self {
            base_nametable_address: 0x2000,
            vram_address_increment: 1,
            sprite_pattern_table: 0,
            background_pattern_table: 0,
            wide_sprites: false,
            generate_nmi: false,

            greyscale: false,
            show_leftmost_background: true,
            show_leftmost_sprites: true,
            show_background: false,
            show_sprites: false,
            emphasize_red: false,
            emphasize_green: false,
            emphasize_blue: false,

            sprite_overflow: false,
            sprite_zero_hit: false,
            vblank: false,

            oam_memory: [0; 256],
            oam_address: 0,
            scroll: [0; 2],
            address: 0,

            pixel: [0; 2],
            odd_frame: false,

            cpu_service_pending: false,
            scheduled_mem_op: None,

            out: OutPins::init(),
        }
    }

    pub fn cycle(&mut self, pins: InPins) {
        self.out.ale = false;
        self.out.read_enable = false;
        self.out.write_enable = false;

        self.decide_nmi();
        self.perform_mem_op();
        self.service_cpu(pins);
        self.tick_pixel();

        self.cpu_service_pending |= pins.cpu_cycle;
    }
    fn service_cpu(&mut self, pins: InPins) {
        if !self.cpu_service_pending {
            return;
        }
        self.cpu_service_pending = false;
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
                let data = pins.cpu_data;

                self.base_nametable_address = 0x2000 + (data as u16 & 0b11) * 0x400;
                self.vram_address_increment = if data & 4 != 0 { 32 } else { 1 };
                self.sprite_pattern_table = if data & 8 != 0 { 0x1000 } else { 0 };
                self.background_pattern_table = if data & 16 != 0 { 0x1000 } else { 0 };
                self.wide_sprites = data & 32 != 0;
                self.generate_nmi = data & 128 != 0;
            }
            1 => {
                if pins.cpu_read {
                    return;
                }
                let data = pins.cpu_data;
                self.greyscale = data & 1 != 0;
                self.show_leftmost_background = data & 2 != 0;
                self.show_leftmost_sprites = data & 4 != 0;
                self.show_background = data & 8 != 0;
                self.show_sprites = data & 16 != 0;
                self.emphasize_red = data & 32 != 0;
                self.emphasize_green = data & 64 != 0;
                self.emphasize_blue = data & 128 != 0;
            }
            2 => {
                let overflow = (self.sprite_overflow as u8) << 5;
                let hit = (self.sprite_zero_hit as u8) << 6;
                let blank = (self.vblank as u8) << 7;

                self.out.cpu_data = Some(overflow | hit | blank);
                self.vblank = false;
                self.address = 0;
                self.scroll = [0; 2];
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

                self.scroll[0] = self.scroll[1];
                self.scroll[1] = pins.cpu_data;
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

                self.address += self.vram_address_increment;
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
        if self.pixel == NMI_START {
            self.vblank = true;
        } else if self.pixel == NMI_END {
            self.vblank = false;
        }

        self.out.nmi = self.generate_nmi && self.vblank;
    }
    fn tick_pixel(&mut self) {
        let max_x = if self.odd_frame {
            PIXELS_PER_LINE - 2
        } else {
            PIXELS_PER_LINE - 1
        };
        let max_y = LINES_PER_FRAME - 1;
        let final_pixel = [max_x, max_y];

        if self.pixel == final_pixel {
            self.pixel = [0, 0];
            self.odd_frame = !self.odd_frame;
        } else {
            self.pixel[0] += 1;
            if self.pixel[0] == PIXELS_PER_LINE {
                self.pixel[0] = 0;
                self.pixel[1] += 1;
            }
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

    pub fn out(&self) -> OutPins {
        self.out
    }
}

struct MemOp {
    address: u16,
    data: Option<u8>,
}

#[derive(Copy, Clone, Debug)]
pub struct InPins {
    pub cpu_cycle: bool,
    pub cpu_read: bool,
    pub cpu_address: u16,
    pub cpu_data: u8,
    pub mem_data: u8,
}
impl InPins {
    pub fn init() -> Self {
        Self {
            cpu_cycle: false,
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
