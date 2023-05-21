use crate::cpu::{Cpu, InPins as CPins};


pub struct Processor {
    cpu: Cpu,
    cpu_pins: CPins,

    out: OutPins,
}
impl Processor {
    pub fn new() -> Self {
        let (cpu, cpu_pins) = Cpu::new();
        Self {
            cpu,
            cpu_pins,
            out: OutPins::init(),
        }
    }

    pub fn cycle(&mut self, pins: InPins) {
        self.cpu_pins.data = pins.data;
        self.cpu_pins.reset = pins.reset;
        self.cpu_pins.irq = pins.irq;
        self.cpu_pins.nmi = pins.nmi;
        
        self.cpu.cycle(self.cpu_pins);

        self.out.address = self.cpu.out().address;
        self.out.data = self.cpu.out().data;
        self.out.read = self.cpu.out().read;
        self.out.sync = self.cpu.out().sync;
    }

    pub fn out(&self) -> OutPins {
        self.out
    }

    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }
}



#[derive(Copy, Clone, Debug)]
pub struct InPins {
    pub data: u8,
    pub reset: bool,
    pub irq: bool,
    pub nmi: bool,
}
impl InPins {
    pub fn init() -> Self {
        Self {
            data: 0,
            reset: false,
            irq: false,
            nmi: false,
        }
    }
}


#[derive(Copy, Clone, Debug)]
pub struct OutPins {
    pub address: u16,
    pub data: u8,
    pub read: bool,
    pub sync: bool,
}
impl OutPins {
    pub fn init() -> Self {
        Self {
            address: 0,
            data: 0,
            read: true,
            sync: false,
        }
    }
}
