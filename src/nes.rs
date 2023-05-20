use crate::ppu::{InPins as PPins, Ppu};
use crate::{cpu::Cpu, cpu::InPins as CPins, mapper::InPins as MPins, mapper::Mapper};

pub struct Nes<M> {
    cpu: Cpu,
    ppu: Ppu,
    mapper: M,

    cpu_cycles: usize,
    ppu_cycles: usize,

    cpu_pins: CPins,
    ppu_pins: PPins,
    mapper_pins: MPins,

    ppu_address_latch: u8,

    busses: Busses,
    memory: [u8; 2048],
    vram: [u8; 2048],
}
impl<M> Nes<M> {
    pub fn new(mapper: M) -> Self {
        let (cpu, cpu_pins) = Cpu::new();
        let mapper_pins = MPins::init();
        Self {
            cpu,
            ppu: Ppu::new(),
            mapper,

            cpu_cycles: 0,
            ppu_cycles: 0,

            cpu_pins,
            ppu_pins: PPins::init(),
            mapper_pins,

            ppu_address_latch: 0,

            busses: Busses::init(),
            memory: [0; 2048],
            vram: [0; 2048],
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
        self.tick_counters();
    }
    fn cycle_devices(&mut self) {
        if self.cpu_should_cycle() {
            self.cycle_cpu();
        }
        if self.ppu_should_cycle() {
            self.cycle_ppu();
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
        self.cpu_pins.nmi = self.ppu.out().nmi;
    }
    fn cycle_ppu(&mut self) {
        self.update_ppu_pins();
        self.ppu.cycle(self.ppu_pins);
        self.update_ppu_address_latch();
    }
    fn update_ppu_pins(&mut self) {
        self.ppu_pins.cpu_cycle = self.cpu_should_cycle();
        self.ppu_pins.cpu_read = self.cpu.out().read;
        self.ppu_pins.cpu_address = self.cpu.out().address;
        self.ppu_pins.cpu_data = self.busses.cpu_data;

        self.ppu_pins.mem_data = self.busses.ppu_data;
    }
    fn update_ppu_address_latch(&mut self) {
        let out = self.ppu.out();
        if out.ale {
            self.ppu_address_latch = (out.mem_address_data & 0xFF) as u8;
        }
    }
    fn cycle_mapper(&mut self) {
        self.update_mapper_pins();
        self.mapper.cycle(self.mapper_pins);
    }
    fn update_mapper_pins(&mut self) {
        let cpu_out = self.cpu.out();
        let ppu_out = self.ppu.out();

        self.mapper_pins.cpu_address = cpu_out.address;
        self.mapper_pins.cpu_data = self.busses.cpu_data;
        self.mapper_pins.cpu_read = cpu_out.read;
        self.mapper_pins.cpu_cycle = self.cpu_should_cycle();

        self.mapper_pins.ppu_address = self.ppu_address();
        self.mapper_pins.ppu_data = self.ppu_data();
        self.mapper_pins.ppu_read_enable = ppu_out.read_enable;
        self.mapper_pins.ppu_write_enable = ppu_out.write_enable;
        self.mapper_pins.ppu_cycle = self.ppu_should_cycle();
    }
    fn update_busses(&mut self) {
        self.update_ppu_data_bus(); // This has to happen before self.update_cpu_data_bus(), because the busses might be crossed.
        self.update_cpu_data_bus();
    }
    fn update_ppu_data_bus(&mut self) {
        let ppu_out = self.ppu.out();
        let map_out = self.mapper.out();

        self.busses.ppu_data = 0;
        if ppu_out.write_enable {
            self.busses.ppu_data |= self.ppu_data();
        }
        if let Some(data) = map_out.ppu_data {
            self.busses.ppu_data |= data;
        }

        self.update_ppu_memory();
    }
    fn update_ppu_memory(&mut self) {
        let ppu_out = self.ppu.out();
        let map_out = self.mapper.out();
        let address = self.ppu_address() as usize;

        if map_out.ciram_ce {
            let address = (address & 0x7FF) | self.mapper.out().ciram_a10 as usize;

            if ppu_out.write_enable && !ppu_out.ale {
                self.vram[address] = self.busses.ppu_data;
            }
            if ppu_out.read_enable && !ppu_out.ale {
                self.busses.ppu_data |= self.vram[address];
            }
        }
    }
    fn update_cpu_data_bus(&mut self) {
        let cpu_out = self.cpu.out();
        let ppu_out = self.ppu.out();
        let map_out = self.mapper.out();

        self.busses.cpu_data = 0;
        if !cpu_out.read {
            self.busses.cpu_data |= cpu_out.data;
        }
        if let Some(data) = map_out.cpu_data {
            self.busses.cpu_data |= data;
        }
        if let Some(data) = ppu_out.cpu_data {
            self.busses.cpu_data |= data;
        }
        if ppu_out.cross_data_busses {
            // Required for addressing ppu memory thru $PPUDATA
            self.busses.cpu_data |= self.busses.ppu_data;
        }

        self.update_cpu_memory();
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

        self.ppu_cycles += 1;
        self.ppu_cycles %= 4;
    }

    fn cpu_should_cycle(&self) -> bool {
        self.cpu_cycles == 0
    }
    fn ppu_should_cycle(&self) -> bool {
        self.ppu_cycles == 0
    }

    fn ppu_address(&self) -> u16 {
        let high = self.ppu.out().mem_address_data & 0xFF00;
        let low = self.ppu_address_latch as u16;
        low | high
    }
    fn ppu_data(&self) -> u8 {
        (self.ppu.out().mem_address_data & 0xFF) as u8
    }

    pub fn force_update_pins(&mut self) {
        self.update_cpu_pins();
        self.update_ppu_pins();
        self.update_mapper_pins();
    }
}

struct Busses {
    cpu_data: u8,
    ppu_data: u8,
}
impl Busses {
    fn init() -> Self {
        Self {
            cpu_data: 0,
            ppu_data: 0,
        }
    }
}
