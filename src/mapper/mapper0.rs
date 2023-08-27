use super::{Mapper, MapperBus};
use crate::{nesbus::CpuBus, ppu::PpuBus, rom::Rom};

pub struct Mapper0 {
    prg: Vec<u8>,
    chr: Vec<u8>,
    large_prg: bool,
    vertical_mirror: bool,
}
impl Mapper0 {
    pub fn new(rom: &Rom) -> Self {
        let large_prg = rom.prg_rom.len() > 0x4000;
        Self {
            prg: rom.prg_rom.to_vec(),
            chr: rom.chr_rom.to_vec(),
            large_prg,
            vertical_mirror: rom.header.vertical_mirroring,
        }
    }

    fn handle_cpu(&mut self, cpu: &mut CpuBus) {
        let addr = cpu.address() as usize;
        if addr < 0x8000 {
            return;
        };
        let addr = addr % 0x8000;
        let addr = if self.large_prg { addr } else { addr % 0x4000 };

        if cpu.read() {
            cpu.set_data(self.prg[addr]);
        }
    }
    fn handle_ppu(&mut self, bus: &mut MapperBus, ppu: &mut PpuBus) {
        if ppu.address() < 0x2000 && ppu.read_enable() {
            ppu.set_data(self.chr[ppu.address() as usize]);
        }

        let a10 = ppu.address() >> 10 & 1 != 0;
        let a11 = ppu.address() >> 11 & 1 != 0;
        bus.set_vram_a10(if self.vertical_mirror { a10 } else { a11 });
        let enable = (0x2000..0x3000).contains(&ppu.address());

        bus.set_vram_enable(enable);
    }

    pub fn overwrite(&mut self, addr: u16, value: u8) {
        if addr < 0x8000 {
            return;
        };
        let addr = addr % if self.large_prg { 0x8000 } else { 0x4000 };
        self.prg[addr as usize] = value;
    }
}
impl Mapper for Mapper0 {
    fn cycle(&mut self, bus: &mut super::MapperBus, cpu: &mut CpuBus, ppu: &mut PpuBus) {
        self.handle_cpu(cpu);
        self.handle_ppu(bus, ppu);
    }

    fn cycle_with_ppu(&mut self, bus: &mut super::MapperBus, ppu: &mut PpuBus) {
        self.handle_ppu(bus, ppu);
    }
}
