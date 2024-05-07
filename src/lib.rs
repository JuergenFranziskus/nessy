#![feature(bigint_helper_methods)]

use std::io::{self, Write};

use cpu_6502::{instruction::decode, Cpu};
use mapper::MapperBus;
use nesbus::CpuBus;
use ppu::{Ppu, PpuBus};
pub mod input;
pub mod mapper;
pub mod nesbus;
pub mod ppu;
pub mod apu;
mod util;

pub fn simple_debug(
    cycle: u64,
    cpu: &Cpu,
    bus: CpuBus,
    ppu: &Ppu,
    _ppu_bus: PpuBus,
    _mapper_bus: MapperBus,
    mut out: impl Write,
) -> io::Result<()> {
    write!(out, "{cycle:0>3}:    ")?;
    write!(out, "{} ", if bus.rst() { "RST" } else { "   " })?;
    write!(out, "{} ", if bus.nmi() { "NMI" } else { "   " })?;
    write!(out, "{} ", if bus.irq() { "IRQ" } else { "   " })?;
    write!(out, "{} ", if bus.not_ready() { "   " } else { "RDY" })?;
    write!(out, "{} ", if bus.halt() { "HLT" } else { "   " })?;
    write!(out, "{} ", if bus.sync() { "SYN" } else { "   " })?;

    write!(out, "  ")?;
    write!(out, "{:0>4x} ", bus.address())?;
    write!(out, "{}", if bus.read() { "R" } else { " " })?;
    write!(out, "{} ", if !bus.read() { "W" } else { " " })?;
    write!(out, "{:0>2x}", bus.data())?;

    if bus.sync() && !bus.halt() {
        let (op, mode) = decode(bus.data());
        write!(out, "  {op:?} {mode:<9}")?;
    } else {
        write!(out, "               ")?;
    }

    write!(out, "    ")?;
    write!(out, "A: {:0>2x}", cpu.a())?;
    write!(out, " | X: {:0>2x}", cpu.x())?;
    write!(out, " | Y: {:0>2x}", cpu.y())?;
    write!(out, " | SP: {:0>2x}", cpu.sp() & 0xFF)?;
    write!(out, " | PC: {:0>4x}", cpu.pc())?;

    let flags = cpu.flags();
    write!(out, "  ")?;
    write!(out, "{}", if flags.negative() { "N" } else { " " })?;
    write!(out, "{}", if flags.overflow() { "V" } else { " " })?;
    write!(out, "  ")?;
    write!(out, "{}", if flags.decimal() { "D" } else { " " })?;
    write!(out, "{}", if flags.irq_disable() { "I" } else { " " })?;
    write!(out, "{}", if flags.zero() { "Z" } else { " " })?;
    write!(out, "{}", if flags.carry() { "C" } else { " " })?;

    let [x, y] = ppu.dot();
    write!(out, "     DOT: {x:>3}|{y:<3}")?;

    writeln!(out)
}
