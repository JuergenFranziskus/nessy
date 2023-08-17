use nessy::{
    cpu::{Cpu6502, Status},
    mapper::nrom::NRom,
    nesbus::NesBus,
    rom::Rom,
};
use std::io::{BufRead, BufReader};

#[test]
fn nestest() {
    let log = std::fs::File::open("test_roms/nestest_log.txt").unwrap();
    let log = BufReader::new(log);
    let lines = log.lines();

    let src = std::fs::read("test_roms/nestest.nes").unwrap();
    let rom = Rom::parse(&src).unwrap();
    assert_eq!(rom.header.mapper, 0);
    assert_eq!(rom.header.submapper, 0);

    let mut mapper = NRom::new(&rom);

    // Override the reset vector to start execution in automatic mode
    mapper.overwrite(0xFFFC, 0x00);
    mapper.overwrite(0xFFFD, 0xC0);

    let mut bus = NesBus::new(mapper);
    let mut cpu = Cpu6502::init();
    // Perform reset sequence
    cpu.exec(&mut bus);

    for line in lines {
        let line = line.unwrap();
        compare_debug_line(&line, &cpu, &bus);
        cpu.exec(&mut bus);

        if cpu.jammed() {
            eprintln!("CPU jammed on cycle {}", bus.cycle());
            break;
        }
    }

    let ram = bus.ram();
    let res_0 = ram[2];
    let res_1 = ram[3];

    assert_eq!(res_0, 0, "First set of tests failed with {res_0:x}");
    assert_eq!(res_1, 0, "Second set of tests failed with {res_1:x}");
}

fn compare_debug_line(line: &str, cpu: &Cpu6502, _bus: &NesBus<NRom>) {
    let pc = u16::from_str_radix(&line[0..4], 16).unwrap();
    assert_eq!(pc, cpu.pc());

    let a = u8::from_str_radix(&line[50..52], 16).unwrap();
    assert_eq!(a, cpu.a());

    let x = u8::from_str_radix(&line[55..57], 16).unwrap();
    assert_eq!(x, cpu.x());

    let y = u8::from_str_radix(&line[60..62], 16).unwrap();
    assert_eq!(y, cpu.y());

    let status = u8::from_str_radix(&line[65..67], 16).unwrap();
    let status = Status::from_pushable_bits(status);
    assert_eq!(
        status,
        cpu.status(),
        "Should {:b}, was {:b}",
        status.bits(),
        cpu.status().bits()
    );

    let sp = u8::from_str_radix(&line[71..73], 16).unwrap();
    assert_eq!(sp, cpu.sp());
}
