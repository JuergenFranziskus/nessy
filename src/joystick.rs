pub struct Joystick {
    last_m2: bool,

    pads: [[bool; 8]; 2],
    indices: [usize; 2],
    strobe: bool,

    out: OutPins,
}
impl Joystick {
    pub fn new() -> Self {
        Self {
            last_m2: false,

            pads: [[false; 8]; 2],
            indices: [0; 2],
            strobe: false,
            out: OutPins::init(),
        }
    }

    pub fn master_cycle(&mut self, pins: InPins) {
        self.service_cpu(pins);
        if self.strobe {
            self.indices = [0; 2];
        }
        self.last_m2 = pins.cpu_m2;
    }

    fn service_cpu(&mut self, pins: InPins) {
        let m2_edge = self.last_m2 != pins.cpu_m2;
        if !m2_edge || !pins.cpu_m2 {
            return;
        }
        self.out.data = None;

        match (pins.address, pins.read) {
            (0x4016, false) => self.strobe = pins.data & 1 != 0,
            (0x4016, true) => self.out.data = Some(self.next_bit(0)),
            (0x4017, true) => self.out.data = Some(self.next_bit(1)),
            _ => (),
        }
    }

    fn next_bit(&mut self, pad: usize) -> u8 {
        let index = self.indices[pad];
        let buttons = self.pads[pad];

        if index >= 8 {
            1
        } else {
            self.indices[pad] += 1;
            let value = buttons[index] as u8;
            value
        }
    }

    pub fn out(&self) -> OutPins {
        self.out
    }

    pub fn set_button(&mut self, pad: u8, button: u8, pressed: bool) {
        let pad = pad as usize;
        let button = button as usize;
        self.pads[pad][button] = pressed;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InPins {
    pub cpu_m2: bool,
    pub address: u16,
    pub data: u8,
    pub read: bool,
}
impl InPins {
    pub fn init() -> Self {
        Self {
            cpu_m2: false,
            address: 0,
            data: 0,
            read: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct OutPins {
    pub data: Option<u8>,
}
impl OutPins {
    pub fn init() -> Self {
        Self { data: None }
    }
}
