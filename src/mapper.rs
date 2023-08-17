use crate::nesbus::CpuBus;

pub mod nrom;

pub trait Mapper {
    fn cycle(&mut self, cpu: &mut CpuBus);
}
