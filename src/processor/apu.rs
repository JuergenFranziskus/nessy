pub struct Apu {
    master_cycle: u8,
    last_m2: bool,

    dmc: Dmc,
    status: Status,
    frame_counter: FrameCounter,

    out: AOutPins,
}
impl Apu {
    pub fn init() -> Self {
        Self {
            master_cycle: 0,
            last_m2: false,

            dmc: Dmc::init(),
            status: Status::init(),
            frame_counter: FrameCounter::init(),
            out: AOutPins::init(),
        }
    }

    pub fn master_cycle(&mut self, pins: AInPins) {
        if self.should_cycle() {
            self.cycle(pins);
        }
        self.service_cpu(pins);
        self.tick_counter();

        self.last_m2 = pins.m2;
        self.out.irq = self.status.dmc_interrupt || self.status.frame_interrupt;
    }

    fn cycle(&mut self, _pins: AInPins) {
        self.status.frame_interrupt = self.frame_counter.tick == 0;

        if self.frame_counter.cycle == 0 {
            self.frame_counter.tick += 1;
            let max = if self.frame_counter.mode { 5 } else { 4 };
            self.frame_counter.tick %= max;
        }

        self.frame_counter.cycle += 1;
        self.frame_counter.cycle %= 3728;
    }
    fn service_cpu(&mut self, pins: AInPins) {
        let m2_edge = self.last_m2 != pins.m2;
        if !m2_edge || !pins.m2 {
            return;
        }

        if !(0x4000..0x4018).contains(&pins.address) {
            return;
        }
        let address = pins.address - 0x4000;

        // eprintln!("APU register access: {address:x}, write: {:x?}", (!pins.read).then_some(pins.data));

        match address {
            0x0..=0xF => (), // Sound control, not yet relevant
            0x10 => {
                if pins.read {
                    return;
                }

                self.dmc.irq_enable = pins.data & 128 != 0;
                self.dmc.loop_enable = pins.data & 64 != 0;
                self.dmc.frequency = pins.data & 0xF;
            }
            0x11 => {
                if pins.read {
                    return;
                }
                self.dmc.load_counter = pins.data & 0b1111111;
            }
            0x12 => {
                if pins.read {
                    return;
                }
                self.dmc.sample_address = pins.data;
            }
            0x13 => {
                if pins.read {
                    return;
                }
                self.dmc.sample_length = pins.data;
            }
            0x14 => (), // OAM DMA, not yet relevant
            0x15 => {
                if pins.read {
                    let dmc_irq = (self.status.dmc_interrupt as u8) << 7;
                    let frame_irq = (self.status.frame_interrupt as u8) << 6;
                    self.out.data = Some(dmc_irq | frame_irq);

                    self.status.frame_interrupt = false;
                } else {
                    self.status.enable_dmc = pins.data & 16 != 0;
                    self.status.enable_noise = pins.data & 8 != 0;
                    self.status.enable_triangle = pins.data & 4 != 0;
                    self.status.enable_pulse_2 = pins.data & 2 != 0;
                    self.status.enable_pulse_1 = pins.data & 1 != 0;
                }
            }
            0x16 => (), // Handled by joysticks
            0x17 => {
                if pins.read {
                    return;
                }

                self.frame_counter.mode = pins.data & 128 != 0;
                self.frame_counter.irq_enable = pins.data & 64 == 0;
            }
            _ => unreachable!("#{address:x} is not a valid apu register or is not implemented"),
        }
    }

    fn should_cycle(&self) -> bool {
        self.master_cycle == 0
    }
    fn tick_counter(&mut self) {
        self.master_cycle += 1;
        self.master_cycle %= 24;
    }

    pub fn out(&self) -> AOutPins {
        self.out
    }
}

#[derive(Copy, Clone, Debug)]
pub struct AInPins {
    pub m2: bool,
    pub address: u16,
    pub data: u8,
    pub read: bool,
}
impl AInPins {
    pub fn init() -> Self {
        Self {
            m2: false,
            address: 0,
            data: 0,
            read: false,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct AOutPins {
    pub data: Option<u8>,
    pub irq: bool,
}
impl AOutPins {
    pub fn init() -> Self {
        Self {
            data: None,
            irq: false,
        }
    }
}

struct Dmc {
    irq_enable: bool,
    loop_enable: bool,
    frequency: u8,
    load_counter: u8,
    sample_address: u8,
    sample_length: u8,
}
impl Dmc {
    fn init() -> Self {
        Self {
            irq_enable: false,
            loop_enable: false,
            frequency: 0,
            load_counter: 0,
            sample_address: 0,
            sample_length: 0,
        }
    }
}

struct Status {
    enable_dmc: bool,
    enable_noise: bool,
    enable_triangle: bool,
    enable_pulse_1: bool,
    enable_pulse_2: bool,
    dmc_interrupt: bool,
    frame_interrupt: bool,
}
impl Status {
    fn init() -> Self {
        Self {
            enable_dmc: false,
            enable_noise: false,
            enable_triangle: false,
            enable_pulse_1: false,
            enable_pulse_2: false,

            dmc_interrupt: false,
            frame_interrupt: false,
        }
    }
}

struct FrameCounter {
    mode: bool,
    irq_enable: bool,
    tick: u8,
    cycle: u16,
}
impl FrameCounter {
    fn init() -> Self {
        Self {
            mode: false,
            irq_enable: false,
            tick: 0,
            cycle: 0,
        }
    }
}
