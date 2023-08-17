use super::Mapper;
use crate::{nesbus::CpuBus, rom::Rom};

pub struct NRom {
    prg: Vec<u8>,
    _chr: Vec<u8>,
    large_prg: bool,
}
impl NRom {
    pub fn new(rom: &Rom) -> Self {
        let large_prg = rom.prg_rom.len() > 0x4000;
        Self {
            prg: rom.prg_rom.to_vec(),
            _chr: rom.chr_rom.to_vec(),
            large_prg,
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

    pub fn overwrite(&mut self, addr: u16, value: u8) {
        if addr < 0x8000 {
            return;
        };
        let addr = addr % if self.large_prg { 0x8000 } else { 0x4000 };
        self.prg[addr as usize] = value;
    }
}
impl Mapper for NRom {
    fn cycle(&mut self, cpu: &mut CpuBus) {
        self.handle_cpu(cpu);
    }
}
