use self::instruction::{decode, AddressMode, Opcode};

mod instruction;

#[derive(Copy, Clone, Debug)]
pub struct Cpu {
    a: u8,
    x: u8,
    y: u8,
    sp: u8,
    pc: u16,
    flags: Flags,

    last_nmi: bool,
    nmi_pending: bool,
    irq_pending: bool,

    state: State,
    out: OutPins,
}
impl Cpu {
    pub fn new() -> (Self, InPins) {
        let pins = InPins::init();
        let cpu = Self {
            a: 0,
            x: 0,
            y: 0,
            sp: 0,
            pc: 0,

            last_nmi: false,
            nmi_pending: false,
            irq_pending: false,

            flags: Flags::init(),
            state: State::init(),
            out: OutPins::init(),
        };

        (cpu, pins)
    }
    pub fn cycle(&mut self, pins: InPins) -> OutPins {
        self.out.read = true;
        self.out.halted = false;

        let mut backup = None;
        if !pins.ready {
            backup = Some(*self)
        }

        if self.out.sync {
            // If you're reading this later, reminder:
            // The interrupt logic goes in 'fetch', not here.
            self.decode(pins);
            self.out.sync = false;
        }

        self.exec(pins);
        self.poll_interrupts(pins);

        if !pins.ready && self.out.read {
            *self = backup.unwrap();
            self.out.halted = true;
        }

        self.out
    }
    fn poll_interrupts(&mut self, pins: InPins) {
        if !self.last_nmi && pins.nmi {
            self.nmi_pending = true;
        }
        self.irq_pending = pins.irq;

        self.last_nmi = pins.nmi;
    }

    pub fn out(&self) -> OutPins {
        self.out
    }

    fn decode(&mut self, pins: InPins) {
        let (opcode, address_mode) = decode(pins.data);
        self.sync_state(opcode, address_mode);
    }
    fn sync_state(&mut self, opcode: Opcode, address_mode: AddressMode) {
        self.state.address_mode_done = false;
        self.state.opcode = opcode;
        self.state.address_mode = address_mode;
        self.state.address_mode_cycles = 0;
        self.state.opcode_cycles = 0;
        self.state.break_mode = BreakMode::brk();
        self.state.page_crossed = false;
    }

