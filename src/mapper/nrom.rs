#![allow(dead_code)]
use super::{InPins, Mapper, OutPins};
use crate::rom::Mirroring;

/// The NROM mapper, #0 according to INES designation.
pub struct NRom {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    mirror: Mirroring,

    out: OutPins,
}

impl Mapper for NRom {
    fn master_cycle(&mut self, pins: InPins) {
        self.service_cpu(pins);
        self.service_ppu(pins);
    }

    fn out(&self) -> OutPins {
        self.out
    }
}
impl NRom {
    pub fn new(prg_rom: Vec<u8>, chr_rom: Vec<u8>, mirror: Mirroring) -> Self {
        Self {
            prg_rom,
            chr_rom,
            mirror,

            out: OutPins::init(),
        }
    }

    fn service_cpu(&mut self, pins: InPins) {
        self.out.cpu_data = None;

        let address = pins.cpu_address as usize;
        if address >= 0x8000 {
            let mut address = address - 0x8000;
            if address >= self.prg_rom.len() {
                address &= 0x3FFF
            }
            let address = address;

            if pins.cpu_read {
                self.out.cpu_data = Some(self.prg_rom[address]);
            }
        }
    }
    fn service_ppu(&mut self, pins: InPins) {
        self.out.ppu_data = None;

        let address = pins.ppu_address as usize;
        if address < 0x2000 && pins.ppu_read_enable {
            self.out.ppu_data = Some(self.chr_rom[address]);
        }

        let a10 = address & (1 << 10) != 0;
        let a11 = address & (1 << 11) != 0;

        self.out.ciram_a10 = match self.mirror {
            Mirroring::Horizontal => a11,
            Mirroring::Vertical => a10,
        };
        self.out.ciram_ce = (0x2000..0x3EFF).contains(&address);
    }
}
