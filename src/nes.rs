use crate::{cpu::Cpu, cpu::InPins as CPins, mapper::InPins as MPins, mapper::Mapper};

pub struct Nes<M> {
    cpu: Cpu,
    mapper: M,

    cpu_cycles: usize,

    cpu_pins: CPins,
    mapper_pins: MPins,

    busses: Busses,
    memory: [u8; 2048],
}
impl<M> Nes<M> {
    pub fn new(mapper: M) -> Self {
        let (cpu, cpu_pins) = Cpu::new();
        let mapper_pins = MPins::init();
        Self {
            cpu,
            mapper,

            cpu_cycles: 0,

            cpu_pins,
            mapper_pins,

            busses: Busses::init(),
            memory: [0; 2048],
        }
    }

    pub fn cpu_cycles(&self) -> usize {
        self.cpu_cycles
    }
    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }
    pub fn cpu_pins(&self) -> CPins {
        self.cpu_pins
    }
}
impl<M: Mapper> Nes<M> {
    pub fn master_cycle(&mut self) {
        self.cycle_devices();
        self.update_busses();
        self.update_cpu_memory();
        self.tick_counters();
    }
    fn cycle_devices(&mut self) {
        if self.cpu_should_cycle() {
            self.cycle_cpu();
        }

        self.cycle_mapper();
    }
    fn cycle_cpu(&mut self) {
        self.update_cpu_pins();
        self.cpu.cycle(self.cpu_pins);
    }
    fn update_cpu_pins(&mut self) {
        self.cpu_pins.data = self.busses.cpu_data;
        self.cpu_pins.irq = self.mapper.out().irq;
    }
    fn cycle_mapper(&mut self) {
        self.update_mapper_pins();
        self.mapper.cycle(self.mapper_pins);
    }
    fn update_mapper_pins(&mut self) {
        let cpu_out = self.cpu.out();

        self.mapper_pins.cpu_address = cpu_out.address;
        self.mapper_pins.cpu_data = self.busses.cpu_data;
        self.mapper_pins.cpu_read = cpu_out.read;
        self.mapper_pins.cpu_cycle = self.cpu_should_cycle();
    }
    fn update_busses(&mut self) {
        self.update_cpu_address_bus();
        self.update_cpu_data_bus();
    }
    fn update_cpu_address_bus(&mut self) {
        let cpu_out = self.cpu.out();

        self.busses.cpu_address = 0;
        if !cpu_out.halted {
            self.busses.cpu_address |= cpu_out.address;
        }
    }
    fn update_cpu_data_bus(&mut self) {
        let cpu_out = self.cpu.out();
        let map_out = self.mapper.out();

        self.busses.cpu_data = 0;
        if !cpu_out.read {
            self.busses.cpu_data |= cpu_out.data;
        }
        if let Some(data) = map_out.cpu_data {
            self.busses.cpu_data |= data;
        }
    }
    fn update_cpu_memory(&mut self) {
        let cpu_out = self.cpu.out();

        let address = cpu_out.address as usize;
        if address < 0x2000 {
            let address = address % 0x800;

            if cpu_out.read {
                self.busses.cpu_data |= self.memory[address];
            } else {
                self.memory[address] = self.busses.cpu_data;
            }
        }
    }
    fn tick_counters(&mut self) {
        self.cpu_cycles += 1;
        self.cpu_cycles %= 12;
    }

    fn cpu_should_cycle(&self) -> bool {
        self.cpu_cycles == 0
    }

    pub fn force_update_pins(&mut self) {
        self.update_cpu_pins();
        self.update_mapper_pins();
    }
}

struct Busses {
    cpu_address: u16,
    cpu_data: u8,
}
impl Busses {
    fn init() -> Self {
        Self {
            cpu_address: 0,
            cpu_data: 0,
        }
    }
}
