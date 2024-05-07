use std::sync::Arc;

use cpu_6502::Cpu;
use nes_rom_parser::Rom;
use nessy::{
    mapper::{get_mapper, DynMapper},
    nesbus::NesBus,
};
use winit::{
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

use crate::ROM_FILE;

pub struct App {
    pub window: Arc<Window>,
    pub cpu: Cpu,
    pub nesbus: NesBus<DynMapper>,
}
impl App {
    pub fn init() -> (App, EventLoop<()>) {
        let ev_loop = EventLoop::new().unwrap();
        let window = Arc::new(WindowBuilder::new().build(&ev_loop).unwrap());

        let (cpu, bus) = start_nes();

        let app = Self {
            window,
            cpu,
            nesbus: bus,
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

fn start_nes() -> (Cpu, NesBus<DynMapper>) {
    let src = std::fs::read(ROM_FILE).unwrap();
    let rom = Rom::parse(&src).unwrap();
    eprintln!("{:#?}", rom.header);
    let mapper = get_mapper(&rom);

    let cpu = Cpu::new();
    let bus = NesBus::new(mapper);

    (cpu, bus)
}
