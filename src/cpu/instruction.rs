use std::fmt::Display;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op {
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
    BRK,
    BVC,
    BVS,
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

    // Illegal operations
    ALR,
    ANC,
    ANE,
    ARR,
    DCP,
    ISC,
    LAS,
    LAX,
    LXA,
    RLA,
    RRA,
    SAX,
    SBX,
    SHA,
    SHX,
    SHY,
    SLO,
    SRE,
    TAS,
    JAM,
}
impl Op {
    pub fn reads_operand(self) -> bool {
        use Op::*;
        self.is_rmw()
            || matches!(
                self,
                LDA | LDX | LDY | EOR | AND | ORA | ADC | SBC | CMP | CPX | CPY | BIT | LAX | NOP
            )
    }
    pub fn writes_operand(self) -> bool {
        use Op::*;
        self.is_rmw() || matches!(self, STA | STX | STY | SAX)
    }
    pub fn is_rmw(self) -> bool {
        use Op::*;
        matches!(
            self,
            ASL | LSR | ROL | ROR | INC | DEC | SLO | SRE | RLA | RRA | ISC | DCP | SHA | SHX | SHY
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AddrMode {
    Implied,
    Immediate,
    Relative,
    Accumulator,
    Zero,
    ZeroX,
    ZeroY,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    Indirect,
    XIndirect,
    IndirectY,
}
impl Display for AddrMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Self::Implied => "",
            Self::Immediate => "#",
            Self::Relative => "#",
            Self::Accumulator => "A",
            Self::Zero => "[#]",
            Self::ZeroX => "[#+X]",
            Self::ZeroY => "[#+Y]",
            Self::Absolute => "[$]",
            Self::AbsoluteX => "[$+X]",
            Self::AbsoluteY => "[$+Y]",
            Self::Indirect => "[[$]]",
            Self::XIndirect => "[[#+X]]",
            Self::IndirectY => "[[#] + Y]",
        };

        Display::fmt(str, f)
    }
}

pub fn decode(opcode: u8) -> (Op, AddrMode) {
    let a = opcode >> 5;
    let b = opcode >> 2 & 7;
    let c = opcode & 3;

    let addr_mode = decode_addr_mode(a, b, c);
    let op = decode_op(a, b, c);

    (op, addr_mode)
}

