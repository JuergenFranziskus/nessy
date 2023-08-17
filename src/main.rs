use nessy::{cpu::Cpu6502, mapper::nrom::NRom, nesbus::NesBus, rom::Rom};
use std::time::Instant;

fn main() {
    let src = std::fs::read("./roms/DonkeyKong.nes").unwrap();
    let rom = Rom::parse(&src).unwrap();
    eprintln!("{:#?}", rom.header);
    assert!(rom.header.mapper == 0);
    assert!(rom.header.submapper == 0);
    let mapper = NRom::new(&rom);

    let mut cpu = Cpu6502::init();
    let mut bus = NesBus::new(mapper);

    let start = Instant::now();
    for _ in 0..1000000 {
        cpu.exec(&mut bus);
    }
    let took = start.elapsed();
    let cycles = bus.cycle();
    let per_second = (cycles as f64) / took.as_secs_f64();

    println!(
        "Ran an average of {:.2} megacycles per second",
        per_second / 1_000_000.0
    );
}
