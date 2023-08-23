use crate::{nesbus::CpuBus, util::set_flag_u8};

pub struct Input {
    controllers: [Controller; 2],
    indices: [u8; 2],
    strobe: bool,
}
impl Input {
    pub fn init() -> Self {
        Self {
            controllers: [Controller(0); 2],
            indices: [0; 2],
            strobe: false,
        }
    }

    pub fn cycle(&mut self, cpu: &mut CpuBus) {
        self.strobe();
        self.handle_cpu(cpu);
    }
    fn strobe(&mut self) {
        if self.strobe {
            self.indices = [0; 2];
        }
    }

    fn handle_cpu(&mut self, cpu: &mut CpuBus) {
        if !cpu.read() {
            if cpu.address() != 0x4016 {
                return;
            };
            let strobe = cpu.data() & 1 != 0;
            self.strobe = strobe;
        } else {
            if cpu.address() != 0x4016 && cpu.address() != 0x4017 {
                return;
            };
            let port = (cpu.address() % 2) as usize;
            let index = self.indices[port];
            if index >= 8 {
                cpu.set_data(0x41);
                return;
            }
            let bit = self.controllers[port].0 & (1 << index) != 0;
            cpu.set_data(if bit { 0x41 } else { 0x40 });
            self.indices[port] += 1;
        }
    }

    pub fn controller_mut(&mut self, controller: u8) -> &mut Controller {
        &mut self.controllers[controller as usize]
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Controller(u8);
impl Controller {
    pub fn set_a(&mut self, a: bool) {
        set_flag_u8(&mut self.0, Self::A, a)
    }
    pub fn set_b(&mut self, a: bool) {
        set_flag_u8(&mut self.0, Self::B, a)
    }
    pub fn set_select(&mut self, a: bool) {
        set_flag_u8(&mut self.0, Self::SELECT, a)
    }
    pub fn set_start(&mut self, a: bool) {
        set_flag_u8(&mut self.0, Self::START, a)
    }
    pub fn set_up(&mut self, a: bool) {
        set_flag_u8(&mut self.0, Self::UP, a)
    }
    pub fn set_down(&mut self, a: bool) {
        set_flag_u8(&mut self.0, Self::DOWN, a)
    }
    pub fn set_left(&mut self, a: bool) {
        set_flag_u8(&mut self.0, Self::LEFT, a)
    }
    pub fn set_right(&mut self, a: bool) {
        set_flag_u8(&mut self.0, Self::RIGHT, a)
    }

    const A: u8 = 0;
    const B: u8 = 1;
    const SELECT: u8 = 2;
    const START: u8 = 3;
    const UP: u8 = 4;
    const DOWN: u8 = 5;
    const LEFT: u8 = 6;
    const RIGHT: u8 = 7;
}
