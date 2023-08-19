use crate::nesbus::CpuBus;

pub struct Apu {
    dma: Dma,
}
impl Apu {
    pub fn init() -> Self {
        Self { dma: Dma::init() }
    }

    pub fn cycle(&mut self, cpu: &mut CpuBus) {
        self.dma.perform_dma(cpu);
        self.handle_cpu(cpu);
        self.dma.tick_counters();
    }

    fn handle_cpu(&mut self, cpu: &mut CpuBus) {
        match cpu.address() {
            0x4014 => {
                if cpu.read() {
                    return;
                };
                self.dma.start_oam_dma(cpu.data());
            }
            _ => (),
        }
    }
}

struct Dma {
    put_cycle: bool,

    oam_dma: bool,
    oam_data_valid: bool,
    oam_page: u8,
    oam_step: u8,
}
impl Dma {
    fn init() -> Self {
        Self {
            put_cycle: false,

            oam_dma: false,
            oam_data_valid: false,
            oam_page: 0,
            oam_step: 0,
        }
    }

    fn perform_dma(&mut self, cpu: &mut CpuBus) {
        cpu.set_not_ready(self.oam_dma);
        if !self.oam_dma {
            return;
        };
        if !cpu.halt() {
            return;
        };

        if self.put_cycle {
            if !self.oam_data_valid {
                return;
            };
            cpu.set_read(false);
            cpu.set_address(0x2004);
            self.oam_step = self.oam_step.wrapping_add(1);
            if self.oam_step == 0 {
                self.oam_dma = false;
            }
        } else {
            let addr = (self.oam_page as u16) << 8 | (self.oam_step as u16);
            cpu.set_read(true);
            cpu.set_address(addr);
            self.oam_data_valid = true;
        }
    }

    fn start_oam_dma(&mut self, page: u8) {
        self.oam_dma = true;
        self.oam_data_valid = false;
        self.oam_page = page;
        self.oam_step = 0;
    }

    fn tick_counters(&mut self) {
        self.put_cycle = !self.put_cycle
    }
}
