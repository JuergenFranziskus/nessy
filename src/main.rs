use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use m6502::core::Core;
use nessy::{apu::Bus, mapper::mapper0::Mapper0, nes::Nes, rom::Rom};
use spin_sleep::{sleep, sleep_until};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowAttributes},
};

use crate::render::Render;

mod render;

fn main() {
    let rom = std::fs::read("roms/DonkeyKong.nes").unwrap();
    let rom = Rom::parse(rom).unwrap();

    println!("{:#?}", rom.header);
    assert_eq!(rom.header.mapper, 0);
    assert_eq!(rom.header.submapper, 0);

    let mapper = Mapper0::new(rom);
    let nes = Nes::new(Box::new(mapper));

    let ev_loop = EventLoop::new().unwrap();
    let mut app = App::new(nes);

    ev_loop.run_app(&mut app).unwrap();
}

const FRAME_TIME: Duration = Duration::new(0, 1_000_000_000 / 144);
const NES_FRAME_TIME: Duration = Duration::new(0, 1_000_000_000 / 60);

struct App {
    init: Option<Init>,
    last_frame: Instant,
    last_nes_frame: Instant,

    nes: Nes,
    framebuffer: [u32; 256 * 240],
}
impl App {
    fn new(nes: Nes) -> Self {
        Self {
            init: None,
            last_frame: Instant::now(),
            last_nes_frame: Instant::now(),

            nes,
            framebuffer: [u32::MAX; _],
        }
    }

    fn update_render(&mut self) {
        self.update();
        self.render();
    }
    fn update(&mut self) {
        while self.last_nes_frame.elapsed() >= NES_FRAME_TIME {
            self.last_nes_frame += NES_FRAME_TIME;
            run_for_frame(&mut self.nes, &mut self.framebuffer);
        }
    }
    fn render(&mut self) {
        if let Some(init) = &mut self.init {
            init.render.render(&self.framebuffer);
            init.window.request_redraw();
        }

        let took = self.last_frame.elapsed();
        if took < FRAME_TIME {
            let sleep_for = FRAME_TIME - took;
            sleep(sleep_for);
        }
        self.last_frame = Instant::now();
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.init.is_none() {
            self.init = Some(Init::new(event_loop));
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => self
                .init
                .iter_mut()
                .for_each(|i| i.resize(size.width, size.height)),
            WindowEvent::RedrawRequested => self.update_render(),
            _ => (),
        }
    }
}

struct Init {
    window: Arc<Window>,
    render: Render,
}
impl Init {
    fn new(ev_loop: &ActiveEventLoop) -> Self {
        let attributes = WindowAttributes::default()
            .with_title("Nessy")
            .with_inner_size(PhysicalSize::new(1024, 720));
        let window = ev_loop.create_window(attributes).unwrap();
        let window = Arc::new(window);

        let render = Render::new(Arc::clone(&window));

        Self { window, render }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.render.resize(width, height);
    }
}

fn run_for_frame(nes: &mut Nes, framebuffer: &mut [u32]) {
    run_until_not_nmi(nes, framebuffer);
    run_until_nmi(nes, framebuffer);
}

fn run_until_nmi(nes: &mut Nes, framebuffer: &mut [u32]) {
    while !nes.ppu.is_vblank() {
        clock(nes, framebuffer);
    }
}
fn run_until_not_nmi(nes: &mut Nes, framebuffer: &mut [u32]) {
    while nes.ppu.is_vblank() {
        clock(nes, framebuffer);
    }
}

fn clock(nes: &mut Nes, framebuffer: &mut [u32]) {
    let pixels = nes.clock();
    //print_debug(nes.cpu.cpu().core(), nes.cpu_bus);

    for (p, x, y) in pixels {
        let i = y * 256 + x;
        let i = i as usize;
        let p = p as usize * 3;
        let r = PALETTE[p + 0] as u32;
        let g = PALETTE[p + 1] as u32;
        let b = PALETTE[p + 2] as u32;
        let rgba = (0xFF << 24) | (b << 16) | (g << 8) | (r << 0);

        if i < framebuffer.len() {
            framebuffer[i] = rgba;
        }
    }
}

static PALETTE: &[u8; 1536] = include_bytes!("nes_palette.pal");

fn print_debug(core: Core, bus: Bus) {
    print!("( ");

    let c = if core.p.c() { "C" } else { " " };
    let z = if core.p.z() { "Z" } else { " " };
    let i = if core.p.i() { "I" } else { " " };
    let d = if core.p.d() { "D" } else { " " };
    let b = if core.p.b() { "B" } else { " " };
    let o = if core.p.o() { "1" } else { " " };
    let v = if core.p.v() { "V" } else { " " };
    let n = if core.p.n() { "N" } else { " " };

    print!("A: {:0>2x} | ", core.a);
    print!("{n}{v}{o}{b}{d}{i}{z}{c} | ",);
    print!("PC: {:0>4x} | ", core.pc);
    print!("S: {:0>2x} | ", core.s);
    print!("X: {:0>2x} | ", core.x);
    print!("Y: {:0>2x}", core.y);

    print!(" )      ");

    let rw = if bus.rw() { "r" } else { "W" };
    let irq = if bus.irq() { "IRQ" } else { "   " };
    let nmi = if bus.nmi() { "NMI" } else { "   " };
    print!(
        "{irq} {nmi}   {:0>4x} {rw} {:0>2x}      ",
        bus.addr, bus.data
    );

    if bus.sync() {
        let (op, am) = m6502::instr::decode(bus.data);
        print!("┌ {op:?} {am}")
    } else {
        print!("│-");
    }

    println!();
}
