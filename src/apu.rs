use m6502::Bus as CpuBus;
use m6502::M6502;

const CPU_CLOCK_HZ: u32 = 1_789773;
const CYCLES_PER_FRAME: u32 = CPU_CLOCK_HZ / 60;
const APU_CYCLES_PER_FRAME: u32 = CYCLES_PER_FRAME / 2 + 1;
const APU_FRAME_COUNTER_TICK_ZERO: u32 = 3728;
const APU_FRAME_COUNTER_TICK_ONE: u32 = 7456;
const APU_FRAME_COUNTER_TICK_TWO: u32 = 11185;
const APU_FRAME_COUNTER_TICK_THREE: u32 = 14914;
const APU_FRAME_COUNTER_TICKS: [u32; 4] = [
    APU_FRAME_COUNTER_TICK_ZERO,
    APU_FRAME_COUNTER_TICK_ONE,
    APU_FRAME_COUNTER_TICK_TWO,
    APU_FRAME_COUNTER_TICK_THREE,
];

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Bus {
    pub addr: u16,
    pub data: u8,
    flags: u8,
}
impl Bus {
    pub fn new() -> Self {
        Self {
            addr: 0,
            data: 0,
            flags: 0,
        }
    }

    pub fn irq(self) -> bool {
        self.flags & Self::IRQ != 0
    }
    pub fn nmi(self) -> bool {
        self.flags & Self::NMI != 0
    }
    pub fn rw(self) -> bool {
        self.flags & Self::RW != 0
    }
    pub fn sync(self) -> bool {
        self.flags & Self::SYNC != 0
    }

    pub fn set_irq(&mut self, to: bool) {
        self.flags &= !Self::IRQ;
        if to {
            self.flags |= Self::IRQ;
        }
    }
    pub fn set_nmi(&mut self, to: bool) {
        self.flags &= !Self::NMI;
        if to {
            self.flags |= Self::NMI;
        }
    }
    pub fn set_rw(&mut self, to: bool) {
        self.flags &= !Self::RW;
        if to {
            self.flags |= Self::RW;
        }
    }
    pub fn set_sync(&mut self, to: bool) {
        self.flags &= !Self::SYNC;
        if to {
            self.flags |= Self::SYNC;
        }
    }

    const IRQ: u8 = 1;
    const NMI: u8 = 2;
    const RW: u8 = 4;
    const SYNC: u8 = 8;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Apu {
    cpu: M6502,
    cpu_bus: CpuBus,

    put_cycle: bool,
    oam_dma: Dma,
    oam_bank: u8,
    oam_cycle: u8,

    status: Status,

    apu_cycle: u32,
    frame_counter: FrameCounter,

    controllers: [Controller; 2],
    controller_strobe: bool,
}
impl Apu {
    pub fn start() -> Self {
        Self {
            cpu: M6502::start(),
            cpu_bus: CpuBus::new(),

            put_cycle: false,
            oam_dma: Dma::Idle,
            oam_bank: 0,
            oam_cycle: 0,

            status: Status::new(),

            apu_cycle: 0,
            frame_counter: FrameCounter::new(),

            controllers: [Controller::new(); _],
            controller_strobe: false,
        }
    }

    pub fn cpu(&self) -> M6502 {
        self.cpu
    }
    pub fn controllers(&mut self) -> &mut [Controller; 2] {
        &mut self.controllers
    }

    pub fn clock(&mut self, bus: &mut Bus) {
        self.strobe_controllers();
        self.clock_cpu(bus);
        self.handle_cpu(bus);
        self.clock_apu();
        self.tick_counters();
    }
    fn strobe_controllers(&mut self) {
        if self.controller_strobe {
            self.controllers[0].strobe();
            self.controllers[1].strobe();
        }
    }
    fn clock_cpu(&mut self, bus: &mut Bus) {
        self.sync_cpu_bus(bus);

        match self.oam_dma {
            Dma::Idle => {
                self.cpu.clock(&mut self.cpu_bus);
                self.sync_apu_bus(bus);
            }
            Dma::Halt => {
                self.cpu.clock(&mut self.cpu_bus);
                if self.cpu_bus.rw() {
                    if self.put_cycle {
                        self.oam_dma = Dma::Get;
                    } else {
                        self.oam_dma = Dma::Align;
                    }
                }
                self.sync_apu_bus(bus);
            }
            Dma::Align => {
                if self.put_cycle {
                    self.oam_dma = Dma::Get;
                }
            }
            Dma::Get => {
                bus.addr = (self.oam_bank as u16) << 8 | (self.oam_cycle) as u16;
                bus.set_rw(true);
                bus.set_sync(false);
                self.oam_dma = Dma::Put;
            }
            Dma::Put => {
                bus.addr = 0x2004;
                bus.set_rw(false);
                bus.set_sync(false);
                let end;
                (self.oam_cycle, end) = self.oam_cycle.overflowing_add(1);
                if end {
                    self.oam_dma = Dma::End
                } else {
                    self.oam_dma = Dma::Get;
                }
            }
            Dma::End => {
                self.sync_apu_bus(bus);
                self.oam_dma = Dma::Idle;
            }
        }
    }
    fn sync_cpu_bus(&mut self, bus: &Bus) {
        self.cpu_bus.data = bus.data;
        self.cpu_bus
            .set_irq(bus.irq() || self.status.is_irq_active());
        self.cpu_bus.set_nmi(bus.nmi());
    }
    fn sync_apu_bus(&self, bus: &mut Bus) {
        bus.addr = self.cpu_bus.addr;
        bus.data = self.cpu_bus.data;
        bus.set_rw(self.cpu_bus.rw());
        bus.set_sync(self.cpu_bus.sync());
    }

