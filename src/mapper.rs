use crate::cpu::CpuPins;

pub mod nrom;

pub trait Mapper {
    fn cycle(&mut self, cpu: &mut CpuPins);
}
