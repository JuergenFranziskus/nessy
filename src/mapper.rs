use crate::nes::NesBus;

pub mod nrom;

pub trait Mapper {
    fn master_cycle(&mut self, bus: &mut NesBus, cycle: u64);
}
