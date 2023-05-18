use std::fmt::Display;

pub fn decode(byte: u8) -> (Opcode, AddressMode) {
    use AddressMode::*;
    use Opcode::*;
    match byte {
        0x00 => (BRK, Implied),
        0xd8 => (CLD, Implied),
        0xea => (NOP, Implied),
        0xa2 => (LDX, Immediate),
        0x96 => (STX, ZeroY),
        0xb6 => (LDX, ZeroY),
        0xbe => (LDX, AbsoluteY),
        0x9a => (TXS, Implied),
        0xba => (TSX, Implied),
        0x4c => (JMP, Absolute),
        0x6c => (JMP, Indirect),
        0x8a => (TXA, Implied),
        0xaa => (TAX, Implied),
        0xca => (DEX, Implied),
        0xa0 => (LDY, Immediate),
        0xc0 => (CPY, Immediate),
        0xe0 => (CPX, Immediate),
        0x20 => (JSR, Absolute),
        0x60 => (RTS, Implied),
        0x40 => (RTI, Implied),
        0xBC => (LDY, Absolute),
        _ => {
            let a = byte >> 5;
            let b = (byte >> 2) & 0b111;
            let c = byte & 0b11;

            if c == 1 && !(a == 4 && b == 2) {
                let opcode = match a {
                    0 => ORA,
                    1 => AND,
                    2 => EOR,
                    3 => ADC,
                    4 => STA,
                    5 => LDA,
                    6 => CMP,
                    7 => SBC,
                    _ => unreachable!(),
                };
                let mode = match b {
                    0 => IndirectX,
                    1 => Zero,
                    2 => Immediate,
                    3 => Absolute,
                    4 => IndirectY,
                    5 => ZeroX,
                    6 => AbsoluteY,
                    7 => AbsoluteX,
                    _ => unreachable!(),
                };
                return (opcode, mode);
            }
            if c == 2 && (b == 1 || b == 3 || (b == 2 && a < 4) || b == 5 || (b == 7 && a != 4)) {
                let opcode = match a {
                    0 => ASL,
                    1 => ROL,
                    2 => LSR,
                    3 => ROR,
                    4 => STX,
                    5 => LDX,
                    6 => DEC,
                    7 => INC,
                    _ => unreachable!(),
                };
                let mode = match b {
                    1 => Zero,
                    2 => Accumulator,
                    3 => Absolute,
                    5 => ZeroX,
                    7 => AbsoluteX,
                    _ => unreachable!(),
                };
                return (opcode, mode);
            }
            if c == 0
                && ((b == 3 && a != 0)
                    || (b == 1 && a >= 4)
                    || (b == 1 && a == 1)
                    || (b == 5 && a == 4)
                    || (b == 5 && a == 5))
            {
                let opcode = match a {
                    1 => BIT,
                    4 => STY,
                    5 => LDY,
                    6 => CPY,
                    7 => CPX,
                    _ => unreachable!(),
                };
                let mode = match b {
                    1 => Zero,
                    3 => Absolute,
                    5 => ZeroX,
                    _ => unreachable!(),
                };
                return (opcode, mode);
            }
            if c == 0 && b == 2 {
                let opcode = match a {
                    0 => PHP,
                    1 => PLP,
                    2 => PHA,
                    3 => PLA,
                    4 => DEY,
                    5 => TAY,
                    6 => INY,
                    7 => INX,
                    _ => unreachable!(),
                };

                return (opcode, Implied);
            }
            if c == 0 && b == 4 {
                let opcode = match a {
                    0 => BPL,
                    1 => BMI,
                    2 => BVC,
                    3 => BVS,
                    4 => BCC,
                    5 => BCS,
                    6 => BNE,
                    7 => BEQ,
                    _ => unreachable!(),
                };
                return (opcode, Relative);
            }
            if c == 0 && b == 6 {
                let opcode = match a {
                    0 => CLC,
                    1 => SEC,
                    2 => CLI,
                    3 => SEI,
                    4 => TYA,
                    5 => CLV,
                    6 => CLD,
                    7 => SED,
                    _ => unreachable!(),
                };
                return (opcode, Implied);
            }

            panic!("Unrecognized opcode {byte:x}: a = {a}, b = {b}, c = {c}")
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Opcode {
    ADC,
    AND,
    ASL,
    BCC,
    BCS,
    BEQ,
    BIT,
    BMI,
    BNE,
    BPL,
    BVC,
    BVS,
    BRK,
    CLC,
    CLD,
    CLI,
    CLV,
    CMP,
    CPX,
    CPY,
    DEC,
    DEX,
    DEY,
    EOR,
    INC,
    INX,
    INY,
    JMP,
    JSR,
    LDA,
    LDX,
    LDY,
    LSR,
    NOP,
    ORA,
    PHA,
    PHP,
    PLA,
    PLP,
    ROL,
    ROR,
    RTI,
    RTS,
    SBC,
    SEC,
    SED,
    SEI,
    STA,
    STX,
    STY,
    TAX,
    TAY,
    TSX,
    TXA,
    TXS,
    TYA,
}
impl Opcode {
    pub fn ignores_operand(self) -> bool {
        use Opcode::*;
        matches!(self, STA | STX | STY | JMP | JSR)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AddressMode {
    Implied,
    Accumulator,
    Immediate,
    Zero,
    ZeroX,
    ZeroY,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    Indirect,
    IndirectX,
    IndirectY,
    Relative,
}
impl Display for AddressMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use AddressMode::*;
        match self {
            Implied => write!(f, ""),
            Accumulator => write!(f, "acc"),
            Immediate => write!(f, "#"),
            Zero => write!(f, "[#]"),
            ZeroX => write!(f, "[# + x]"),
            ZeroY => write!(f, "[# + y]"),
            Absolute => write!(f, "[$]"),
            AbsoluteX => write!(f, "[$ + x]"),
            AbsoluteY => write!(f, "[$ + y]"),
            Indirect => write!(f, "[[$]]"),
            IndirectX => write!(f, "[[$ + x]]"),
            IndirectY => write!(f, "[[$] + y]"),
            Relative => write!(f, "#"),
        }
    }
}
