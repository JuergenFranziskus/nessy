use super::Mapper;
use crate::{cpu::CpuPins, rom::Rom};

pub struct NRom {
    prg: Vec<u8>,
    mirror_prg: bool,
}
impl NRom {
    pub fn new(rom: &Rom) -> Self {
        Self {
            prg: rom.prg_rom.to_vec(),
            mirror_prg: rom.prg_rom.len() >= 16384,
        }
    }

    fn handle_cpu(&self, cpu: &mut CpuPins) {
        let address = cpu.address() as usize;
        if address < 0x8000 {
            return;
        };
        let address = address - 0x8000;
        let address = if self.mirror_prg {
            address % 16384
        } else {
            address
        };

        if cpu.read() {
            cpu.set_data(self.prg[address]);
        }
    }

    pub fn overwrite(&mut self, address: usize, value: u8) {
        if address < 0x8000 {
            return;
        };
        let address = if self.mirror_prg {
            address % 16384
        } else {
            address
        };

        self.prg[address] = value;
    }
}
impl Mapper for NRom {
    fn cycle(&mut self, cpu: &mut crate::cpu::CpuPins) {
        self.handle_cpu(cpu);
    }
}