    fn exec(&mut self, pins: InPins) {
        if !self.state.address_mode_done {
            self.state.address_mode_done = self.exec_address_mode(pins);
        }

        if self.state.address_mode_done {
            self.exec_opcode(pins);
        }
    }
    fn exec_address_mode(&mut self, pins: InPins) -> bool {
        use AddressMode::*;
        let done = match self.state.address_mode {
            Implied => true,
            Accumulator => true,
            Immediate => self.exec_immediate(),
            Zero => self.exec_zero(pins),
            ZeroX => self.exec_zero_offset(pins, self.x),
            ZeroY => self.exec_zero_offset(pins, self.y),
            Absolute => self.exec_absolute(pins),
            AbsoluteX => self.exec_absolute_offset(pins, self.x),
            AbsoluteY => self.exec_absolute_offset(pins, self.y),
            Indirect => self.exec_indirect(pins),
            IndirectX => self.exec_indirect_x(pins),
            IndirectY => self.exec_indirect_y(pins),
            Relative => self.exec_relative(pins),
        };
        self.state.address_mode_cycles += 1;
        done
    }
    fn exec_opcode(&mut self, pins: InPins) {
        use Opcode::*;
        match self.state.opcode {
            ADC => self.exec_adc(pins),
            AND => self.exec_and(pins),
            ASL => self.exec_asl(pins),
            BCC => self.exec_generic_branch(!self.flags.carry),
            BCS => self.exec_generic_branch(self.flags.carry),
            BEQ => self.exec_generic_branch(self.flags.zero),
            BIT => self.exec_bit(pins),
            BNE => self.exec_generic_branch(!self.flags.zero),
            BMI => self.exec_generic_branch(self.flags.negative),
            BPL => self.exec_generic_branch(!self.flags.negative),
            BVC => self.exec_generic_branch(!self.flags.overflow),
            BVS => self.exec_generic_branch(self.flags.overflow),
            BRK => self.exec_brk(pins),
            CLC => self.exec_clc(),
            CLD => self.exec_cld(),
            CLI => self.exec_cli(),
            CLV => self.exec_clv(),
            CMP => self.exec_compare(self.a, pins),
            CPX => self.exec_compare(self.x, pins),
            CPY => self.exec_compare(self.y, pins),
            DEC => self.exec_dec(pins),
            DEX => self.exec_dex(),
            DEY => self.exec_dey(),
            EOR => self.exec_eor(pins),
            INC => self.exec_inc(pins),
            INX => self.exec_inx(),
            INY => self.exec_iny(),
            JMP => self.exec_jmp(),
            JSR => self.exec_jsr(),
            LDA => self.exec_lda(pins),
            LDX => self.exec_ldx(pins),
            LDY => self.exec_ldy(pins),
            LSR => self.exec_lsr(pins),
            NOP => self.exec_nop(),
            ORA => self.exec_ora(pins),
            PHA => self.exec_pha(),
            PHP => self.exec_php(),
            PLA => self.exec_pla(pins),
            PLP => self.exec_plp(pins),
            SBC => self.exec_sbc(pins),
            SEC => self.exec_sec(),
            SED => self.exec_sed(),
            SEI => self.exec_sei(),
            STA => self.exec_sta(),
            STX => self.exec_stx(),
            STY => self.exec_sty(),
            TAX => self.exec_transfer(self.a, TransferTarget::X),
            TAY => self.exec_transfer(self.a, TransferTarget::Y),
            TSX => self.exec_transfer(self.sp, TransferTarget::X),
            TXS => self.exec_transfer(self.x, TransferTarget::S),
            TXA => self.exec_transfer(self.x, TransferTarget::A),
            TYA => self.exec_transfer(self.y, TransferTarget::A),
            ROL => self.exec_rol(pins),
            ROR => self.exec_ror(pins),
            RTI => self.exec_rti(pins),
            RTS => self.exec_rts(pins),
        }
        self.state.opcode_cycles += 1;
    }

