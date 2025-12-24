use m6502::Bus as CpuBus;
use m6502::M6502;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Bus {
    pub addr: u16,
    pub data: u8,
    flags: u16,
}
impl Bus {
    pub fn new() -> Self {
        Self {
            addr: 0,
            data: 0,
            flags: 0,
        }
    }

    pub fn irq(self) -> bool {
        self.flags & Self::IRQ != 0
    }
    pub fn nmi(self) -> bool {
        self.flags & Self::NMI != 0
    }
    pub fn oe1(self) -> bool {
        self.flags & Self::OE1 != 0
    }
    pub fn oe2(self) -> bool {
        self.flags & Self::OE2 != 0
    }
    pub fn out0(self) -> bool {
        self.flags & Self::OUT0 != 0
    }
    pub fn out1(self) -> bool {
        self.flags & Self::OUT1 != 0
    }
    pub fn out2(self) -> bool {
        self.flags & Self::OUT2 != 0
    }
    pub fn rw(self) -> bool {
        self.flags & Self::RW != 0
    }
    pub fn sync(self) -> bool {
        self.flags & Self::SYNC != 0
    }

    pub fn set_irq(&mut self, to: bool) {
        self.flags &= !Self::IRQ;
        if to {
            self.flags |= Self::IRQ;
        }
    }
    pub fn set_nmi(&mut self, to: bool) {
        self.flags &= !Self::NMI;
        if to {
            self.flags |= Self::NMI;
        }
    }
    pub fn set_oe1(&mut self, to: bool) {
        self.flags &= !Self::OE1;
        if to {
            self.flags |= Self::OE1;
        }
    }
    pub fn set_oe2(&mut self, to: bool) {
        self.flags &= !Self::OE2;
        if to {
            self.flags |= Self::OE2;
        }
    }
    pub fn set_out0(&mut self, to: bool) {
        self.flags &= !Self::OUT0;
        if to {
            self.flags |= Self::OUT0;
        }
    }
    pub fn set_out1(&mut self, to: bool) {
        self.flags &= !Self::OUT1;
        if to {
            self.flags |= Self::OUT1;
        }
    }
    pub fn set_out2(&mut self, to: bool) {
        self.flags &= !Self::OUT2;
        if to {
            self.flags |= Self::OUT2;
        }
    }
    pub fn set_rw(&mut self, to: bool) {
        self.flags &= !Self::RW;
        if to {
            self.flags |= Self::RW;
        }
    }
    pub fn set_sync(&mut self, to: bool) {
        self.flags &= !Self::SYNC;
        if to {
            self.flags |= Self::SYNC;
        }
    }

    const IRQ: u16 = 4;
    const NMI: u16 = 8;
    const OE1: u16 = 16;
    const OE2: u16 = 32;
    const OUT0: u16 = 64;
    const OUT1: u16 = 128;
    const OUT2: u16 = 256;
    const RW: u16 = 512;
    const SYNC: u16 = 1024;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Apu {
    cpu: M6502,
    cpu_bus: CpuBus,

    put_cycle: bool,
    oam_dma: Dma,
    oam_bank: u8,
    oam_cycle: u8,
}
impl Apu {
    pub fn start() -> Self {
        Self {
            cpu: M6502::start(),
            cpu_bus: CpuBus::new(),

            put_cycle: false,
            oam_dma: Dma::Idle,
            oam_bank: 0,
            oam_cycle: 0,
        }
    }

    pub fn cpu(self) -> M6502 {
        self.cpu
    }

    pub fn clock(&mut self, bus: &mut Bus) {
        self.clock_cpu(bus);

        self.handle_cpu(bus);

        self.put_cycle = !self.put_cycle;
    }
    fn clock_cpu(&mut self, bus: &mut Bus) {
        self.sync_cpu_bus(bus);

        match self.oam_dma {
            Dma::Idle => {
                self.cpu.clock(&mut self.cpu_bus);
                self.sync_apu_bus(bus);
            }
            Dma::Halt => {
                self.cpu.clock(&mut self.cpu_bus);
                if self.cpu_bus.rw() {
                    if self.put_cycle {
                        self.oam_dma = Dma::Get;
                    } else {
                        self.oam_dma = Dma::Align;
                    }
                }
                self.sync_apu_bus(bus);
            }
            Dma::Align => {
                if self.put_cycle {
                    self.oam_dma = Dma::Get;
                }
            }
            Dma::Get => {
                bus.addr = (self.oam_bank as u16) << 8 | (self.oam_cycle) as u16;
                bus.set_rw(true);
                bus.set_sync(false);
                self.oam_dma = Dma::Put;
            }
            Dma::Put => {
                bus.addr = 0x2004;
                bus.set_rw(false);
                bus.set_sync(false);
                let end;
                (self.oam_cycle, end) = self.oam_cycle.overflowing_add(1);
                if end {
                    self.oam_dma = Dma::End
                } else {
                    self.oam_dma = Dma::Get;
                }
            }
            Dma::End => {
                self.sync_apu_bus(bus);
                self.oam_dma = Dma::Idle;
            }
        }
    }
    fn sync_cpu_bus(&mut self, bus: &Bus) {
        self.cpu_bus.data = bus.data;
        self.cpu_bus.set_irq(bus.irq());
        self.cpu_bus.set_nmi(bus.nmi());
    }
    fn sync_apu_bus(&self, bus: &mut Bus) {
        bus.addr = self.cpu_bus.addr;
        bus.data = self.cpu_bus.data;
        bus.set_rw(self.cpu_bus.rw());
        bus.set_sync(self.cpu_bus.sync());
    }

    fn handle_cpu(&mut self, _bus: &mut Bus) {
        match self.cpu_bus.addr {
            0x4014 if !self.cpu_bus.rw() => {
                self.oam_bank = self.cpu_bus.data;
                self.oam_cycle = 0;
                self.oam_dma = Dma::Halt;
            }
            _ => (),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Dma {
    Idle,
    Halt,
    Align,
    Get,
    Put,
    End,
}
