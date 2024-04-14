use std::sync::Arc;

use cpu_6502::Cpu;
use nes_rom_parser::Rom;
use nessy::{
    input::Controller,
    mapper::{get_mapper, DynMapper},
    nesbus::NesBus,
};
use parking_lot::Mutex;
use winit::{
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

use crate::ROM_FILE;

pub struct App {
    pub window: Arc<Window>,
    pub cpu: Cpu,
    pub nesbus: NesBus<DynMapper>,
    pub ctrl_inputs: [Arc<Mutex<Controller>>; 2],
}
impl App {
    pub fn init() -> (App, EventLoop<()>) {
        let ev_loop = EventLoop::new().unwrap();
        let window = Arc::new(WindowBuilder::new().build(&ev_loop).unwrap());

        let (cpu, bus, ctrl_inputs) = start_nes();

        let app = Self {
            window,
            cpu,
            nesbus: bus,
            ctrl_inputs,
        };

        (app, ev_loop)
    }

    pub fn run_nes_until_vsync(&mut self) {
        let mut last_blank = self.nesbus.ppu().is_vblank();

        loop {
            let blank = self.nesbus.ppu().is_vblank();
            let pos_edge = blank && !last_blank;
            if pos_edge {
                break;
            };
            last_blank = blank;
            self.cpu.exec(&mut self.nesbus);
        }
    }
}

fn start_nes() -> (Cpu, NesBus<DynMapper>, [Arc<Mutex<Controller>>; 2]) {
    let src = std::fs::read(ROM_FILE).unwrap();
    let rom = Rom::parse(&src).unwrap();
    eprintln!("{:#?}", rom.header);
    let mapper = get_mapper(&rom);

    let rec_ctrl_0 = Arc::new(Mutex::new(Controller(0)));
    let rec_ctrl_1 = Arc::new(Mutex::new(Controller(0)));

    let cpu = Cpu::new();
    let bus = NesBus::new(mapper, [Arc::clone(&rec_ctrl_0), Arc::clone(&rec_ctrl_1)]);

    (cpu, bus, [rec_ctrl_0, rec_ctrl_1])
}
