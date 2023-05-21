use nessy::{
    cpu::Cpu,
    mapper::nrom::NRom,
    nes::Nes,
    rom::Rom,
    processor::InPins as RPins,
    processor::OutPins,
};

fn main() {
    let rom_src = std::fs::read("roms/ZeldaScreen.nes").unwrap();
    let rom = Rom::parse(&rom_src);
    assert_eq!(rom.header.mapper, 0);
    let mapper = NRom::new(rom.prg_rom, rom.chr_rom, rom.header.mirroring);
    let mut nes = Nes::new(mapper);

    let mut print_instruction = true;
    for cycle in 0.. {
        nes.master_cycle();

        let debug = nes.cpu_cycles() == 0;
        let synced = nes.cpu().out().sync;
        if debug {
            nes.force_update_pins();
            print_cycle_debug(
                cycle,
                nes.processor_pins(),
                nes.processor().out(),
                nes.cpu(),
                print_instruction,
            );
            print_instruction = synced;
        }
    }
}

#[allow(dead_code)]
fn print_cycle_debug(cycle: isize, pins: RPins, out: OutPins, cpu: &Cpu, print_instruction: bool) {
    let data = if out.read { pins.data } else { out.data };
    let address = out.address;
    let rw = if out.read { "     " } else { "WRITE" };
    let sync = if out.sync { "SYNC" } else { "    " };
    let nmi = if pins.nmi { "NMI" } else { "   " };
    let irq = if pins.irq { "IRQ" } else { "   " };
    let reset = if pins.reset { "RST" } else { "   " };

    let a = cpu.a();
    let x = cpu.x();
    let y = cpu.y();
    let sp = cpu.sp();
    let pc = cpu.pc();

    let flags = cpu.flags();
    let c = if flags.carry { "C" } else { " " };
    let z = if flags.zero { "Z" } else { " " };
    let i = if flags.irq_disable { "I" } else { " " };
    let d = if flags.decimal { "D" } else { " " };
    let v = if flags.overflow { "V" } else { " " };
    let n = if flags.negative { "N" } else { " " };

    let instr = if print_instruction {
        format!("{:?} {}", cpu.opcode(), cpu.address_mode())
    } else {
        "".to_string()
    };

    println!(
        "{cycle:0>4}: {nmi} {irq} {reset} {rw} {sync} {address:0>4x} = {data:>2x}; \
        {instr:<14}     \
        A = {a:>2x}, X = {x:>2x}, Y = {y:>2x}, SP = {sp:>2x}, PC = {pc:>4x};  \
        {n}{v}  {d}{i}{z}{c}"
    );
}
