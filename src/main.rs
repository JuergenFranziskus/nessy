use ihex::{Reader, Record};
use nessy::cpu::{InPins, OutPins, CPU};

const REPORT_OUT: usize = 0xf001;
const REPORT_IN: usize = 0xf004;

fn main() {
    let mut memory = load_program();

    let (mut cpu, mut pins) = CPU::new();

    cpu.force_pc(0x400);
    cpu.force_fetch();
    pins.data = memory[0x400];

    for _ in 0.. {
        iteration(&mut pins, &mut cpu, &mut memory);
    }
}

fn iteration(pins: &mut InPins, cpu: &mut CPU, memory: &mut [u8]) -> OutPins {
    let out = cpu.cycle(*pins);
    let address = out.address as usize;

    if !out.read && address == REPORT_OUT {
        print!("{}", out.data as char);
    } else if !out.read {
        memory[address] = out.data;
    } else if address == REPORT_IN {
        pins.data = 'c' as u8;
    } else {
        pins.data = memory[address];
    }

    out
}

#[allow(dead_code)]
fn print_cycle_debug(cycle: isize, pins: InPins, out: OutPins, cpu: &CPU, print_instruction: bool) {
    let data = pins.data;
    let address = out.address;
    let rw = if out.read { "     " } else { "WRITE" };
    let sync = if out.sync { "SYNC" } else { "    " };

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
        "{cycle:0>4}: {rw} {sync} {address:0>4x} = {data:>2x}; \
        {instr:<14}     \
        A = {a:>2x}, X = {x:>2x}, Y = {y:>2x}, SP = {sp:>2x}, PC = {pc:>4x};  \
        {n}{v}  {d}{i}{z}{c}"
    );
}

fn load_program() -> [u8; 65536] {
    let src = std::fs::read_to_string("test_roms/klaus2m_functional.hex").unwrap();
    let mut memory = [0; 65536];

    let reader = Reader::new(&src);
    for record in reader {
        let record = record.unwrap();

        match record {
            Record::Data { offset, value } => {
                let start = offset as usize;
                let len = value.len();
                let end = start + len;
                let slice = &mut memory[start..end];
                slice.copy_from_slice(&value);
            }
            Record::EndOfFile => break,
            _ => unreachable!(),
        }
    }

    memory
}
