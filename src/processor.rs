use crate::cpu::{Cpu, InPins as CPins};

pub struct Processor {
    master_cycle: u8,
    cpu: Cpu,
    cpu_pins: CPins,

    out: OutPins,
}
impl Processor {
    pub fn new() -> Self {
        let (cpu, cpu_pins) = Cpu::new();
        Self {
            master_cycle: 0,
            cpu,
            cpu_pins,
            out: OutPins::init(),
        }
    }

    pub fn master_cycle(&mut self, pins: InPins) {
        if self.should_cycle_cpu() {
            self.cpu_cycle(pins);
        }

        self.tick_counter();

        self.out.m2 = self.master_cycle >= 6; // Phi-2 is high for the second half of a cpu cycle
    }
    fn should_cycle_cpu(&self) -> bool {
        self.master_cycle == 0
    }
    fn tick_counter(&mut self) {
        self.master_cycle += 1;
        self.master_cycle %= 12;
    }
    fn cpu_cycle(&mut self, pins: InPins) {
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
    pub m2: bool,
    pub address: u16,
    pub data: u8,
    pub read: bool,
    pub sync: bool,
}
impl OutPins {
    pub fn init() -> Self {
        Self {
            m2: false,
            address: 0,
            data: 0,
            read: true,
            sync: false,
        }
    }
}
