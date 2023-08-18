use crate::{
    nesbus::CpuBus,
    util::{get_flag_u16, get_flag_u8, set_flag_u16, set_flag_u8},
};

const WIDTH: u16 = 241;
const HEIGHT: u16 = 262;

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
        }
    }

    pub fn cycle(&mut self, bus: &mut PpuBus, cpu: &mut CpuBus) {
        self.common_cycle(bus);
        self.handle_cpu(bus, cpu);
    }
    pub fn cycle_alone(&mut self, bus: &mut PpuBus) {
        self.common_cycle(bus);
    }

    fn common_cycle(&mut self, bus: &mut PpuBus) {
        self.update_data_latch(bus); // The order is important here
        self.perform_memop(bus);

        // TODO: produce a pixel of video output

        self.decide_vblank(bus);
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
    fn decide_vblank(&mut self, bus: &mut PpuBus) {
        let start = [1, 241];
        let end = [1, 261];

        if self.dot == start {
            self.meta.set_vblank(true);
        } else if self.dot == end {
            self.meta.set_vblank(false);
        }

        bus.set_nmi(self.meta.vblank() && self.control.nmi_enable());
    }
    fn tick_counter(&mut self) {
        let last = if self.meta.odd_frame() {
            [WIDTH - 2, HEIGHT - 1]
        } else {
            [WIDTH - 1, HEIGHT - 1]
        };
        if self.dot == last {
            self.dot = [0, 0];
            self.meta.set_odd_frame(!self.meta.odd_frame());
        } else {
            self.dot[0] += 1;
            if self.dot[0] == WIDTH {
                self.dot[0] = 0;
                self.dot[1] += 1;
            }
        }
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
                } else {
                    self.t.0 &= !0xFF;
                    self.t.0 |= data as u16;
                    self.v = self.t;
                    self.meta.set_w(false);
                }
            }
            7 => {
                let palette = is_palette_address(addr);
                let palette_index = normalize_palette_address(addr);
                if cpu.read() {
                    self.read(self.v.0, bus);
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
                        self.write(addr, cpu.data(), bus);
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
        self.v.0 &= 0x4000;
    }

    pub fn dot(&self) -> [u16; 2] {
        self.dot
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
pub struct Meta(u16);
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

    pub fn sprite_overflow(self) -> bool {
        self.get_flag(Self::SPRITE_OVERFLOW)
    }
    pub fn sprite_zero_hit(self) -> bool {
        self.get_flag(Self::SPRITE_ZERO_HIT)
    }
    pub fn vblank(self) -> bool {
        self.get_flag(Self::VBLANK)
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
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Control(u8);
impl Control {
    pub fn init() -> Self {
        Self(0)
    }

    pub fn nmi_enable(self) -> bool {
        get_flag_u8(self.0, Self::NMI_ENABLE)
    }

    pub fn inc_amount(self) -> u16 {
        todo!()
    }

    const NMI_ENABLE: u8 = 7;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Mask(u8);
impl Mask {
    pub fn init() -> Self {
        Self(0)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct V(u16);
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
    pub fn nametable(self) -> u8 {
        (self.0 >> Self::NAMETABLE) as u8 & 0b11
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

    const COARSE_X: u16 = 0;
    const COARSE_Y: u16 = 5;
    const NAMETABLE: u16 = 10;
    const FINE_Y: u16 = 12;
}

fn is_palette_address(addr: u16) -> bool {
    (0x3F00..0x4000).contains(&addr)
}
fn normalize_palette_address(addr: u16) -> usize {
    let addr = (addr as usize & 0xFF) % 0x20;
    match addr {
        0x10 => 0x00,
        0x14 => 0x04,
        0x18 => 0x08,
        0x1C => 0x0C,
        _ => addr,
    }
}
