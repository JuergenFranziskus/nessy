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

    oam_dma: OamDma,
    oam_page: u8,
    oam_step: u8,
}
impl Dma {
    fn init() -> Self {
        Self {
            put_cycle: false,

            oam_dma: OamDma::Idle,
            oam_page: 0,
            oam_step: 0,
        }
    }

    fn perform_dma(&mut self, cpu: &mut CpuBus) {
        match self.oam_dma {
            OamDma::Idle => cpu.set_not_ready(false),
            OamDma::Started => {
                cpu.set_not_ready(true);
                if !cpu.halt() {
                    return;
                };
                if self.put_cycle {
                    return;
                };
                cpu.set_read(true);
                cpu.set_address(self.oam_addr());
                self.oam_dma = OamDma::ToWrite;
            }
            OamDma::ToWrite => {
                cpu.set_read(false);
                cpu.set_address(0x2004);
                let done = self.oam_step == 255;
                self.oam_dma = if done { OamDma::Idle } else { OamDma::ToRead };
                self.oam_step = self.oam_step.wrapping_add(1);
            }
            OamDma::ToRead => {
                cpu.set_address(self.oam_addr());
                cpu.set_read(true);
                self.oam_dma = OamDma::ToWrite;
            }
        }
    }

    fn start_oam_dma(&mut self, page: u8) {
        self.oam_dma = OamDma::Started;
        self.oam_page = page;
        self.oam_step = 0;
    }

    fn tick_counters(&mut self) {
        self.put_cycle = !self.put_cycle
    }

    fn oam_addr(&self) -> u16 {
        let low = self.oam_step as u16;
        let high = (self.oam_page as u16) << 8;
        low | high
    }
}

enum OamDma {
    Idle,
    Started,
    ToRead,
    ToWrite,
}
