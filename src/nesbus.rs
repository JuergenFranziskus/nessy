use crate::{
    cpu::{
        instruction::{decode, AddrMode, Op},
        Bus6502, Cpu6502, CpuPins,
    },
    mapper::Mapper,
};
use std::io::{self, stderr, Write};

pub struct NesBus<M> {
    cycle: u64,
    mapper: M,

    ram: Box<[u8; 2048]>,
}
impl<M: Mapper> NesBus<M> {
    pub fn new(mapper: M) -> Self {
        Self {
            cycle: 0,
            mapper,

            ram: Box::new([0; 2048]),
        }
    }

    fn update_ram(&mut self, cpu: &mut CpuPins) {
        let addr = cpu.address() as usize;
        if addr >= 0x2000 {
            return;
        };
        let addr = addr % 2048;

        if cpu.read() {
            cpu.set_data(self.ram[addr]);
        } else {
            self.ram[addr] = cpu.data();
        }
    }

    pub fn cycle(&self) -> u64 {
        self.cycle
    }
    pub fn ram(&self) -> &[u8] {
        &*self.ram
    }
}
impl<M: Mapper> Bus6502 for NesBus<M> {
    fn cycle(&mut self, cpu: &mut Cpu6502) {
        self.mapper.cycle(cpu.pins_mut());
        self.update_ram(cpu.pins_mut());

        if DEBUG {
            print_debug(self.cycle, cpu.pins(), cpu).unwrap();
        }

        self.cycle += 1;
    }
}

const DEBUG: bool = false;
fn print_debug(cycle: u64, pins: CpuPins, cpu: &Cpu6502) -> io::Result<()> {
    let mut out = stderr();

    write!(out, "{cycle:0>3}: ")?;
    write!(out, "{} ", if pins.rst() { "RST" } else { "   " })?;
    write!(out, "{} ", if pins.irq() { "IRQ" } else { "   " })?;
    write!(out, "{} ", if pins.nmi() { "NMI" } else { "   " })?;
    write!(out, "{}", if pins.read() { "R" } else { " " })?;
    write!(out, "{}", if !pins.read() { "W" } else { " " })?;
    write!(out, "{}", if pins.sync() { "S" } else { " " })?;
    write!(out, "{}", if pins.halt() { "H" } else { " " })?;
    write!(out, "{}", if pins.not_ready() { " " } else { "Y" })?;
    write!(out, " {:0>4x}", pins.address())?;
    write!(out, " {:0>2x} ", pins.data())?;
    if pins.sync() {
        let int = cpu.is_doing_interrupt();
        let byte = pins.data();
        let (op, mode) = if int {
            (Op::BRK, AddrMode::Implied)
        } else {
            decode(byte)
        };
        write!(out, "{:>3?} {:<9}", op, mode)?;
    } else {
        write!(out, "             ")?;
    }
    let status = cpu.status();
    write!(out, "A: {:0>2x} | ", cpu.a())?;
    write!(out, "X: {:0>2x} | ", cpu.x())?;
    write!(out, "Y: {:0>2x} | ", cpu.y())?;
    write!(out, "SP: {:0>2x} | ", cpu.sp())?;
    write!(out, "PC: {:0>4x}    ", cpu.pc())?;
    write!(out, "{}", if status.carry() { "C" } else { " " })?;
    write!(out, "{}", if status.zero() { "Z" } else { " " })?;
    write!(out, "{}", if status.irq_disable() { "I" } else { " " })?;
    write!(out, "{}", if status.decimal() { "D" } else { " " })?;
    write!(out, "{}", if status.overflow() { "V" } else { " " })?;
    write!(out, "{}    ", if status.negative() { "N" } else { " " })?;

    writeln!(out)?;
    Ok(())
}
