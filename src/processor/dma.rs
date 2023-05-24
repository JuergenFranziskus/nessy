pub struct OamDma {
    last_m2: bool,
    get_cycle: bool,

    state: State,
    address_high: u8,
    address_low: u8,

    out: OutPins,
}
impl OamDma {
    pub fn init() -> Self {
        Self {
            last_m2: false,
            get_cycle: true,
            state: State::Idle,
            address_high: 0,
            address_low: 0,
            out: OutPins::init(),
        }
    }

    pub fn master_cycle(&mut self, pins: InPins) {
        let low_edge = self.last_m2 && self.last_m2 != pins.m2;
        if low_edge {
            self.cycle(pins);
            self.get_cycle = !self.get_cycle;
        }
        self.service_cpu(pins);

        self.last_m2 = pins.m2;
    }

    fn cycle(&mut self, pins: InPins) {
        match self.state {
            State::Idle => (),
            State::Initializing => {
                if self.get_cycle && pins.cpu_halted {
                    self.out.address = Some(self.address());
                    self.out.read = true;
                    self.state = State::Reading;
                }
            }
            State::Reading => {
                self.out.data = Some(pins.data);
                self.out.read = false;
                self.out.address = Some(0x2004);
                self.state = State::Writing;

                if self.address_low == 0xFF {
                    self.out.halt_cpu = false;
                    self.state = State::Ending;
                }
            }
            State::Writing => {
                self.address_low += 1;
                self.out.read = true;
                self.out.data = None;
                self.out.address = Some(self.address());
                self.state = State::Reading;
            }
            State::Ending => {
                self.out.data = None;
                self.address_low = 0;
                self.out.read = true;
                self.out.address = None;
                self.state = State::Idle;
            }
        }
    }
    fn service_cpu(&mut self, pins: InPins) {
        let high_edge = pins.m2 && pins.m2 != self.last_m2;
        if !high_edge {
            return;
        }
        if pins.address != 0x4014 {
            return;
        }
        if pins.read {
            return;
        };

        self.address_high = pins.data;
        self.state = State::Initializing;
        self.out.halt_cpu = true;
    }

    fn address(&self) -> u16 {
        self.address_low as u16 | (self.address_high as u16) << 8
    }

    pub fn out(&self) -> OutPins {
        self.out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Idle,
    Initializing,
    Reading,
    Writing,
    Ending,
}

#[derive(Clone, Copy, Debug)]
pub struct InPins {
    pub m2: bool,
    pub cpu_halted: bool,
    pub data: u8,
    pub address: u16,
    pub read: bool,
}
impl InPins {
    pub fn init() -> Self {
        Self {
            m2: false,
            cpu_halted: false,
            data: 0,
            address: 0,
            read: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct OutPins {
    pub halt_cpu: bool,
    pub data: Option<u8>,
    pub address: Option<u16>,
    pub read: bool,
}
impl OutPins {
    pub fn init() -> Self {
        Self {
            halt_cpu: false,
            data: None,
            address: None,
            read: true,
        }
    }
}