    fn exec_immediate(&mut self) -> bool {
        match self.state.address_mode_cycles {
            0 => {
                self.read_pc_byte();
                false
            }
            1 => true,
            _ => unreachable!(),
        }
    }
    fn exec_zero(&mut self, pins: InPins) -> bool {
        match self.state.address_mode_cycles {
            0 => self.read_pc_byte(),
            1 => {
                self.start_address_operand(pins.data);
                if self.ignore_operand() {
                    return true;
                } else {
                    self.read(self.address());
                }
            }
            2 => return true,
            _ => unreachable!(),
        }

        false
    }
    fn exec_zero_offset(&mut self, pins: InPins, offset: u8) -> bool {
        match self.state.address_mode_cycles {
            0 => self.read_pc_byte(),
            1 => {
                self.start_address_operand(pins.data);
            }
            2 => {
                self.add_address(offset);
                if self.ignore_operand() {
                    return true;
                } else {
                    self.read(self.address() & 0xFF);
                }
            }
            3 => return true,
            _ => unreachable!(),
        }

        false
    }
    fn exec_absolute(&mut self, pins: InPins) -> bool {
        match self.state.address_mode_cycles {
            0 => {
                self.read_pc_byte();
                false
            }
            1 => {
                self.start_address_operand(pins.data);
                self.read_pc_byte();
                false
            }
            2 => {
                self.finish_address_operand(pins.data);
                let ignores = self.ignore_operand();
                if !ignores {
                    self.read_address();
                }
                ignores
            }
            3 => true,
            _ => unreachable!(),
        }
    }
    fn exec_absolute_offset(&mut self, pins: InPins, offset: u8) -> bool {
        match self.state.address_mode_cycles {
            0 => {
                self.read_pc_byte();
                false
            }
            1 => {
                self.start_address_operand(pins.data);
                self.read_pc_byte();
                false
            }
            2 => {
                self.finish_address_operand(pins.data);
                let carry = self.add_address(offset);
                if carry {
                    self.cross_page();
                }

                let ignores = self.ignore_operand();
                if !ignores && !self.page_crossed() {
                    self.read_address();
                }
                ignores
            }
            3 => {
                if self.page_crossed() {
                    self.read_address();
                    false
                } else {
                    true
                }
            }
            4 => true,
            _ => unreachable!(),
        }
    }
    fn exec_indirect(&mut self, pins: InPins) -> bool {
        match self.state.address_mode_cycles {
            0 => {
                self.read_pc_byte();
                false
            }
            1 => {
                self.start_address_operand(pins.data);
                self.read_pc_byte();
                false
            }
            2 => {
                self.finish_address_operand(pins.data);
                self.read_address();
                false
            }
            3 => {
                let [low, high] = self.address().to_le_bytes();
                let low = low.wrapping_add(1);
                let address = u16::from_le_bytes([low, high]);
                self.read(address);
                self.start_address_operand(pins.data);
                false
            }
            4 => {
                self.finish_address_operand(pins.data);
                true
            }
            _ => unreachable!(),
        }
    }
    fn exec_indirect_x(&mut self, pins: InPins) -> bool {
        match self.state.address_mode_cycles {
            0 => {
                self.read_pc_byte();
                false
            }
            1 => {
                self.start_address_operand(pins.data);
                self.add_address(self.x);
                false
            }
            2 => {
                self.read_address();
                false
            }
            3 => {
                let address = self.address();
                self.start_address_operand(pins.data);
                self.read((address + 1) & 0xFF);
                false
            }
            4 => {
                self.finish_address_operand(pins.data);
                let ignore = self.ignore_operand();
                if !ignore {
                    self.read_address();
                }
                ignore
            }
            5 => true,
            _ => unreachable!(),
        }
    }
    fn exec_indirect_y(&mut self, pins: InPins) -> bool {
        match self.state.address_mode_cycles {
            0 => {
                self.read_pc_byte();
                false
            }
            1 => {
                self.start_address_operand(pins.data);
                self.read_address();
                false
            }
            2 => {
                let address = self.address() + 1;
                self.start_address_operand(pins.data);
                self.read(address & 0xFF);
                false
            }
            3 => {
                self.finish_address_operand(pins.data);
                let carry = self.add_address(self.y);
                if carry {
                    self.cross_page()
                };

                let ignores = self.ignore_operand();
                if !ignores && !carry {
                    self.read_address();
                }
                ignores
            }
            4 => {
                if self.page_crossed() {
                    self.read_address();
                }
                !self.page_crossed()
            }
            5 => true,
            _ => unreachable!(),
        }
    }
    fn exec_relative(&mut self, pins: InPins) -> bool {
        match self.state.address_mode_cycles {
            0 => {
                self.read_pc_byte();
                false
            }
            1 => {
                let offset = pins.data.sign_cast() as i16;
                let address = self.pc.wrapping_add_signed(offset);
                if address & 0xFF00 != self.pc & 0xFF00 {
                    self.cross_page();
                }
                self.set_address(address);
                true
            }
            _ => unreachable!(),
        }
    }

