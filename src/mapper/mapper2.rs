use super::{Mapper, MapperBus};
use crate::{nesbus::CpuBus, ppu::PpuBus};
use nes_rom_parser::Rom;

pub struct Mapper2 {
    prg: Vec<u8>,
    chr: Vec<u8>,
    banks: u8,
    bank: u8,
    vertical_mirror: bool,
    bank_conflicts: bool,
}
impl Mapper2 {
    pub fn new(rom: &Rom) -> Self {
        let banks = rom.header.prg_rom_size / 16384;
        let bank = 0;

        let bank_conflicts = rom.header.submapper == 2;

        let ret = Self {
            prg: rom.prg_rom.to_vec(),
            chr: vec![0; rom.header.chr_ram_size as usize],
            banks: banks as u8,
            bank,
            vertical_mirror: rom.header.vertical_mirroring,
            bank_conflicts,
        };

        ret
    }

    fn handle_cpu(&mut self, cpu: &mut CpuBus) {
        if !(0x8000..).contains(&cpu.address()) {
            return;
        };
        if cpu.read() {
            let addr = self.prg_rom_address(cpu.address());
            cpu.set_data(self.prg[addr]);
        } else {
            if self.bank_conflicts {
                let addr = self.prg_rom_address(cpu.address());
                let rom_data = self.prg[addr];
                self.bank = rom_data & cpu.data();
            } else {
                self.bank = cpu.data();
            }
        }
    }
    fn handle_ppu(&mut self, bus: &mut MapperBus, ppu: &mut PpuBus) {
        if ppu.address() < 0x2000 {
            if ppu.read_enable() {
                ppu.set_data(self.chr[ppu.address() as usize]);
            }
            if ppu.write_enable() {
                self.chr[ppu.address() as usize] = ppu.data();
            }
        }

        let a10 = ppu.address() >> 10 & 1 != 0;
        let a11 = ppu.address() >> 11 & 1 != 0;
        bus.set_vram_a10(if self.vertical_mirror { a10 } else { a11 });
        let enable = (0x2000..0x3000).contains(&ppu.address());

        bus.set_vram_enable(enable);
    }

    fn prg_rom_address(&self, addr: u16) -> usize {
        let addr = addr as usize;
        let high = addr > 0xC000;
        let offset = addr % 16384;

        let bank = if high { self.banks - 1 } else { self.bank };
        let bank_start = (bank as usize) * 16384;

        bank_start | offset
    }
}
impl Mapper for Mapper2 {
    fn cycle(&mut self, bus: &mut MapperBus, cpu: &mut CpuBus, ppu: &mut PpuBus) {
        self.handle_cpu(cpu);
        self.handle_ppu(bus, ppu);
    }

    fn cycle_with_ppu(&mut self, bus: &mut MapperBus, ppu: &mut PpuBus) {
        self.handle_ppu(bus, ppu);
    }
}
