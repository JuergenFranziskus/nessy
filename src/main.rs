use m6502::{core::Core, Bus};
use minifb::{Window, WindowOptions};
use nessy::{mapper::mapper0::Mapper0, nes::Nes, rom::Rom};

fn main() {
    run();
}

#[allow(dead_code)]
fn run() {
    let rom = std::fs::read("roms/DonkeyKong.nes").unwrap();
    let rom = Rom::parse(rom).unwrap();

    println!("{:#?}", rom.header);
    assert_eq!(rom.header.mapper, 0);
    assert_eq!(rom.header.submapper, 0);

    let mapper = Mapper0::new(rom);
    let mut nes = Nes::new(Box::new(mapper));
    let mut framebuffer = [0; 256 * 240];

    let opt = WindowOptions {
        resize: true,
        ..Default::default()
    };
    let mut window = Window::new("NES WOW", 640, 480, opt).unwrap();
    window.set_target_fps(60);

    while window.is_open() {
        run_until_not_nmi(&mut nes, &mut framebuffer);
        run_until_nmi(&mut nes, &mut framebuffer);
        window.update_with_buffer(&framebuffer, 256, 240).unwrap();
    }
}

fn run_until_nmi(nes: &mut Nes, framebuffer: &mut [u32; 256 * 240]) {
    while !nes.ppu.is_vblank() {
        clock(nes, framebuffer);
    }
}
fn run_until_not_nmi(nes: &mut Nes, framebuffer: &mut [u32; 256 * 240]) {
    while nes.ppu.is_vblank() {
        clock(nes, framebuffer);
    }
}

fn clock(nes: &mut Nes, framebuffer: &mut [u32; 256 * 240]) {
    let pixels = nes.clock();
    //print_debug(nes.cpu.cpu().core(), nes.cpu_bus);

    for (p, x, y) in pixels {
        let i = y * 256 + x;
        let i = i as usize;
        let p = p as usize * 3;
        let r = PALETTE[p + 0] as u32;
        let g = PALETTE[p + 1] as u32;
        let b = PALETTE[p + 2] as u32;
        let rgba = (r << 16) | (g << 8) | (b << 0);

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