    fn exec_adc(&mut self, pins: InPins) {
        assert!(
            !self.flags.decimal,
            "Decimal mode is not implemented (yet?)"
        );
        debug_assert_eq!(
            self.state.opcode_cycles, 0,
            "ADC should never be executed with cycle != 0"
        );

        let b = pins.data;
        let (a, carry) = self.a.carrying_add(b, self.flags.carry);

        let signed_a = self.a.sign_cast();
        let signed_b = b.sign_cast();
        let (_, overflow) = signed_a.carrying_add(signed_b, self.flags.carry);

        self.a = a;
        self.flags.carry = carry;
        self.flags.overflow = overflow;
        self.set_regular_flags(a);

        self.fetch();
    }
    fn exec_and(&mut self, pins: InPins) {
        debug_assert_eq!(
            self.state.opcode_cycles, 0,
            "AND should never be executed with cycle != 0"
        );

        self.a &= pins.data;
        self.set_regular_flags(self.a);

        self.fetch();
    }
    fn exec_asl(&mut self, pins: InPins) {
        let op = |x, flags: &mut Flags| {
            flags.carry = x & 128 != 0;
            x << 1
        };

        self.exec_rmw(pins, op);
    }
    fn exec_bit(&mut self, pins: InPins) {
        debug_assert_eq!(
            self.state.opcode_cycles, 0,
            "BIT should never be executed with cycle != 0"
        );

        let b = pins.data;
        self.flags.negative = b & 128 != 0;
        self.flags.overflow = b & 64 != 0;
        self.flags.zero = self.a & b == 0;

        self.fetch();
    }
    fn exec_generic_branch(&mut self, c: bool) {
        match self.state.opcode_cycles {
            0 => {
                if c {
                    self.pc = self.address();
                } else {
                    self.fetch();
                }
            }
            1 => {
                if !self.page_crossed() {
                    self.fetch();
                }
            }
            2 => self.fetch(),
            _ => unreachable!(),
        }
    }
    fn exec_brk(&mut self, pins: InPins) {
        match self.state.opcode_cycles {
            0 => {
                if self.state.break_mode.increment_pc {
                    self.pc += 1
                }
            }
            1 => {
                if self.state.break_mode.write {
                    self.push(self.pch());
                }
            }
            2 => {
                if self.state.break_mode.write {
                    self.push(self.pcl());
                }
            }
            3 => {
                let flags = self.flags.to_byte(self.state.break_mode.b_flag);
                if self.state.break_mode.write {
                    self.push(flags);
                }
            }
            4 => {
                self.read(self.state.break_mode.vector);
            }
            5 => {
                self.start_address_operand(pins.data);
                self.read(self.state.break_mode.vector + 1);
            }
            6 => {
                if self.state.break_mode.set_irq_disable {
                    self.flags.irq_disable = true;
                }
                self.finish_address_operand(pins.data);
                self.pc = self.state.address;
                self.fetch();
            }
            _ => unreachable!(),
        }
    }
    fn exec_clc(&mut self) {
        match self.state.opcode_cycles {
            0 => self.flags.carry = false,
            1 => self.fetch(),
            _ => unreachable!(),
        }
    }
    fn exec_cld(&mut self) {
        match self.state.opcode_cycles {
            0 => self.flags.decimal = false,
            1 => self.fetch(),
            _ => unreachable!(),
        }
    }
    fn exec_cli(&mut self) {
        match self.state.opcode_cycles {
            0 => self.flags.irq_disable = false,
            1 => self.fetch(),
            _ => unreachable!(),
        }
    }
    fn exec_clv(&mut self) {
        match self.state.opcode_cycles {
            0 => self.flags.overflow = false,
            1 => self.fetch(),
            _ => unreachable!(),
        }
    }
    fn exec_compare(&mut self, a: u8, pins: InPins) {
        match self.state.opcode_cycles {
            0 => {
                let b = pins.data;
                let (result, carry) = a.overflowing_sub(b);
                self.flags.carry = !carry;
                self.set_regular_flags(result);
                self.fetch();
            }
            _ => unreachable!(),
        }
    }
    fn exec_dec(&mut self, pins: InPins) {
        let op = |x: u8, _: &mut Flags| x.wrapping_sub(1);
        self.exec_rmw(pins, op);
    }
    fn exec_dex(&mut self) {
        match self.state.opcode_cycles {
            0 => {
                let value = self.x.wrapping_sub(1);
                self.set_regular_flags(value);
                self.x = value;
            }
            1 => self.fetch(),
            _ => unreachable!(),
        }
    }
    fn exec_dey(&mut self) {
        match self.state.opcode_cycles {
            0 => {
                let value = self.y.wrapping_sub(1);
                self.set_regular_flags(value);
                self.y = value;
            }
            1 => self.fetch(),
            _ => unreachable!(),
        }
    }
    fn exec_eor(&mut self, pins: InPins) {
        debug_assert_eq!(
            self.state.opcode_cycles, 0,
            "EOR should never be executed with cycle != 0"
        );

        self.a ^= pins.data;
        self.set_regular_flags(self.a);

        self.fetch();
    }
    fn exec_inc(&mut self, pins: InPins) {
        let op = |x: u8, _: &mut Flags| x.wrapping_add(1);
        self.exec_rmw(pins, op);
    }
    fn exec_inx(&mut self) {
        match self.state.opcode_cycles {
            0 => {
                let value = self.x.wrapping_add(1);
                self.set_regular_flags(value);
                self.x = value;
            }
            1 => self.fetch(),
            _ => unreachable!(),
        }
    }
    fn exec_iny(&mut self) {
        match self.state.opcode_cycles {
            0 => {
                let value = self.y.wrapping_add(1);
                self.set_regular_flags(value);
                self.y = value;
            }
            1 => self.fetch(),
            _ => unreachable!(),
        }
    }
    fn exec_jmp(&mut self) {
        debug_assert_eq!(self.state.opcode_cycles, 0);
        self.pc = self.state.address;
        self.fetch();
    }
    fn exec_jsr(&mut self) {
        match self.state.opcode_cycles {
            0 => {
                self.pc -= 1;
                self.push(self.pch());
            }
            1 => {
                self.push(self.pcl());
                self.pc = self.state.address;
            }
            2 => {
                self.fetch();
            }
            _ => unreachable!(),
        }
    }
    fn exec_lda(&mut self, pins: InPins) {
        match self.state.opcode_cycles {
            0 => {
                self.a = pins.data;
                self.set_regular_flags(self.a);
                self.fetch();
            }
            _ => unreachable!(),
        }
    }
    fn exec_ldx(&mut self, pins: InPins) {
        match self.state.opcode_cycles {
            0 => {
                self.x = pins.data;
                self.set_regular_flags(self.x);
                self.fetch();
            }
            _ => unreachable!(),
        }
    }
    fn exec_ldy(&mut self, pins: InPins) {
        match self.state.opcode_cycles {
            0 => {
                self.y = pins.data;
                self.set_regular_flags(self.y);
                self.fetch();
            }
            _ => unreachable!(),
        }
    }
    fn exec_lsr(&mut self, pins: InPins) {
        let op = |x, flags: &mut Flags| {
            flags.carry = x & 1 != 0;
            x >> 1
        };

        self.exec_rmw(pins, op);
    }
    fn exec_nop(&mut self) {
        match self.state.opcode_cycles {
            0 => (),
            1 => self.fetch(),
            _ => unreachable!(),
        }
    }
    fn exec_ora(&mut self, pins: InPins) {
        debug_assert_eq!(
            self.state.opcode_cycles, 0,
            "EOR should never be executed with cycle != 0"
        );

        self.a |= pins.data;
        self.set_regular_flags(self.a);

        self.fetch();
    }
    fn exec_pha(&mut self) {
        match self.state.opcode_cycles {
            0 => (),
            1 => {
                self.push(self.a);
            }
            2 => self.fetch(),
            _ => unreachable!(),
        }
    }
    fn exec_php(&mut self) {
        match self.state.opcode_cycles {
            0 => (),
            1 => {
                self.push(self.flags.to_byte(true));
            }
            2 => self.fetch(),
            _ => unreachable!(),
        }
    }
    fn exec_pla(&mut self, pins: InPins) {
        match self.state.opcode_cycles {
            0 => (),
            1 => {
                self.pop();
            }
            2 => {
                self.a = pins.data;
                self.set_regular_flags(self.a);
                self.fetch();
            }
            _ => unreachable!(),
        }
    }
    fn exec_plp(&mut self, pins: InPins) {
        match self.state.opcode_cycles {
            0 => (),
            1 => {
                self.pop();
            }
            2 => {
                self.flags = Flags::from_byte(pins.data);
                self.fetch();
            }
            _ => unreachable!(),
        }
    }
    fn exec_sbc(&mut self, pins: InPins) {
        assert!(
            !self.flags.decimal,
            "Decimal mode is not implemented (yet?)"
        );
        debug_assert_eq!(
            self.state.opcode_cycles, 0,
            "SBC should never be executed with cycle != 0"
        );

        let b = pins.data;
        let (a, carry) = self.a.borrowing_sub(b, self.flags.carry);

        let signed_a = self.a.sign_cast();
        let signed_b = b.sign_cast();
        let (_, overflow) = signed_a.borrowing_sub(signed_b, self.flags.carry);

        self.a = a;
        self.flags.carry = carry;
        self.flags.overflow = overflow;
        self.set_regular_flags(a);

        self.fetch();
    }
    fn exec_sec(&mut self) {
        match self.state.opcode_cycles {
            0 => self.flags.carry = true,
            1 => self.fetch(),
            _ => unreachable!(),
        }
    }
    fn exec_sed(&mut self) {
        match self.state.opcode_cycles {
            0 => self.flags.decimal = true,
            1 => self.fetch(),
            _ => unreachable!(),
        }
    }
    fn exec_sei(&mut self) {
        match self.state.opcode_cycles {
            0 => self.flags.irq_disable = true,
            1 => self.fetch(),
            _ => unreachable!(),
        }
    }
    fn exec_sta(&mut self) {
        match self.state.opcode_cycles {
            0 => {
                self.write(self.state.address, self.a);
            }
            1 => self.fetch(),
            _ => unreachable!(),
        }
    }
    fn exec_stx(&mut self) {
        match self.state.opcode_cycles {
            0 => {
                self.write(self.state.address, self.x);
            }
            1 => self.fetch(),
            _ => unreachable!(),
        }
    }
    fn exec_sty(&mut self) {
        match self.state.opcode_cycles {
            0 => {
                self.write(self.state.address, self.y);
            }
            1 => self.fetch(),
            _ => unreachable!(),
        }
    }
    fn exec_transfer(&mut self, value: u8, into: TransferTarget) {
        match self.state.opcode_cycles {
            0 => {
                let target = match into {
                    TransferTarget::A => &mut self.a,
                    TransferTarget::X => &mut self.x,
                    TransferTarget::Y => &mut self.y,
                    TransferTarget::S => &mut self.sp,
                };
                *target = value;

                let affect_flags = into != TransferTarget::S;
                if affect_flags {
                    self.set_regular_flags(value);
                }
            }
            1 => self.fetch(),
            _ => unreachable!(),
        }
    }
    fn exec_rol(&mut self, pins: InPins) {
        let op = |x, flags: &mut Flags| {
            let old_carry = flags.carry as u8;
            flags.carry = x & 128 != 0;
            (x << 1) & old_carry
        };

        self.exec_rmw(pins, op);
    }
    fn exec_ror(&mut self, pins: InPins) {
        let op = |x, flags: &mut Flags| {
            let old_carry = (flags.carry as u8) << 7;
            flags.carry = x & 1 != 0;
            (x >> 1) & old_carry
        };

        self.exec_rmw(pins, op);
    }
    fn exec_rts(&mut self, pins: InPins) {
        match self.state.opcode_cycles {
            0 => (),
            1 => (),
            2 => {
                self.pop();
            }
            3 => {
                self.start_address_operand(pins.data);
                self.pop();
            }
            4 => {
                self.finish_address_operand(pins.data);
            }
            5 => {
                self.pc = self.address() + 1;
                self.fetch();
            }
            _ => unreachable!(),
        }
    }
    fn exec_rti(&mut self, pins: InPins) {
        match self.state.opcode_cycles {
            0 => (),
            1 => (),
            2 => {
                self.pop();
            }
            3 => {
                self.flags = Flags::from_byte(pins.data);
                self.pop();
            }
            4 => {
                self.start_address_operand(pins.data);
                self.pop();
            }
            5 => {
                self.finish_address_operand(pins.data);
                self.pc = self.address();
                self.fetch();
            }
            _ => unreachable!(),
        }
    }

