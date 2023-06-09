#![allow(dead_code)]
use super::Mapper;
use crate::{
    nes::NesBus,
    rom::{Mirroring, Rom},
};

/// The NROM mapper, #0 according to INES designation.
pub struct NRom {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    mirror: Mirroring,

    last_m2: bool,
    last_read: bool,
    last_write: bool,
}

impl Mapper for NRom {
    fn master_cycle(&mut self, bus: &mut NesBus, _cycle: u64) {
        self.service_cpu(bus);
        self.service_ppu(bus);

        self.last_m2 = bus.cpu_m2;
        self.last_read = bus.ppu_read_enable;
        self.last_write = bus.ppu_write_enable;
    }
}
impl NRom {
    pub fn new(rom: &Rom) -> Self {
        let prg_rom = rom.prg_rom.clone();
        let chr_rom = rom.chr_rom.clone();
        let mirror = rom.header.mirroring;

        Self {
            prg_rom,
            chr_rom,
            mirror,

            last_m2: false,
            last_read: false,
            last_write: false,
        }
    }

    fn service_cpu(&mut self, bus: &mut NesBus) {
        let m2_edge = bus.cpu_m2 && self.last_m2 != bus.cpu_m2;
        if !m2_edge {
            return;
        }

        let address = bus.cpu_address as usize;
        if address >= 0x8000 {
            let mut address = address - 0x8000;
            if address >= self.prg_rom.len() {
                address &= 0x3FFF
            }
            let address = address;

            if bus.everyone_reads_cpu_bus() {
                bus.cpu_data = self.prg_rom[address];
            }
        }
    }
    fn service_ppu(&mut self, bus: &mut NesBus) {
        if !self.ppu_edge(bus) {
            return;
        }

        let address = bus.ppu_address as usize;
        if address < 0x2000 && bus.ppu_read_enable {
            bus.ppu_data = self.chr_rom[address];
        }

        let a10 = address & (1 << 10) != 0;
        let a11 = address & (1 << 11) != 0;

        bus.map_ciram_a10 = match self.mirror {
            Mirroring::Horizontal => a11,
            Mirroring::Vertical => a10,
        };
        bus.map_ciram_enable = (0x2000..0x3EFF).contains(&address);
    }

    fn ppu_edge(&self, bus: &NesBus) -> bool {
        let read = bus.ppu_read_enable;
        let write = bus.ppu_write_enable;
        let read_edge = read != self.last_read && read;
        let write_edge = write != self.last_write && write;
        read_edge | write_edge
    }
}