fn decode_op(a: u8, b: u8, c: u8) -> Op {
    match (a, b, c) {
        (0, 0, 0) => Op::BRK,
        (0, 2, 0) => Op::PHP,
        (0, 4, 0) => Op::BPL,
        (0, 6, 0) => Op::CLC,
        (0, _, 0) => Op::NOP,

        (1, 0, 0) => Op::JSR,
        (1, 1, 0) => Op::BIT,
        (1, 2, 0) => Op::PLP,
        (1, 3, 0) => Op::BIT,
        (1, 4, 0) => Op::BMI,
        (1, 5, 0) => Op::NOP,
        (1, 6, 0) => Op::SEC,
        (1, 7, 0) => Op::NOP,

        (2, 0, 0) => Op::RTI,
        (2, 1, 0) => Op::NOP,
        (2, 2, 0) => Op::PHA,
        (2, 3, 0) => Op::JMP,
        (2, 4, 0) => Op::BVC,
        (2, 5, 0) => Op::NOP,
        (2, 6, 0) => Op::CLI,
        (2, 7, 0) => Op::NOP,

        (3, 0, 0) => Op::RTS,
        (3, 1, 0) => Op::NOP,
        (3, 2, 0) => Op::PLA,
        (3, 3, 0) => Op::JMP,
        (3, 4, 0) => Op::BVS,
        (3, 5, 0) => Op::NOP,
        (3, 6, 0) => Op::SEI,
        (3, 7, 0) => Op::NOP,

        (4, 0, 0) => Op::NOP,
        (4, 1, 0) => Op::STY,
        (4, 2, 0) => Op::DEY,
        (4, 3, 0) => Op::STY,
        (4, 4, 0) => Op::BCC,
        (4, 5, 0) => Op::STY,
        (4, 6, 0) => Op::TYA,
        (4, 7, 0) => Op::SHY,

        (5, 0, 0) => Op::LDY,
        (5, 1, 0) => Op::LDY,
        (5, 2, 0) => Op::TAY,
        (5, 3, 0) => Op::LDY,
        (5, 4, 0) => Op::BCS,
        (5, 5, 0) => Op::LDY,
        (5, 6, 0) => Op::CLV,
        (5, 7, 0) => Op::LDY,

        (6, 0, 0) => Op::CPY,
        (6, 1, 0) => Op::CPY,
        (6, 2, 0) => Op::INY,
        (6, 3, 0) => Op::CPY,
        (6, 4, 0) => Op::BNE,
        (6, 5, 0) => Op::NOP,
        (6, 6, 0) => Op::CLD,
        (6, 7, 0) => Op::NOP,

        (7, 0, 0) => Op::CPX,
        (7, 1, 0) => Op::CPX,
        (7, 2, 0) => Op::INX,
        (7, 3, 0) => Op::CPX,
        (7, 4, 0) => Op::BEQ,
        (7, 5, 0) => Op::NOP,
        (7, 6, 0) => Op::SED,
        (7, 7, 0) => Op::NOP,

        (0, _, 1) => Op::ORA,
        (1, _, 1) => Op::AND,
        (2, _, 1) => Op::EOR,
        (3, _, 1) => Op::ADC,
        (4, 2, 1) => Op::NOP,
        (4, _, 1) => Op::STA,
        (5, _, 1) => Op::LDA,
        (6, _, 1) => Op::CMP,
        (7, _, 1) => Op::SBC,

        (_, 4, 2) => Op::JAM,
        (0..=3, 0, 2) => Op::JAM,
        (0..=3, 6, 2) => Op::NOP,
        (0, _, 2) => Op::ASL,
        (1, _, 2) => Op::ROL,
        (2, _, 2) => Op::LSR,
        (3, _, 2) => Op::ROR,

        (4, 0, 2) => Op::NOP,
        (4, 1, 2) => Op::STX,
        (4, 2, 2) => Op::TXA,
        (4, 3, 2) => Op::STX,
        (4, 5, 2) => Op::STX,
        (4, 6, 2) => Op::TXS,
        (4, 7, 2) => Op::SHX,

        (5, 0, 2) => Op::LDX,
        (5, 1, 2) => Op::LDX,
        (5, 2, 2) => Op::TAX,
        (5, 3, 2) => Op::LDX,
        (5, 5, 2) => Op::LDX,
        (5, 6, 2) => Op::TSX,
        (5, 7, 2) => Op::LDX,

        (6, 0, 2) => Op::NOP,
        (6, 1, 2) => Op::DEC,
        (6, 2, 2) => Op::DEX,
        (6, 3, 2) => Op::DEC,
        (6, 5, 2) => Op::DEC,
        (6, 6, 2) => Op::NOP,
        (6, 7, 2) => Op::DEC,

        (7, 0, 2) => Op::NOP,
        (7, 1, 2) => Op::INC,
        (7, 2, 2) => Op::NOP,
        (7, 3, 2) => Op::INC,
        (7, 5, 2) => Op::INC,
        (7, 6, 2) => Op::NOP,
        (7, 7, 2) => Op::INC,

        (0, 2, 3) => Op::ANC,
        (0, _, 3) => Op::SLO,

        (1, 2, 3) => Op::ANC,
        (1, _, 3) => Op::RLA,

        (2, 2, 3) => Op::ALR,
        (2, _, 3) => Op::SRE,

        (3, 2, 3) => Op::ARR,
        (3, _, 3) => Op::RRA,

        (4, 0, 3) => Op::SAX,
        (4, 1, 3) => Op::SAX,
        (4, 2, 3) => Op::ANE,
        (4, 3, 3) => Op::SAX,
        (4, 4, 3) => Op::SHA,
        (4, 5, 3) => Op::SAX,
        (4, 6, 3) => Op::TAX,
        (4, 7, 3) => Op::SHA,

        (5, 0, 3) => Op::LAX,
        (5, 1, 3) => Op::LAX,
        (5, 2, 3) => Op::LXA,
        (5, 3, 3) => Op::LAX,
        (5, 4, 3) => Op::LAX,
        (5, 5, 3) => Op::LAX,
        (5, 6, 3) => Op::LAS,
        (5, 7, 3) => Op::LAX,

        (6, 2, 3) => Op::SBX,
        (6, _, 3) => Op::DCP,

        (7, 2, 3) => Op::SBC,
        (7, _, 3) => Op::ISC,

        (_, _, 4..) | (_, 8.., _) | (8.., _, _) => unreachable!(),
    }
}
fn decode_addr_mode(a: u8, b: u8, c: u8) -> AddrMode {
    match (b, c) {
        (0, 0) if a == 1 => AddrMode::Absolute,
        (0, 0) if a < 4 => AddrMode::Implied,
        (0, 0) => AddrMode::Immediate,
        (0, 1) => AddrMode::XIndirect,
        (0, 2) if a < 4 => AddrMode::Implied,
        (0, 2) => AddrMode::Immediate,
        (0, 3) => AddrMode::XIndirect,

        (1, _) => AddrMode::Zero,

        (2, 0) => AddrMode::Implied,
        (2, 1) => AddrMode::Immediate,
        (2, 2) if a < 4 => AddrMode::Accumulator,
        (2, 2) => AddrMode::Implied,
        (2, 3) => AddrMode::Immediate,

        (3, 0) if a == 3 => AddrMode::Indirect,
        (3, _) => AddrMode::Absolute,

        (4, 0) => AddrMode::Relative,
        (4, 1) => AddrMode::IndirectY,
        (4, 2) => AddrMode::Implied,
        (4, 3) => AddrMode::IndirectY,

        (5, 2) if a == 4 || a == 5 => AddrMode::ZeroY,
        (5, 3) if a == 4 || a == 5 => AddrMode::ZeroY,
        (5, _) => AddrMode::ZeroX,

        (6, 0 | 2) => AddrMode::Implied,
        (6, 1 | 3) => AddrMode::AbsoluteY,

        (7, 0 | 1) => AddrMode::AbsoluteX,
        (7, 2 | 3) if a == 4 || a == 5 => AddrMode::AbsoluteY,
        (7, 2 | 3) => AddrMode::AbsoluteX,

        (_, 4..) | (8.., _) => unreachable!(),
    }
}