    fn exec_rmw(&mut self, pins: InPins, op: fn(u8, &mut Flags) -> u8) {
        if self.state.address_mode == AddressMode::Accumulator {
            match self.state.opcode_cycles {
                0 => {
                    self.a = op(self.a, &mut self.flags);
                    self.set_regular_flags(self.a);
                }
                1 => self.fetch(),
                _ => unreachable!(),
            }
        } else {
            match self.state.opcode_cycles {
                0 => {
                    self.write(self.address(), pins.data);
                }
                1 => {
                    let old_value = self.out.data;
                    let value = op(old_value, &mut self.flags);
                    self.set_regular_flags(value);
                    self.write(self.address(), value);
                }
                2 => self.fetch(),
                _ => unreachable!(),
            }
        }
    }

    fn set_regular_flags(&mut self, value: u8) {
        self.flags.zero = value == 0;
        self.flags.negative = value > 127;
    }

    fn read(&mut self, address: u16) {
        self.out.address = address;
    }
    fn write(&mut self, address: u16, value: u8) {
        self.out.address = address;
        self.out.data = value;
        self.out.read = false;
    }
    fn push(&mut self, value: u8) {
        let address = 0x100 + self.sp as u16;
        self.sp = self.sp.wrapping_sub(1);
        self.write(address, value);
    }
    fn pop(&mut self) {
        self.sp = self.sp.wrapping_add(1);
        let address = 0x100 + self.sp as u16;
        self.read(address);
    }
    fn fetch(&mut self) {
        if self.nmi_pending {
            self.nmi_pending = false;
            self.sync_state(Opcode::BRK, AddressMode::Implied);
            self.state.break_mode = BreakMode::nmi();
        } else if self.irq_pending && !self.flags.irq_disable {
            self.sync_state(Opcode::BRK, AddressMode::Implied);
            self.state.break_mode = BreakMode::irq();
        } else {
            self.read_pc_byte();
            self.out.sync = true;
        }
    }
    fn read_pc_byte(&mut self) {
        self.read(self.pc);
        self.pc += 1;
    }
    fn start_address_operand(&mut self, byte: u8) {
        self.state.address = byte as u16;
    }
    fn finish_address_operand(&mut self, byte: u8) {
        self.state.address |= (byte as u16) << 8;
    }
    fn add_address(&mut self, offset: u8) -> bool {
        let new_address = self.address().wrapping_add(offset as u16);
        let carry = new_address & 0xFF00 != self.address() & 0xFF00;
        self.set_address(new_address);
        carry
    }
    fn pcl(&self) -> u8 {
        (self.pc & 0xFF) as u8
    }
    fn pch(&self) -> u8 {
        (self.pc >> 8) as u8
    }
    fn ignore_operand(&self) -> bool {
        self.state.opcode.ignores_operand()
    }
    fn page_crossed(&self) -> bool {
        self.state.page_crossed
    }
    fn cross_page(&mut self) {
        self.state.page_crossed = true;
    }
    fn address(&self) -> u16 {
        self.state.address
    }
    fn set_address(&mut self, address: u16) {
        self.state.address = address;
    }
    fn read_address(&mut self) {
        self.read(self.address());
    }

