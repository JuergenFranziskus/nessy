use super::apu::Bus as CpuBus;
use super::ppu::Bus as PpuBus;

pub mod mapper0;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Bus {
    flags: u8,
}
impl Bus {
    pub fn new() -> Self {
        Self { flags: 0 }
    }

    pub fn ciram_a10(self) -> bool {
        self.flags & Self::CIRAM_A10 != 0
    }
    pub fn ciram_ce(self) -> bool {
        self.flags & Self::CIRAM_CE != 0
    }

    pub fn set_ciram_a10(&mut self, to: bool) {
        self.flags &= !Self::CIRAM_A10;
        if to {
            self.flags |= Self::CIRAM_A10;
        }
    }
    pub fn set_ciram_ce(&mut self, to: bool) {
        self.flags &= !Self::CIRAM_CE;
        if to {
            self.flags |= Self::CIRAM_CE;
        }
    }

    const CIRAM_A10: u8 = 1;
    const CIRAM_CE: u8 = 2;
}

pub trait Mapper {
    fn clock_with_cpu(&mut self, bus: &mut Bus, cpu: &mut CpuBus, ppu: &mut PpuBus);
    fn clock_with_ppu(&mut self, bus: &mut Bus, ppu: &mut PpuBus);
}
