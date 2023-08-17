use cpu_6502::{instruction::decode, Cpu};
use nessy::{
    mapper::nrom::NRom,
    nesbus::{CpuBus, NesBus},
    rom::Rom,
};

fn main() {
    let src = std::fs::read("./test_roms/nestest.nes").unwrap();
    let rom = Rom::parse(&src).unwrap();
    eprintln!("{:#?}", rom.header);
    assert!(rom.header.mapper == 0);
    assert!(rom.header.submapper == 0);
    let mut mapper = NRom::new(&rom);
    mapper.overwrite(0xFFFC, 0x00);
    mapper.overwrite(0xFFFD, 0xC0);

    let mut cpu = Cpu::new();
    let mut bus = NesBus::new(mapper, debug);

    for _ in 0..9100 {
        cpu.exec(&mut bus);
    }
}

fn debug(cycle: u64, cpu: &Cpu, bus: CpuBus) {
    print!("{cycle:0>3}:    ");
    print!("{} ", if bus.rst() { "RST" } else { "   " });
    print!("{} ", if bus.nmi() { "NMI" } else { "   " });
    print!("{} ", if bus.irq() { "IRQ" } else { "   " });
    print!("{} ", if bus.not_ready() { "   " } else { "RDY" });
    print!("{} ", if bus.halt() { "HLT" } else { "   " });
    print!("{} ", if bus.sync() { "SYN" } else { "   " });

    print!("  ");
    print!("{:0>4x} ", bus.address());
    print!("{}", if bus.read() { "R" } else { " " });
    print!("{} ", if !bus.read() { "W" } else { " " });
    print!("{:0>2x}", bus.data());

    if bus.sync() {
        let (op, mode) = decode(bus.data());
        print!("  {op:?} {mode:<9}");
    } else {
        print!("               ");
    }

    print!("    ");
    print!("A: {:0>2x}", cpu.a());
    print!(" | X: {:0>2x}", cpu.x());
    print!(" | Y: {:0>2x}", cpu.y());
    print!(" | SP: {:0>2x}", cpu.sp() & 0xFF);
    print!(" | PC: {:0>4x}", cpu.pc());

    let flags = cpu.flags();
    print!("  ");
    print!("{}", if flags.negative() { "N" } else { " " });
    print!("{}", if flags.overflow() { "V" } else { " " });
    print!("  ");
    print!("{}", if flags.decimal() { "D" } else { " " });
    print!("{}", if flags.irq_disable() { "I" } else { " " });
    print!("{}", if flags.zero() { "Z" } else { " " });
    print!("{}", if flags.carry() { "C" } else { " " });

    println!();
}