    pub fn a(&self) -> u8 {
        self.a
    }
    pub fn x(&self) -> u8 {
        self.x
    }
    pub fn y(&self) -> u8 {
        self.y
    }
    pub fn sp(&self) -> u8 {
        self.sp
    }
    pub fn pc(&self) -> u16 {
        self.pc
    }
    pub fn flags(&self) -> Flags {
        self.flags
    }
    pub fn opcode(&self) -> Opcode {
        self.state.opcode
    }
    pub fn address_mode(&self) -> AddressMode {
        self.state.address_mode
    }

    pub fn force_pc(&mut self, value: u16) {
        self.pc = value;
    }
    pub fn force_fetch(&mut self) {
        self.fetch();
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Flags {
    pub carry: bool,
    pub zero: bool,
    pub irq_disable: bool,
    pub decimal: bool,
    pub overflow: bool,
    pub negative: bool,
}
impl Flags {
    fn init() -> Flags {
        Self {
            carry: false,
            overflow: false,
            zero: false,
            negative: false,
            decimal: false,
            irq_disable: true,
        }
    }

    fn to_byte(self, b_flag: bool) -> u8 {
        let mut byte = 0;
        byte |= self.carry as u8;
        byte |= (self.zero as u8) << 1;
        byte |= (self.irq_disable as u8) << 2;
        byte |= (self.decimal as u8) << 3;
        byte |= (b_flag as u8) << 4;
        byte |= 1 << 5;
        byte |= (self.overflow as u8) << 6;
        byte |= (self.negative as u8) << 7;

        byte
    }
    fn from_byte(byte: u8) -> Self {
        Self {
            carry: byte & 1 != 0,
            zero: byte & 2 != 0,
            irq_disable: byte & 4 != 0,
            decimal: byte & 8 != 0,
            overflow: byte & 64 != 0,
            negative: byte & 128 != 0,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct BreakMode {
    vector: u16,
    b_flag: bool,
    increment_pc: bool,
    write: bool,
    set_irq_disable: bool,
}
impl BreakMode {
    fn brk() -> Self {
        Self {
            vector: 0xFFFE,
            b_flag: true,
            write: true,
            set_irq_disable: true,
            increment_pc: true,
        }
    }
    fn irq() -> Self {
        Self {
            vector: 0xFFFE,
            b_flag: false,
            write: true,
            set_irq_disable: true,
            increment_pc: false,
        }
    }
    fn nmi() -> Self {
        Self {
            vector: 0xFFFA,
            b_flag: false,
            write: true,
            set_irq_disable: false,
            increment_pc: false,
        }
    }
    fn reset() -> Self {
        Self {
            vector: 0xFFFC,
            b_flag: false,
            write: false,
            set_irq_disable: true,
            increment_pc: false,
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct State {
    break_mode: BreakMode,
    /// The number of cycles that the relevant addressing mode has been executing for.
    address_mode_cycles: u8,
    /// The number of cycles that the relevant instruction sans addressing mode has been executing for.
    opcode_cycles: u8,

    address_mode_done: bool,
    opcode: Opcode,
    address_mode: AddressMode,

    address: u16,
    page_crossed: bool,
}
impl State {
    fn init() -> Self {
        Self {
            break_mode: BreakMode::reset(),
            address_mode_cycles: 0,
            opcode_cycles: 0,
            address_mode_done: true,
            opcode: Opcode::BRK,
            address_mode: AddressMode::Implied,
            address: 0,
            page_crossed: false,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct OutPins {
    pub data: u8,
    pub address: u16,
    pub read: bool,
    pub sync: bool,
    pub halted: bool,
}
impl OutPins {
    pub fn init() -> OutPins {
        Self {
            data: 0,
            address: 0,
            read: true,
            sync: false,
            halted: false,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct InPins {
    pub data: u8,
    pub ready: bool,
    pub reset: bool,
    pub irq: bool,
    pub nmi: bool,
    pub so: bool,
}
impl InPins {
    pub fn init() -> Self {
        Self {
            data: 0,
            ready: true,
            reset: false,
            irq: false,
            nmi: false,
            so: false,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum TransferTarget {
    A,
    X,
    Y,
    S,
}

trait SignCast {
    type FlippedType;
    fn sign_cast(self) -> Self::FlippedType;
}

impl SignCast for u8 {
    type FlippedType = i8;
    fn sign_cast(self) -> Self::FlippedType {
        i8::from_le_bytes(self.to_le_bytes())
    }
}
impl SignCast for i8 {
    type FlippedType = u8;
    fn sign_cast(self) -> Self::FlippedType {
        u8::from_le_bytes(self.to_le_bytes())
    }
}
