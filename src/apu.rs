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

    pub fn ad1(self) -> bool {
        self.flags & Self::AD1 != 0
    }
    pub fn ad2(self) -> bool {
        self.flags & Self::AD2 != 0
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
    pub fn set_ad1(&mut self, to: bool) {
        self.flags &= !Self::AD1;
        if to {
            self.flags |= Self::AD1;
        }
    }
    pub fn set_ad2(&mut self, to: bool) {
        self.flags &= !Self::AD2;
        if to {
            self.flags |= Self::AD2;
        }
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

    const AD1: u16 = 1;
    const AD2: u16 = 2;
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
}
impl Apu {
    pub fn start() -> Self {
        Self {
            cpu: M6502::start(),
            cpu_bus: CpuBus::new(),
        }
    }

    pub fn cpu(self) -> M6502 {
        self.cpu
    }

    pub fn clock(&mut self, bus: &mut Bus) {
        self.cpu_bus.data = bus.data;
        self.cpu_bus.set_irq(bus.irq());
        self.cpu_bus.set_nmi(bus.nmi());

        self.cpu.clock(&mut self.cpu_bus);

        bus.addr = self.cpu_bus.addr;
        bus.data = self.cpu_bus.data;
        bus.set_rw(self.cpu_bus.rw());
        bus.set_sync(self.cpu_bus.sync());
    }
}
