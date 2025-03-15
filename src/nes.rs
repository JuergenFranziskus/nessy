use crate::apu::Apu;
use crate::apu::Bus as CpuBus;
use crate::mapper::Bus as MapperBus;
use crate::mapper::Mapper;
use crate::ppu::Bus as PpuBus;
use crate::ppu::Ppu;

pub struct Nes {
    pub cpu: Apu,
    pub cpu_bus: CpuBus,
    pub ppu: Ppu,
    pub ppu_bus: PpuBus,
    pub mapper: Box<dyn Mapper>,
    pub mapper_bus: MapperBus,

    pub ram: [u8; 2048],
    pub vram: [u8; 2048],
}
impl Nes {
    pub fn new(mapper: Box<dyn Mapper>) -> Self {
        Self {
            cpu: Apu::start(),
            cpu_bus: CpuBus::new(),
            ppu: Ppu::start(),
            ppu_bus: PpuBus::new(),
            mapper,
            mapper_bus: MapperBus::new(),

            ram: [0; 2048],
            vram: [0; 2048],
        }
    }

    pub fn clock(&mut self) -> [(u8, u32, u32); 3] {
        self.cpu.clock(&mut self.cpu_bus);
        self.ppu.clock(&mut self.ppu_bus, &mut self.cpu_bus, true);
        self.mapper
            .clock_with_cpu(&mut self.mapper_bus, &mut self.cpu_bus, &mut self.ppu_bus);
        self.update_ram();
        self.update_vram();

        let p0 = self.ppu.output();

        self.ppu.clock(&mut self.ppu_bus, &mut self.cpu_bus, false);
        self.mapper
            .clock_with_ppu(&mut self.mapper_bus, &mut self.ppu_bus);
        self.update_vram();

        let p1 = self.ppu.output();

        self.ppu.clock(&mut self.ppu_bus, &mut self.cpu_bus, false);
        self.mapper
            .clock_with_ppu(&mut self.mapper_bus, &mut self.ppu_bus);
        self.update_vram();

        let p2 = self.ppu.output();

        [p0, p1, p2]
    }

    fn update_ram(&mut self) {
        let addr = self.cpu_bus.addr as usize;
        if addr >= 0x2000 {
            return;
        };
        let offset = addr % 0x800;
        if self.cpu_bus.rw() {
            self.cpu_bus.data = self.ram[offset];
        } else {
            self.ram[offset] = self.cpu_bus.data;
        }
    }
    fn update_vram(&mut self) {
        if !self.mapper_bus.ciram_ce() {
            return;
        };

        let a_10 = if self.mapper_bus.ciram_a10() {
            1 << 10
        } else {
            0
        };
        let a_other = self.ppu_bus.addr & 0b11111_11111;
        let offset = (a_10 | a_other) as usize;

        if self.ppu_bus.rd() {
            self.ppu_bus.data = self.vram[offset];
        } else if self.ppu_bus.wr() {
            self.vram[offset] = self.ppu_bus.data;
        }
    }
}
