use crate::nes::NesBus;

pub struct Dma {
    dma_cycle: u8,
    get_cycle: bool,
    last_m2: bool,

    status: Status,
    address_high: u8,
    address_low: u8,
}
impl Dma {
    pub fn new() -> Self {
        Dma {
            dma_cycle: 0,
            get_cycle: true,
            last_m2: false,
            status: Status::Idle,
            address_high: 0,
            address_low: 0,
        }
    }

    pub fn master_cycle(&mut self, bus: &mut NesBus) {
        self.service_cpu(bus);
        self.cycle(bus);
        self.tick();
    }
    fn tick(&mut self) {
        self.dma_cycle += 1;
        self.dma_cycle %= 12;
    }

    fn service_cpu(&mut self, bus: &mut NesBus) {
        let m2_edge = bus.cpu_m2 && self.last_m2 != bus.cpu_m2;
        if !m2_edge {
            return;
        }

        let address = bus.cpu_address as usize;
        if address != 0x4014 || bus.cpu_read {
            return;
        }

        self.address_high = bus.cpu_data;
        self.address_low = 0;
        bus.oam_dma_halts_cpu = true;
        self.status = Status::Begin;
    }

    fn cycle(&mut self, bus: &mut NesBus) {
        if !self.should_cycle() {
            return;
        }

        use Status::*;
        self.status = match self.status {
            Idle => Idle,
            Begin => {
                bus.oam_dma_halts_cpu = true;
                Initializing
            }
            Initializing => {
                if self.get_cycle && bus.cpu_halted {
                    bus.cpu_address = self.address();
                    bus.oam_dma_writes = false;
                    Reading
                } else {
                    Initializing
                }
            }
            Reading => {
                bus.cpu_address = 0x2004;
                bus.oam_dma_writes = true;
                if self.address_low == 0xFF {
                    Done
                } else {
                    Writing
                }
            }
            Writing => {
                self.address_low += 1;
                bus.cpu_address = self.address();
                bus.oam_dma_writes = false;
                Reading
            }
            Done => {
                bus.oam_dma_halts_cpu = false;
                bus.oam_dma_writes = false;
                Idle
            }
        };

        self.get_cycle = !self.get_cycle;
    }

    fn address(&self) -> u16 {
        let high = (self.address_high as u16) << 8;
        let low = self.address_low as u16;
        low | high
    }
    fn should_cycle(&self) -> bool {
        self.dma_cycle == 0
    }
}

enum Status {
    Idle,
    Begin,
    Initializing,
    Reading,
    Writing,
    Done,
}
