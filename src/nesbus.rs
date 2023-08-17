use crate::mapper::Mapper;
use cpu_6502::{Bus, Cpu};

pub struct NesBus<M, D> {
    cycle: u64,
    cpu: CpuBus,
    mapper: M,
    ram: Box<[u8; 2048]>,

    debug_callback: D,
}
impl<M, D> NesBus<M, D> {
    pub fn new(mapper: M, debug_callback: D) -> Self {
        Self {
            cycle: 0,
            cpu: CpuBus::init(),
            ram: Box::new([0; 2048]),
            mapper,
            debug_callback,
        }
    }
}
impl<M, D> Bus for NesBus<M, D>
where
    M: Mapper,
    D: FnMut(u64, &Cpu, CpuBus),
{
    fn data(&self) -> u8 {
        self.cpu.data()
    }

    fn rst(&self) -> bool {
        self.cpu.rst()
    }

    fn nmi(&self) -> bool {
        self.cpu.nmi()
    }

    fn irq(&self) -> bool {
        self.cpu.irq()
    }

    fn not_ready(&self) -> bool {
        self.cpu.not_ready()
    }

    fn set_data(&mut self, data: u8) {
        self.cpu.set_data(data);
    }

    fn set_address(&mut self, addr: u16) {
        self.cpu.set_address(addr);
    }

    fn set_read(&mut self, read: bool) {
        self.cpu.set_read(read);
    }

    fn set_sync(&mut self, sync: bool) {
        self.cpu.set_sync(sync);
    }

    fn set_halt(&mut self, halt: bool) {
        self.cpu.set_halt(halt);
    }

    fn cycle(&mut self, cpu: &cpu_6502::Cpu) {
        self.mapper.cycle(&mut self.cpu);

        let addr = self.cpu.address() as usize;
        if addr < 2048 {
            if self.cpu.read() {
                self.cpu.set_data(self.ram[addr]);
            } else {
                self.ram[addr] = self.cpu.data();
            }
        }

        (self.debug_callback)(self.cycle, cpu, self.cpu);
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
        self.flags & (1 << flag) != 0
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
        let mask = 1 << flag;
        self.flags &= !mask;
        self.flags |= if value { mask } else { 0 };
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
