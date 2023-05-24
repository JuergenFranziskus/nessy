use self::{
    apu::{AInPins, Apu},
    dma::OamDma,
};
use crate::cpu::{Cpu, InPins as CPins};
use dma::InPins as DPins;

mod apu;
mod dma;

pub struct Processor {
    cpu_cycle: u8,

    cpu: Cpu,
    apu: Apu,
    dma: OamDma,

    cpu_pins: CPins,
    apu_pins: AInPins,
    dma_pins: DPins,

    data_bus: u8,
    address_bus: u16,

    out: OutPins,
}
impl Processor {
    pub fn new() -> Self {
        let (cpu, cpu_pins) = Cpu::new();
        Self {
            cpu_cycle: 0,
            cpu,
            cpu_pins,
            apu: Apu::init(),
            apu_pins: AInPins::init(),
            dma: OamDma::init(),
            dma_pins: DPins::init(),
            data_bus: 0,
            address_bus: 0,
            out: OutPins::init(),
        }
    }

    pub fn master_cycle(&mut self, pins: InPins) {
        self.out.m2 = self.cpu_cycle >= 6;

        self.update_pins(pins);

        if self.should_cycle_cpu() {
            self.cpu.cycle(self.cpu_pins);
        }
        self.apu.master_cycle(self.apu_pins);
        self.dma.master_cycle(self.dma_pins);

        self.update_busses(pins);
        self.update_out_pins();
        self.tick_counters();
    }
    fn should_cycle_cpu(&self) -> bool {
        self.cpu_cycle == 0
    }
    fn tick_counters(&mut self) {
        self.cpu_cycle += 1;
        self.cpu_cycle %= 12;
    }
    fn update_pins(&mut self, pins: InPins) {
        self.apu_pins.m2 = self.out.m2;
        self.apu_pins.address = self.address_bus;
        self.apu_pins.data = self.data_bus;
        self.apu_pins.read = self.out.read;

        self.cpu_pins.data = self.data_bus;
        self.cpu_pins.nmi = pins.nmi;
        self.cpu_pins.irq = pins.irq | self.apu.out().irq;
        self.cpu_pins.ready = !self.dma.out().halt_cpu;

        self.dma_pins.address = self.address_bus;
        self.dma_pins.data = self.data_bus;
        self.dma_pins.cpu_halted = self.cpu.out().halted;
        self.dma_pins.m2 = self.out.m2;
        self.dma_pins.read = self.cpu.out().read;
    }
    fn update_busses(&mut self, pins: InPins) {
        let cpu_out = self.cpu.out();
        let apu_out = self.apu.out();
        let dma_out = self.dma.out();

        if !cpu_out.halted && !cpu_out.read {
            self.data_bus = cpu_out.data;
        } else if let Some(data) = apu_out.data {
            self.data_bus = data;
        } else if let Some(data) = dma_out.data {
            self.data_bus = data;
        } else {
            self.data_bus = pins.data;
        }

        if let Some(address) = dma_out.address {
            self.address_bus = address;
        } else {
            self.address_bus = cpu_out.address;
        }
    }
    fn update_out_pins(&mut self) {
        self.out.data = self.data_bus;
        self.out.read = self.cpu.out().read && self.dma.out().read;
        self.out.sync = self.cpu.out().sync;
        self.out.address = self.address_bus;
    }

    pub fn out(&self) -> OutPins {
        self.out
    }

    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }

    pub fn cpu_cycle(&self) -> u8 {
        self.cpu_cycle
    }

    pub fn cpu_pins(&self) -> CPins {
        self.cpu_pins
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