    fn handle_cpu(&mut self, bus: &mut Bus) {
        match self.cpu_bus.addr {
            0x4014 if !bus.rw() => {
                self.oam_bank = self.cpu_bus.data;
                self.oam_cycle = 0;
                self.oam_dma = Dma::Halt;
            }
            0x4015 => {
                if bus.rw() {
                    let i = (self.status.dmc_irq as u8) << 7;
                    let f = (self.status.frame_irq as u8) << 6;
                    bus.data = i | f;
                } else {
                    self.status.dmc_irq = false;
                }
            }
            0x4016 => {
                if bus.rw() {
                    self.controllers[0].read(bus);
                    self.controllers[0].shift();
                } else {
                    self.controller_strobe = bus.data & 1 != 0;
                }
            }
            0x4017 => {
                if !bus.rw() {
                    self.frame_counter.mode = bus.data & 0x80 != 0;
                    self.frame_counter.irq_inhibit = bus.data & 0x40 != 0;
                    self.frame_counter.step = 0;
                } else {
                    self.controllers[1].read(bus);
                    self.controllers[1].shift();
                }
            }
            _ => (),
        }
    }
    fn tick_counters(&mut self) {
        self.put_cycle = !self.put_cycle;
        if self.put_cycle {
            self.apu_cycle += 1;
            self.apu_cycle %= APU_CYCLES_PER_FRAME;
        }
    }

    fn clock_apu(&mut self) {
        if self.put_cycle {
            self.do_apu_put_cycle();
        } else {
            self.do_apu_get_cycle();
        }
    }
    fn do_apu_get_cycle(&mut self) {}
    fn do_apu_put_cycle(&mut self) {
        if APU_FRAME_COUNTER_TICKS.contains(&self.apu_cycle) {
            self.tick_frame_counter();
        }
    }
    fn tick_frame_counter(&mut self) {
        if self.frame_counter.should_raise_irq() {
            self.status.frame_irq = true;
        }

        if self.frame_counter.should_tick_envelope_and_linear() {
            self.tick_envelope_and_linear();
        }

        if self.frame_counter.should_tick_length_and_sweep() {
            self.tick_length_and_sweep();
        }

        self.frame_counter.tick();
    }
    fn tick_envelope_and_linear(&mut self) {}
    fn tick_length_and_sweep(&mut self) {}
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Dma {
    Idle,
    Halt,
    Align,
    Get,
    Put,
    End,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct Status {
    dmc_irq: bool,
    frame_irq: bool,
}
impl Status {
    fn new() -> Self {
        Self {
            dmc_irq: false,
            frame_irq: false,
        }
    }
    fn is_irq_active(&self) -> bool {
        self.dmc_irq | self.frame_irq
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct FrameCounter {
    mode: bool,
    irq_inhibit: bool,
    step: u8,
}
impl FrameCounter {
    fn new() -> Self {
        Self {
            mode: false,
            irq_inhibit: true,
            step: 0,
        }
    }

    fn should_tick_length_and_sweep(&self) -> bool {
        if self.mode {
            self.step == 1 || self.step == 4
        } else {
            self.step == 1 || self.step == 3
        }
    }
    fn should_tick_envelope_and_linear(&self) -> bool {
        if self.mode { self.step != 3 } else { true }
    }
    fn should_raise_irq(&self) -> bool {
        !self.mode && self.step == 3
    }

    fn tick(&mut self) {
        self.step += 1;

        if (self.mode && self.step > 4) || (!self.mode && self.step > 3) {
            self.step = 0
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Controller {
    latch: u8,
    shift: u8,
}
impl Controller {
    fn new() -> Self {
        Self {
            latch: 0,
            shift: 0xFF,
        }
    }
    fn strobe(&mut self) {
        self.shift = self.latch;
    }

    fn read(&self, bus: &mut Bus) {
        bus.data = (bus.data & !0x7) | (self.shift & 1);
    }
    fn shift(&mut self) {
        self.shift = (self.shift >> 1) | 0x80;
    }

    pub fn set_a(&mut self, to: bool) {
        self.latch &= !Self::A;
        if to {
            self.latch |= Self::A;
        }
    }
    pub fn set_b(&mut self, to: bool) {
        self.latch &= !Self::B;
        if to {
            self.latch |= Self::B;
        }
    }
    pub fn set_select(&mut self, to: bool) {
        self.latch &= !Self::SELECT;
        if to {
            self.latch |= Self::SELECT;
        }
    }
    pub fn set_start(&mut self, to: bool) {
        self.latch &= !Self::START;
        if to {
            self.latch |= Self::START;
        }
    }
    pub fn set_up(&mut self, to: bool) {
        self.latch &= !Self::UP;
        if to {
            self.latch |= Self::UP;
        }
    }
    pub fn set_down(&mut self, to: bool) {
        self.latch &= !Self::DOWN;
        if to {
            self.latch |= Self::DOWN;
        }
    }
    pub fn set_left(&mut self, to: bool) {
        self.latch &= !Self::LEFT;
        if to {
            self.latch |= Self::LEFT;
        }
    }
    pub fn set_right(&mut self, to: bool) {
        self.latch &= !Self::RIGHT;
        if to {
            self.latch |= Self::RIGHT;
        }
    }

    const A: u8 = 1;
    const B: u8 = 2;
    const SELECT: u8 = 4;
    const START: u8 = 8;
    const UP: u8 = 16;
    const DOWN: u8 = 32;
    const LEFT: u8 = 64;
    const RIGHT: u8 = 128;
}
