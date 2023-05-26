use crate::nes::NesBus;

pub struct Joystick {
    last_m2: bool,

    pads: [[bool; 8]; 2],
    indices: [usize; 2],
    strobe: bool,
}
impl Joystick {
    pub fn new() -> Self {
        Self {
            last_m2: false,

            pads: [[false; 8]; 2],
            indices: [0; 2],
            strobe: false,
        }
    }

    pub fn master_cycle(&mut self, bus: &mut NesBus) {
        if self.strobe {
            self.indices = [0; 2];
        }
        self.service_cpu(bus);
        self.last_m2 = bus.cpu_m2;
    }

    fn service_cpu(&mut self, bus: &mut NesBus) {
        let m2_edge = self.last_m2 != bus.cpu_m2;
        if !m2_edge || !bus.cpu_m2 {
            return;
        }

        match (bus.cpu_address, bus.everyone_reads_cpu_bus()) {
            (0x4016, false) => self.strobe = bus.cpu_data & 1 != 0,
            (0x4016, true) => bus.cpu_data = self.next_bit(0),
            (0x4017, true) => bus.cpu_data = self.next_bit(1),
            _ => (),
        }
    }

    fn next_bit(&mut self, pad: usize) -> u8 {
        let index = self.indices[pad];
        let buttons = self.pads[pad];

        if index >= 8 {
            0x41
        } else {
            self.indices[pad] += 1;
            let value = if buttons[index] { 0x41 } else { 0x40 };
            value
        }
    }

    pub fn set_button(&mut self, pad: u8, button: u8, pressed: bool) {
        let pad = pad as usize;
        let button = button as usize;
        self.pads[pad][button] = pressed;
    }
}
