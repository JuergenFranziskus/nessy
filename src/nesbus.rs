use crate::{
    mapper::{Mapper, MapperBus},
    ppu::{Ppu, PpuBus},
    util::{get_flag_u8, set_flag_u8},
};
use cpu_6502::{Bus, Cpu};

pub struct NesBus<M> {
    cycle: u64,
    cpu_bus: CpuBus,
    ppu_bus: PpuBus,
    mapper_bus: MapperBus,
    ppu: Ppu,
    mapper: M,
    ram: Box<[u8; 2048]>,
    vram: Box<[u8; 2048]>,

    debug_callback: Box<dyn FnMut(u64, &Cpu, CpuBus, &Ppu, PpuBus, MapperBus)>,
}
impl<M> NesBus<M> {
    pub fn new(
        mapper: M,
        debug_callback: impl FnMut(u64, &Cpu, CpuBus, &Ppu, PpuBus, MapperBus) + 'static,
    ) -> Self {
        Self {
            cycle: 0,
            cpu_bus: CpuBus::init(),
            ppu_bus: PpuBus::init(),
            mapper_bus: MapperBus::init(),
            ppu: Ppu::init(),
            mapper,
            ram: Box::new([0; 2048]),
            vram: Box::new([0; 2048]),

            debug_callback: Box::new(debug_callback) as _,
        }
    }

    pub fn ppu(&self) -> &Ppu {
        &self.ppu
    }
    pub fn vram(&self) -> &[u8] {
        &*self.vram
    }
    pub fn cycles(&self) -> u64 {
        self.cycle
    }
}
impl<M> NesBus<M>
where
    M: Mapper,
{
    fn cpu_cycle(&mut self) {
        self.ppu.cycle(&mut self.ppu_bus, &mut self.cpu_bus);
        self.mapper
            .cycle(&mut self.mapper_bus, &mut self.cpu_bus, &mut self.ppu_bus);
        self.update_ram();
        self.update_vram();
    }
    fn ppu_cycle(&mut self) {
        self.ppu.cycle_alone(&mut self.ppu_bus, &mut self.cpu_bus);
        self.mapper
            .cycle_with_ppu(&mut self.mapper_bus, &mut self.ppu_bus);
        self.update_vram();
    }

    fn update_ram(&mut self) {
        let addr = self.cpu_bus.address() as usize;
        if addr < 2048 {
            if self.cpu_bus.read() {
                self.cpu_bus.set_data(self.ram[addr]);
            } else {
                self.ram[addr] = self.cpu_bus.data();
            }
        }
    }
    fn update_vram(&mut self) {
        if !self.mapper_bus.vram_enable() {
            return;
        };
        let a10 = self.mapper_bus.vram_a10();
        let mask = 1 << 10;
        let addr = ((self.ppu_bus.address() % 0x800) & !mask) | if a10 { mask } else { 0 };
        let addr = addr as usize;

        if self.ppu_bus.read_enable() {
            self.ppu_bus.set_data(self.vram[addr]);
        }
        if self.ppu_bus.write_enable() {
            self.vram[addr] = self.ppu_bus.data();
        }
    }
}
impl<M> Bus for NesBus<M>
where
    M: Mapper,
{
    fn data(&self) -> u8 {
        self.cpu_bus.data()
    }

    fn rst(&self) -> bool {
        self.cpu_bus.rst()
    }

    fn nmi(&self) -> bool {
        self.cpu_bus.nmi()
    }

    fn irq(&self) -> bool {
        self.cpu_bus.irq()
    }

    fn not_ready(&self) -> bool {
        self.cpu_bus.not_ready()
    }

    fn set_data(&mut self, data: u8) {
        self.cpu_bus.set_data(data);
    }

    fn set_address(&mut self, addr: u16) {
        self.cpu_bus.set_address(addr);
    }

    fn set_read(&mut self, read: bool) {
        self.cpu_bus.set_read(read);
    }

    fn set_sync(&mut self, sync: bool) {
        self.cpu_bus.set_sync(sync);
    }

    fn set_halt(&mut self, halt: bool) {
        self.cpu_bus.set_halt(halt);
    }

    fn cycle(&mut self, cpu: &cpu_6502::Cpu) {
        self.cpu_cycle();
        self.ppu_cycle();
        self.ppu_cycle();

        (self.debug_callback)(
            self.cycle,
            cpu,
            self.cpu_bus,
            &self.ppu,
            self.ppu_bus,
            self.mapper_bus,
        );
        self.cycle += 1;
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct CpuBus {
    address: u16,
    data: u8,
    flags: u8,
}
impl CpuBus {
    pub fn init() -> Self {
        Self {
            address: 0,
            data: 0,
            flags: 0,
        }
    }

    pub fn address(self) -> u16 {
        self.address
    }
    pub fn data(self) -> u8 {
        self.data
    }

    fn get_flag(self, flag: u8) -> bool {
        get_flag_u8(self.flags, flag)
    }
    pub fn rst(self) -> bool {
        self.get_flag(Self::FLAG_RST)
    }
    pub fn nmi(self) -> bool {
        self.get_flag(Self::FLAG_NMI)
    }
    pub fn irq(self) -> bool {
        self.get_flag(Self::FLAG_IRQ)
    }
    pub fn read(self) -> bool {
        self.get_flag(Self::FLAG_READ)
    }
    pub fn sync(self) -> bool {
        self.get_flag(Self::FLAG_SYNC)
    }
    pub fn not_ready(self) -> bool {
        self.get_flag(Self::FLAG_NOT_READY)
    }
    pub fn halt(self) -> bool {
        self.get_flag(Self::FLAG_HALT)
    }

    pub fn set_address(&mut self, addr: u16) {
        self.address = addr;
    }
    pub fn set_data(&mut self, data: u8) {
        self.data = data;
    }

    fn set_flag(&mut self, flag: u8, value: bool) {
        set_flag_u8(&mut self.flags, flag, value)
    }
    pub fn set_rst(&mut self, rst: bool) {
        self.set_flag(Self::FLAG_RST, rst)
    }
    pub fn set_nmi(&mut self, nmi: bool) {
        self.set_flag(Self::FLAG_NMI, nmi)
    }
    pub fn set_irq(&mut self, irq: bool) {
        self.set_flag(Self::FLAG_IRQ, irq)
    }
    pub fn set_read(&mut self, read: bool) {
        self.set_flag(Self::FLAG_READ, read)
    }
    pub fn set_sync(&mut self, sync: bool) {
        self.set_flag(Self::FLAG_SYNC, sync)
    }
    pub fn set_not_ready(&mut self, not_ready: bool) {
        self.set_flag(Self::FLAG_NOT_READY, not_ready)
    }
    pub fn set_halt(&mut self, halt: bool) {
        self.set_flag(Self::FLAG_HALT, halt)
    }

    const FLAG_RST: u8 = 0;
    const FLAG_NMI: u8 = 1;
    const FLAG_IRQ: u8 = 2;
    const FLAG_READ: u8 = 3;
    const FLAG_SYNC: u8 = 4;
    const FLAG_NOT_READY: u8 = 5;
    const FLAG_HALT: u8 = 6;
}
