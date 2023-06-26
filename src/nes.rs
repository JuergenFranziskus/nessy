use crate::cpu::CpuBus;
use crate::dma::Dma;
use crate::joystick::Joystick;
use crate::ppu::Ppu;
use crate::{cpu::Cpu, mapper::Mapper};

pub struct Nes<M> {
    cpu: Cpu,
    ppu: Ppu,
    mapper: M,
    joystick: Joystick,
    dma: Dma,

    cycle: u64,
    bus: NesBus,

    last_m2: bool,
    last_ppu_read: bool,
    last_ppu_write: bool,

    memory: [u8; 2048],
    vram: [u8; 2048],
}
impl<M> Nes<M> {
    pub fn new(mapper: M) -> Self {
        Self {
            cpu: Cpu::new(),
            ppu: Ppu::new(),
            mapper,
            joystick: Joystick::new(),
            dma: Dma::new(),

            cycle: 0,
            bus: NesBus::new(),

            last_m2: false,
            last_ppu_read: false,
            last_ppu_write: false,

            memory: [0; 2048],
            vram: [0; 2048],
        }
    }

    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }

    pub fn ppu(&self) -> &Ppu {
        &self.ppu
    }
    pub fn vram(&self) -> &[u8] {
        &self.vram
    }

    pub fn bus(&self) -> &NesBus {
        &self.bus
    }
    pub fn cycle(&self) -> u64 {
        self.cycle
    }

    pub fn joysticks_mut(&mut self) -> &mut Joystick {
        &mut self.joystick
    }
}
impl<M: Mapper> Nes<M> {
    pub fn master_cycle(&mut self) {
        self.bus.cpu_m2 = self.cycle % 12 >= 6;

        self.cycle_devices();
        self.update_memories();
        self.update_edge_detectors();
        self.tick_cycle_counter();
    }
    fn cycle_devices(&mut self) {
        if self.cpu_should_cycle() {
            self.cpu.cycle(&mut self.bus);
        }

        self.ppu.master_cycle(&mut self.bus, self.cycle);
        self.mapper.master_cycle(&mut self.bus, self.cycle);
        self.joystick.master_cycle(&mut self.bus);
        self.dma.master_cycle(&mut self.bus, self.cycle);
    }

    fn cpu_should_cycle(&self) -> bool {
        self.cycle % 12 == 0
    }

    fn update_memories(&mut self) {
        self.update_cpu_memory();
        self.update_ppu_memory();
    }
    fn update_cpu_memory(&mut self) {
        if !self.m2_edge() {
            return;
        }

        let address = self.bus.cpu_address as usize;
        if address >= 0x800 {
            return;
        }

        if self.bus.everyone_reads_cpu_bus() {
            self.bus.cpu_data = self.memory[address];
        } else {
            self.memory[address] = self.bus.cpu_data;
        }
    }
    fn update_ppu_memory(&mut self) {
        if !self.ppu_edge() {
            return;
        }

        let address = self.bus.ppu_address as usize;
        if !(0x2000..0x3EFF).contains(&address) {
            return;
        }

        let address_high = (self.bus.map_ciram_a10 as usize) << 10;
        let address_low = address & 0x3FF;
        let address = address_low | address_high;

        if self.bus.ppu_write_enable {
            self.vram[address] = self.bus.ppu_data;
        }
        if self.bus.ppu_read_enable {
            self.bus.ppu_data = self.vram[address];
        }
    }

    fn update_edge_detectors(&mut self) {
        self.last_m2 = self.bus.cpu_m2;
        self.last_ppu_read = self.bus.ppu_read_enable;
        self.last_ppu_write = self.bus.ppu_write_enable;
    }
    fn m2_edge(&self) -> bool {
        self.bus.cpu_m2 && self.bus.cpu_m2 != self.last_m2
    }
    fn ppu_edge(&self) -> bool {
        let read_edge = self.last_ppu_read != self.bus.ppu_read_enable && self.bus.ppu_read_enable;
        let write_edge =
            self.last_ppu_write != self.bus.ppu_write_enable && self.bus.ppu_write_enable;

        read_edge || write_edge
    }

    fn tick_cycle_counter(&mut self) {
        self.cycle += 1;
    }
}

#[derive(Copy, Clone, Debug)]
pub struct NesBus {
    pub cpu_address: u16,
    pub cpu_data: u8,
    pub cpu_read: bool,
    pub cpu_sync: bool,
    pub cpu_halted: bool,
    pub cpu_reset: bool,
    pub cpu_m2: bool,

    pub ppu_address: u16,
    pub ppu_data: u8,
    pub ppu_read_enable: bool,
    pub ppu_write_enable: bool,
    pub ppu_nmi: bool,

    pub map_irq: bool,
    pub map_ciram_enable: bool,
    pub map_ciram_a10: bool,

    pub oam_dma_halts_cpu: bool,
    pub oam_dma_writes: bool,
}
impl NesBus {
    pub fn new() -> Self {
        Self {
            cpu_address: 0,
            cpu_data: 0,
            cpu_read: true,
            cpu_sync: false,
            cpu_halted: false,
            cpu_reset: false,
            cpu_m2: false,

            ppu_address: 0,
            ppu_data: 0,
            ppu_read_enable: false,
            ppu_write_enable: false,
            ppu_nmi: false,

            map_irq: false,
            map_ciram_enable: false,
            map_ciram_a10: false,

            oam_dma_halts_cpu: false,
            oam_dma_writes: false,
        }
    }

    pub fn everyone_reads_cpu_bus(&self) -> bool {
        self.cpu_read && !self.oam_dma_writes
    }
}
impl CpuBus for NesBus {
    fn address(&self) -> u16 {
        self.cpu_address
    }

    fn set_address(&mut self, addr: u16) {
        self.cpu_address = addr;
    }

    fn data(&self) -> u8 {
        self.cpu_data
    }

    fn set_data(&mut self, data: u8) {
        self.cpu_data = data;
    }

    fn read(&self) -> bool {
        self.cpu_read
    }

    fn set_read(&mut self, read: bool) {
        self.cpu_read = read;
    }

    fn sync(&self) -> bool {
        self.cpu_sync
    }

    fn set_sync(&mut self, sync: bool) {
        self.cpu_sync = sync;
    }

    fn halted(&self) -> bool {
        self.cpu_halted
    }

    fn set_halted(&mut self, halted: bool) {
        self.cpu_halted = halted;
    }

    fn ready(&self) -> bool {
        !self.oam_dma_halts_cpu
    }

    fn irq(&self) -> bool {
        self.map_irq
    }

    fn nmi(&self) -> bool {
        self.ppu_nmi
    }

    fn reset(&self) -> bool {
        self.cpu_reset
    }

    type Backup = NesBusBackup;
    fn backup(&self) -> Self::Backup {
        NesBusBackup {
            address: self.cpu_address,
            data: self.cpu_data,
            read: self.cpu_read,
            sync: self.cpu_sync,
            halted: self.cpu_halted,
        }
    }
    fn restore(&mut self, backup: Self::Backup) {
        self.cpu_address = backup.address;
        self.cpu_data = backup.data;
        self.cpu_read = backup.read;
        self.cpu_sync = backup.sync;
        self.cpu_halted = backup.halted;
    }
}

pub struct NesBusBackup {
    address: u16,
    data: u8,
    read: bool,
    sync: bool,
    halted: bool,
}
