use self::{mapper0::Mapper0, mapper2::Mapper2};
use crate::{
    nesbus::CpuBus,
    ppu::PpuBus,
    util::{get_flag_u8, set_flag_u8},
};
use nes_rom_parser::Rom;

pub mod mapper0;
pub mod mapper2;

pub trait Mapper {
    fn cycle(&mut self, bus: &mut MapperBus, cpu: &mut CpuBus, ppu: &mut PpuBus);
    fn cycle_with_ppu(&mut self, bus: &mut MapperBus, ppu: &mut PpuBus);
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct MapperBus {
    flags: u8,
}
impl MapperBus {
    pub fn init() -> Self {
        Self { flags: 0 }
    }

    fn get_flag(self, flag: u8) -> bool {
        get_flag_u8(self.flags, flag)
    }
    fn set_flag(&mut self, flag: u8, val: bool) {
        set_flag_u8(&mut self.flags, flag, val);
    }

    pub fn vram_enable(self) -> bool {
        self.get_flag(Self::VRAM_ENABLE)
    }
    pub fn set_vram_enable(&mut self, enable: bool) {
        self.set_flag(Self::VRAM_ENABLE, enable)
    }
    pub fn vram_a10(self) -> bool {
        self.get_flag(Self::VRAM_A10)
    }
    pub fn set_vram_a10(&mut self, a10: bool) {
        self.set_flag(Self::VRAM_A10, a10)
    }
    pub fn irq(self) -> bool {
        self.get_flag(Self::IRQ)
    }
    pub fn set_irq(&mut self, irq: bool) {
        self.set_flag(Self::IRQ, irq)
    }

    const VRAM_ENABLE: u8 = 0;
    const VRAM_A10: u8 = 1;
    const IRQ: u8 = 2;
}

pub struct DynMapper(Box<dyn Mapper + Send>);
impl DynMapper {
    pub fn new(mapper: impl Mapper + Send + 'static) -> Self {
        Self(Box::new(mapper))
    }
}
impl Mapper for DynMapper {
    fn cycle(&mut self, bus: &mut MapperBus, cpu: &mut CpuBus, ppu: &mut PpuBus) {
        self.0.cycle(bus, cpu, ppu);
    }

    fn cycle_with_ppu(&mut self, bus: &mut MapperBus, ppu: &mut PpuBus) {
        self.0.cycle_with_ppu(bus, ppu);
    }
}

pub fn get_mapper(rom: &Rom) -> DynMapper {
    let mapper = rom.header.mapper;
    match rom.header.mapper {
        0 => DynMapper::new(Mapper0::new(rom)),
        2 => DynMapper::new(Mapper2::new(rom)),
        _ => unimplemented!("Mapper {mapper} is not implemented"),
    }
}
