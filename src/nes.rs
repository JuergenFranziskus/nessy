use crate::joystick::InPins as JPins;
use crate::joystick::Joystick;
use crate::ppu::{InPins as PPins, Ppu};
use crate::processor::InPins as RPins;
use crate::processor::Processor;
use crate::{cpu::Cpu, mapper::InPins as MPins, mapper::Mapper};

pub struct Nes<M> {
    processor: Processor,
    ppu: Ppu,
    mapper: M,
    joystick: Joystick,

    processor_pins: RPins,
    ppu_pins: PPins,
    mapper_pins: MPins,
    joystick_pins: JPins,

    ppu_address_latch: u8,

    busses: Busses,
    memory: [u8; 2048],
    vram: [u8; 2048],
    palette_memory: [u8; 0x20],
}
impl<M> Nes<M> {
    pub fn new(mapper: M) -> Self {
        Self {
            processor: Processor::new(),
            ppu: Ppu::new(),
            mapper,
            joystick: Joystick::new(),

            processor_pins: RPins::init(),
            ppu_pins: PPins::init(),
            mapper_pins: MPins::init(),
            joystick_pins: JPins::init(),

            ppu_address_latch: 0,

            busses: Busses::init(),
            memory: [0; 2048],
            vram: [0; 2048],
            palette_memory: [0; 0x20],
        }
    }

    pub fn cpu(&self) -> &Cpu {
        self.processor.cpu()
    }
    pub fn processor_pins(&self) -> RPins {
        self.processor_pins
    }
    pub fn processor(&self) -> &Processor {
        &self.processor
    }

    pub fn ppu(&self) -> &Ppu {
        &self.ppu
    }
    pub fn vram(&self) -> &[u8] {
        &self.vram
    }

    pub fn joysticks_mut(&mut self) -> &mut Joystick {
        &mut self.joystick
    }
}
impl<M: Mapper> Nes<M> {
    pub fn master_cycle(&mut self) {
        self.update_pins();
        self.cycle_devices();
        self.update_busses();
    }
    fn cycle_devices(&mut self) {
        self.processor.master_cycle(self.processor_pins);
        self.ppu.master_cycle(self.ppu_pins);
        self.mapper.master_cycle(self.mapper_pins);
        self.joystick.master_cycle(self.joystick_pins);
    }

    fn update_pins(&mut self) {
        self.update_cpu_pins();
        self.update_ppu_pins();
        self.update_mapper_pins();
        self.update_joystick_pins();
    }
    fn update_cpu_pins(&mut self) {
        self.processor_pins.data = self.busses.processor_data;
        self.processor_pins.irq = self.mapper.out().irq;
        self.processor_pins.nmi = self.ppu.out().nmi;
    }
    fn update_ppu_pins(&mut self) {
        self.ppu_pins.cpu_m2 = self.processor.out().m2;
        self.ppu_pins.cpu_read = self.processor.out().read;
        self.ppu_pins.cpu_address = self.processor.out().address;
        self.ppu_pins.cpu_data = self.busses.processor_data;

        self.ppu_pins.mem_data = self.busses.ppu_data;
    }
    fn update_mapper_pins(&mut self) {
        let cpu_out = self.processor.out();
        let ppu_out = self.ppu.out();

        self.mapper_pins.cpu_address = cpu_out.address;
        self.mapper_pins.cpu_data = self.busses.processor_data;
        self.mapper_pins.cpu_read = cpu_out.read;
        self.mapper_pins.cpu_m2 = cpu_out.m2;

        self.mapper_pins.ppu_address = self.ppu_address();
        self.mapper_pins.ppu_data = self.busses.ppu_data;
        self.mapper_pins.ppu_read_enable = ppu_out.read_enable;
        self.mapper_pins.ppu_write_enable = ppu_out.write_enable;
    }
    fn update_joystick_pins(&mut self) {
        self.joystick_pins.address = self.processor.out().address;
        self.joystick_pins.data = self.processor.out().data;
        self.joystick_pins.read = self.processor.out().read;
        self.joystick_pins.cpu_m2 = self.processor.out().m2;
    }
    fn update_busses(&mut self) {
        self.update_ppu_address_latch();
        self.update_ppu_data_bus(); // This has to happen before self.update_cpu_data_bus(), because the busses might be crossed.
        self.update_cpu_data_bus();
    }
    fn update_ppu_address_latch(&mut self) {
        let ppu_out = self.ppu.out();
        if ppu_out.ale {
            self.ppu_address_latch = (ppu_out.mem_address_data & 0xFF) as u8;
        }
    }
    fn update_ppu_data_bus(&mut self) {
        let ppu_out = self.ppu.out();
        let map_out = self.mapper.out();

        self.busses.ppu_data = 0;
        if ppu_out.write_enable {
            self.busses.ppu_data = self.ppu_data();
        } else if let Some(data) = map_out.ppu_data {
            self.busses.ppu_data = data;
        }

        self.update_ppu_memory();
        self.update_palette_memory();
    }
    fn update_ppu_memory(&mut self) {
        let ppu_out = self.ppu.out();
        let map_out = self.mapper.out();
        let address = self.ppu_address() as usize;

        if map_out.ciram_ce {
            let address = (address & 0x3FF) | (self.mapper.out().ciram_a10 as usize) << 10;

            if ppu_out.write_enable && !ppu_out.ale {
                self.vram[address] = self.busses.ppu_data;
            }
            if ppu_out.read_enable && !ppu_out.ale {
                self.busses.ppu_data |= self.vram[address];
            }
        }
    }
    fn update_palette_memory(&mut self) {
        let ppu_out = self.ppu.out();
        let address = self.ppu_address() as usize;

        if !(0x3F00..0x4000).contains(&address) {
            return;
        }

        let address = (address - 0x3F00) % 0x20;
        let address = match address {
            0x10 => 0x0,
            0x14 => 0x4,
            0x18 => 0x8,
            0x1C => 0xC,
            _ => address,
        };

        if ppu_out.write_enable && !ppu_out.ale {
            self.palette_memory[address] = self.busses.ppu_data;
        }
        if ppu_out.read_enable && !ppu_out.ale {
            self.busses.ppu_data = self.palette_memory[address];
        }
    }
    fn update_cpu_data_bus(&mut self) {
        let cpu_out = self.processor.out();
        let ppu_out = self.ppu.out();
        let map_out = self.mapper.out();
        let joy_out = self.joystick.out();

        self.busses.processor_data = 0;
        if !cpu_out.read {
            self.busses.processor_data = cpu_out.data;
        } else if let Some(data) = map_out.cpu_data {
            self.busses.processor_data = data;
        } else if let Some(data) = ppu_out.cpu_data {
            self.busses.processor_data = data;
        } else if let Some(data) = joy_out.data {
            self.busses.processor_data = data;
        }
        if ppu_out.cross_data_busses {
            // Required for addressing ppu memory thru $PPUDATA
            self.busses.processor_data = self.busses.ppu_data;
        }

        self.update_cpu_memory();
    }
    fn update_cpu_memory(&mut self) {
        let cpu_out = self.processor.out();

        let address = cpu_out.address as usize;
        if address < 0x2000 {
            let address = address % 0x800;

            if cpu_out.read {
                self.busses.processor_data |= self.memory[address];
            } else {
                self.memory[address] = self.busses.processor_data;
            }
        }
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
    processor_data: u8,
    ppu_data: u8,
}
impl Busses {
    fn init() -> Self {
        Self {
            processor_data: 0,
            ppu_data: 0,
        }
    }
}
