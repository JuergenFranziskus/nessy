use super::Bus;
use super::Mapper;
use crate::apu::Bus as CpuBus;
use crate::ppu::Bus as PpuBus;
use crate::rom::Rom;

pub struct Mapper0 {
    rom: Rom,
}
impl Mapper0 {
    pub fn new(rom: Rom) -> Self {
        Self { rom }
    }

    fn handle_cpu(&mut self, cpu: &mut CpuBus) {
        let addr = cpu.addr as usize;
        if addr < 0x8000 {
            return;
        };
        let mut offset = addr - 0x8000;
        let rom = &self.rom.bytes[self.rom.prg_rom.clone()];
        while offset >= rom.len() {
            offset -= rom.len();
        }

        cpu.data = rom[offset];
    }
    fn handle_ppu(&mut self, bus: &mut Bus, ppu: &mut PpuBus) {
        bus.set_ciram_ce(false);
        let addr = ppu.addr as usize;

        if addr < 0x2000 {
            let offset = addr as usize;
            if ppu.rd() {
                ppu.data = self.rom.bytes[self.rom.chr_rom.clone()][offset];
            } else if ppu.wr() {
            }
        } else if addr < 0x3000 {
            bus.set_ciram_ce(true);
            let a_10 = addr & 0x400 != 0;
            let a_11 = addr & 0x800 != 0;
            if self.rom.header.vertical_mirroring {
                bus.set_ciram_a10(a_10);
            } else {
                bus.set_ciram_a10(a_11);
            }
        }
    }
}
impl Mapper for Mapper0 {
    fn clock_with_cpu(&mut self, bus: &mut Bus, cpu: &mut CpuBus, ppu: &mut PpuBus) {
        self.handle_cpu(cpu);
        self.handle_ppu(bus, ppu);
    }

    fn clock_with_ppu(&mut self, bus: &mut Bus, ppu: &mut PpuBus) {
        self.handle_ppu(bus, ppu);
    }
}
