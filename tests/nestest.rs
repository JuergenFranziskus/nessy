use cpu_6502::Cpu;
use nessy::{
    mapper::{nrom::NRom, MapperBus},
    nesbus::{CpuBus, NesBus},
    ppu::{Ppu, PpuBus},
    rom::Rom,
    simple_debug,
};
use std::{
    fs::{self, File},
    io::{stderr, BufRead, BufReader},
};

#[test]
pub fn nestest() {
    let log = File::open("test_roms/nestest_log.txt").unwrap();
    let log = BufReader::new(log);
    let lines = log.lines();

    let src = fs::read("test_roms/nestest.nes").unwrap();
    let rom = Rom::parse(&src).unwrap();
    let mut mapper = NRom::new(&rom);
    mapper.overwrite(0xFFFC, 0x00);
    mapper.overwrite(0xFFFD, 0xC0);

    let mut cpu = Cpu::new();
    let mut bus = NesBus::new(mapper, dummy_debug);

    // Run reset sequence
    cpu.exec(&mut bus);

    for line in lines {
        let line = line.unwrap();
        compare_state(&line, &cpu, &bus);
        cpu.exec(&mut bus);
    }

    println!("Tests are done");
}

const DEBUG: bool = false;
fn dummy_debug(
    cycle: u64,
    cpu: &Cpu,
    bus: CpuBus,
    ppu: &Ppu,
    ppu_bus: PpuBus,
    mapper_bus: MapperBus,
) {
    if !DEBUG {
        return;
    };
    simple_debug(cycle, cpu, bus, ppu, ppu_bus, mapper_bus, stderr()).unwrap();
}

fn compare_state(line: &str, cpu: &Cpu, bus: &NesBus<NRom>) {
    let should_pc = u16::from_str_radix(&line[0..4], 16).unwrap();
    let should_a = u8::from_str_radix(&line[50..52], 16).unwrap();
    let should_x = u8::from_str_radix(&line[55..57], 16).unwrap();
    let should_y = u8::from_str_radix(&line[60..62], 16).unwrap();
    let should_sp = u8::from_str_radix(&line[71..73], 16).unwrap();
    let should_dot_y: u16 = line[78..81]
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let should_dot_x: u16 = line[82..85]
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();

    assert_eq!(should_pc, cpu.pc());
    assert_eq!(should_a, cpu.a());
    assert_eq!(should_x, cpu.x());
    assert_eq!(should_y, cpu.y());
    assert_eq!(should_sp, cpu.sp() as u8);
    assert_eq!(should_dot_y, bus.ppu().dot()[1]);
    assert_eq!(should_dot_x, bus.ppu().dot()[0]);
}
