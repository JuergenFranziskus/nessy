pub mod nrom;

pub trait Mapper {
    fn master_cycle(&mut self, pins: InPins);

    fn out(&self) -> OutPins;
}

#[derive(Copy, Clone, Debug)]
pub struct InPins {
    pub cpu_m2: bool,
    pub cpu_address: u16,
    pub cpu_data: u8,
    pub cpu_read: bool,

    pub ppu_address: u16,
    pub ppu_data: u8,
    pub ppu_read_enable: bool,
    pub ppu_write_enable: bool,
}
impl InPins {
    pub fn init() -> Self {
        Self {
            cpu_m2: false,
            cpu_address: 0,
            cpu_data: 0,
            cpu_read: true,

            ppu_address: 0,
            ppu_data: 0,
            ppu_read_enable: false,
            ppu_write_enable: false,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct OutPins {
    pub cpu_data: Option<u8>,
    pub ppu_data: Option<u8>,
    pub irq: bool,
    pub ciram_a10: bool,
    pub ciram_ce: bool,
}
impl OutPins {
    pub fn init() -> OutPins {
        Self {
            cpu_data: None,
            ppu_data: None,
            irq: false,
            ciram_a10: false,
            ciram_ce: false,
        }
    }
}
