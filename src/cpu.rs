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
}
impl Cpu {
    pub fn new() -> Self {
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
        };

        cpu
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
}
impl Cpu {
    pub fn cycle(&mut self, bus: &mut impl CpuBus) {
        bus.set_read(true);
        bus.set_halted(false);

        let mut backup = None;
        if !bus.ready() {
            backup = Some((*self, bus.backup()));
        }

        if bus.sync() {
            // If you're reading this later, reminder:
            // The interrupt logic goes in 'fetch', not here.
            self.decode(bus);
            bus.set_sync(false);
        }

        self.exec(bus);
        self.poll_interrupts(bus);

        if !bus.ready() && bus.read() {
            let (new_self, bus_backup) = backup.unwrap();
            *self = new_self;
            bus.restore(bus_backup);
            bus.set_halted(true);
        }
    }
    fn poll_interrupts(&mut self, bus: &mut impl CpuBus) {
        if !self.last_nmi && bus.nmi() {
            self.nmi_pending = true;
        }
        self.irq_pending = bus.irq();

        self.last_nmi = bus.nmi();
    }

    fn decode(&mut self, bus: &mut impl CpuBus) {
        let (opcode, address_mode) = decode(bus.data());
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

    fn exec(&mut self, bus: &mut impl CpuBus) {
        if !self.state.address_mode_done {
            self.state.address_mode_done = self.exec_address_mode(bus);
        }

        if self.state.address_mode_done {
            self.exec_opcode(bus);
        }
    }
    fn exec_address_mode(&mut self, bus: &mut impl CpuBus) -> bool {
        use AddressMode::*;
        let done = match self.state.address_mode {
            Implied => true,
            Accumulator => true,
            Immediate => self.exec_immediate(bus),
            Zero => self.exec_zero(bus),
            ZeroX => self.exec_zero_offset(bus, self.x),
            ZeroY => self.exec_zero_offset(bus, self.y),
            Absolute => self.exec_absolute(bus),
            AbsoluteX => self.exec_absolute_offset(bus, self.x),
            AbsoluteY => self.exec_absolute_offset(bus, self.y),
            Indirect => self.exec_indirect(bus),
            IndirectX => self.exec_indirect_x(bus),
            IndirectY => self.exec_indirect_y(bus),
            Relative => self.exec_relative(bus),
        };
        self.state.address_mode_cycles += 1;
        done
    }
    fn exec_opcode(&mut self, bus: &mut impl CpuBus) {
        use Opcode::*;
        match self.state.opcode {
            ADC => self.exec_adc(bus),
            AND => self.exec_and(bus),
            ASL => self.exec_asl(bus),
            BCC => self.exec_generic_branch(!self.flags.carry, bus),
            BCS => self.exec_generic_branch(self.flags.carry, bus),
            BEQ => self.exec_generic_branch(self.flags.zero, bus),
            BIT => self.exec_bit(bus),
            BNE => self.exec_generic_branch(!self.flags.zero, bus),
            BMI => self.exec_generic_branch(self.flags.negative, bus),
            BPL => self.exec_generic_branch(!self.flags.negative, bus),
            BVC => self.exec_generic_branch(!self.flags.overflow, bus),
            BVS => self.exec_generic_branch(self.flags.overflow, bus),
            BRK => self.exec_brk(bus),
            CLC => self.exec_clc(bus),
            CLD => self.exec_cld(bus),
            CLI => self.exec_cli(bus),
            CLV => self.exec_clv(bus),
            CMP => self.exec_compare(self.a, bus),
            CPX => self.exec_compare(self.x, bus),
            CPY => self.exec_compare(self.y, bus),
            DEC => self.exec_dec(bus),
            DEX => self.exec_dex(bus),
            DEY => self.exec_dey(bus),
            EOR => self.exec_eor(bus),
            INC => self.exec_inc(bus),
            INX => self.exec_inx(bus),
            INY => self.exec_iny(bus),
            JMP => self.exec_jmp(bus),
            JSR => self.exec_jsr(bus),
            LDA => self.exec_lda(bus),
            LDX => self.exec_ldx(bus),
            LDY => self.exec_ldy(bus),
            LSR => self.exec_lsr(bus),
            NOP => self.exec_nop(bus),
            ORA => self.exec_ora(bus),
            PHA => self.exec_pha(bus),
            PHP => self.exec_php(bus),
            PLA => self.exec_pla(bus),
            PLP => self.exec_plp(bus),
            SBC => self.exec_sbc(bus),
            SEC => self.exec_sec(bus),
            SED => self.exec_sed(bus),
            SEI => self.exec_sei(bus),
            STA => self.exec_sta(bus),
            STX => self.exec_stx(bus),
            STY => self.exec_sty(bus),
            TAX => self.exec_transfer(self.a, TransferTarget::X, bus),
            TAY => self.exec_transfer(self.a, TransferTarget::Y, bus),
            TSX => self.exec_transfer(self.sp, TransferTarget::X, bus),
            TXS => self.exec_transfer(self.x, TransferTarget::S, bus),
            TXA => self.exec_transfer(self.x, TransferTarget::A, bus),
            TYA => self.exec_transfer(self.y, TransferTarget::A, bus),
            ROL => self.exec_rol(bus),
            ROR => self.exec_ror(bus),
            RTI => self.exec_rti(bus),
            RTS => self.exec_rts(bus),
        }
        self.state.opcode_cycles += 1;
    }

    fn exec_immediate(&mut self, bus: &mut impl CpuBus) -> bool {
        match self.state.address_mode_cycles {
            0 => {
                self.read_pc_byte(bus);
                false
            }
            1 => true,
            _ => unreachable!(),
        }
    }
    fn exec_zero(&mut self, bus: &mut impl CpuBus) -> bool {
        match self.state.address_mode_cycles {
            0 => self.read_pc_byte(bus),
            1 => {
                self.start_address_operand(bus.data());
                if self.ignore_operand() {
                    return true;
                } else {
                    self.read_address(bus);
                }
            }
            2 => return true,
            _ => unreachable!(),
        }

        false
    }
    fn exec_zero_offset(&mut self, bus: &mut impl CpuBus, offset: u8) -> bool {
        match self.state.address_mode_cycles {
            0 => self.read_pc_byte(bus),
            1 => {
                self.start_address_operand(bus.data().wrapping_add(offset));
            }
            2 => {
                if self.ignore_operand() {
                    return true;
                } else {
                    self.read(self.address() & 0xFF, bus);
                }
            }
            3 => return true,
            _ => unreachable!(),
        }

        false
    }
    fn exec_absolute(&mut self, bus: &mut impl CpuBus) -> bool {
        match self.state.address_mode_cycles {
            0 => {
                self.read_pc_byte(bus);
                false
            }
            1 => {
                self.start_address_operand(bus.data());
                self.read_pc_byte(bus);
                false
            }
            2 => {
                self.finish_address_operand(bus.data());
                let ignores = self.ignore_operand();
                if !ignores {
                    self.read_address(bus);
                }
                ignores
            }
            3 => true,
            _ => unreachable!(),
        }
    }
    fn exec_absolute_offset(&mut self, bus: &mut impl CpuBus, offset: u8) -> bool {
        match self.state.address_mode_cycles {
            0 => {
                self.read_pc_byte(bus);
                false
            }
            1 => {
                self.start_address_operand(bus.data());
                self.read_pc_byte(bus);
                false
            }
            2 => {
                self.finish_address_operand(bus.data());
                let carry = self.add_address(offset);
                if carry {
                    self.cross_page();
                }

                let ignores = self.ignore_operand();
                if !ignores {
                    self.read_address(bus);
                }
                ignores
            }
            3 => {
                if self.page_crossed() {
                    self.read_address(bus);
                    false
                } else {
                    true
                }
            }
            4 => true,
            _ => unreachable!(),
        }
    }
    fn exec_indirect(&mut self, bus: &mut impl CpuBus) -> bool {
        match self.state.address_mode_cycles {
            0 => {
                self.read_pc_byte(bus);
                false
            }
            1 => {
                self.start_address_operand(bus.data());
                self.read_pc_byte(bus);
                false
            }
            2 => {
                self.finish_address_operand(bus.data());
                self.read_address(bus);
                false
            }
            3 => {
                let [low, high] = self.address().to_le_bytes();
                let low = low.wrapping_add(1);
                let address = u16::from_le_bytes([low, high]);
                self.read(address, bus);
                self.start_address_operand(bus.data());
                false
            }
            4 => {
                self.finish_address_operand(bus.data());
                true
            }
            _ => unreachable!(),
        }
    }
    fn exec_indirect_x(&mut self, bus: &mut impl CpuBus) -> bool {
        match self.state.address_mode_cycles {
            0 => {
                self.read_pc_byte(bus);
                false
            }
            1 => {
                self.start_address_operand(bus.data().wrapping_add(self.x));
                false
            }
            2 => {
                self.read_address(bus);
                false
            }
            3 => {
                let address = self.address();
                self.start_address_operand(bus.data());
                self.read((address + 1) & 0xFF, bus);
                false
            }
            4 => {
                self.finish_address_operand(bus.data());
                let ignore = self.ignore_operand();
                if !ignore {
                    self.read_address(bus);
                }
                ignore
            }
            5 => true,
            _ => unreachable!(),
        }
    }
    fn exec_indirect_y(&mut self, bus: &mut impl CpuBus) -> bool {
        match self.state.address_mode_cycles {
            0 => {
                self.read_pc_byte(bus);
                false
            }
            1 => {
                self.start_address_operand(bus.data());
                self.read_address(bus);
                false
            }
            2 => {
                let address = self.address() + 1;
                self.start_address_operand(bus.data());
                self.read(address & 0xFF, bus);
                false
            }
            3 => {
                self.finish_address_operand(bus.data());
                let carry = self.add_address(self.y);
                if carry {
                    self.cross_page()
                };

                let ignores = self.ignore_operand();
                if !ignores && !carry {
                    self.read_address(bus);
                }
                ignores
            }
            4 => {
                if self.page_crossed() {
                    self.read_address(bus);
                }
                !self.page_crossed()
            }
            5 => true,
            _ => unreachable!(),
        }
    }
    fn exec_relative(&mut self, bus: &mut impl CpuBus) -> bool {
        match self.state.address_mode_cycles {
            0 => {
                self.read_pc_byte(bus);
                false
            }
            1 => {
                let offset = bus.data().sign_cast() as i16;
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

    fn exec_adc(&mut self, bus: &mut impl CpuBus) {
        debug_assert_eq!(
            self.state.opcode_cycles, 0,
            "ADC should never be executed with cycle != 0"
        );

        let b = bus.data();
        let (a, carry) = self.a.carrying_add(b, self.flags.carry);

        let signed_a = self.a.sign_cast();
        let signed_b = b.sign_cast();
        let (_, overflow) = signed_a.carrying_add(signed_b, self.flags.carry);

        self.a = a;
        self.flags.carry = carry;
        self.flags.overflow = overflow;
        self.set_regular_flags(a);

        self.fetch(bus);
    }
    fn exec_and(&mut self, bus: &mut impl CpuBus) {
        debug_assert_eq!(
            self.state.opcode_cycles, 0,
            "AND should never be executed with cycle != 0"
        );

        self.a &= bus.data();
        self.set_regular_flags(self.a);

        self.fetch(bus);
    }
    fn exec_asl(&mut self, bus: &mut impl CpuBus) {
        let op = |x, flags: &mut Flags| {
            flags.carry = x & 128 != 0;
            x << 1
        };

        self.exec_rmw(bus, op);
    }
    fn exec_bit(&mut self, bus: &mut impl CpuBus) {
        debug_assert_eq!(
            self.state.opcode_cycles, 0,
            "BIT should never be executed with cycle != 0"
        );

        let b = bus.data();
        self.flags.negative = b & 128 != 0;
        self.flags.overflow = b & 64 != 0;
        self.flags.zero = self.a & b == 0;

        self.fetch(bus);
    }
    fn exec_generic_branch(&mut self, c: bool, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => {
                if c {
                    self.pc = self.address();
                } else {
                    self.fetch(bus);
                }
            }
            1 => {
                if !self.page_crossed() {
                    self.fetch(bus);
                }
            }
            2 => self.fetch(bus),
            _ => unreachable!(),
        }
    }
    fn exec_brk(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => {
                if self.state.break_mode.increment_pc {
                    self.pc += 1
                }
            }
            1 => {
                if self.state.break_mode.write {
                    self.push(self.pch(), bus);
                }
            }
            2 => {
                if self.state.break_mode.write {
                    self.push(self.pcl(), bus);
                }
            }
            3 => {
                let flags = self.flags.to_byte(self.state.break_mode.b_flag);
                if self.state.break_mode.write {
                    self.push(flags, bus);
                }
            }
            4 => {
                self.read(self.state.break_mode.vector, bus);
            }
            5 => {
                self.start_address_operand(bus.data());
                self.read(self.state.break_mode.vector + 1, bus);
            }
            6 => {
                if self.state.break_mode.set_irq_disable {
                    self.flags.irq_disable = true;
                }
                self.finish_address_operand(bus.data());
                self.pc = self.state.address;
                self.fetch(bus);
            }
            _ => unreachable!(),
        }
    }
    fn exec_clc(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => self.flags.carry = false,
            1 => self.fetch(bus),
            _ => unreachable!(),
        }
    }
    fn exec_cld(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => self.flags.decimal = false,
            1 => self.fetch(bus),
            _ => unreachable!(),
        }
    }
    fn exec_cli(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => self.flags.irq_disable = false,
            1 => self.fetch(bus),
            _ => unreachable!(),
        }
    }
    fn exec_clv(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => self.flags.overflow = false,
            1 => self.fetch(bus),
            _ => unreachable!(),
        }
    }
    fn exec_compare(&mut self, a: u8, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => {
                let b = bus.data();
                let (result, carry) = a.overflowing_sub(b);
                self.flags.carry = !carry;
                self.set_regular_flags(result);
                self.fetch(bus);
            }
            _ => unreachable!(),
        }
    }
    fn exec_dec(&mut self, bus: &mut impl CpuBus) {
        let op = |x: u8, _: &mut Flags| x.wrapping_sub(1);
        self.exec_rmw(bus, op);
    }
    fn exec_dex(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => {
                let value = self.x.wrapping_sub(1);
                self.set_regular_flags(value);
                self.x = value;
            }
            1 => self.fetch(bus),
            _ => unreachable!(),
        }
    }
    fn exec_dey(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => {
                let value = self.y.wrapping_sub(1);
                self.set_regular_flags(value);
                self.y = value;
            }
            1 => self.fetch(bus),
            _ => unreachable!(),
        }
    }
    fn exec_eor(&mut self, bus: &mut impl CpuBus) {
        debug_assert_eq!(
            self.state.opcode_cycles, 0,
            "EOR should never be executed with cycle != 0"
        );

        self.a ^= bus.data();
        self.set_regular_flags(self.a);

        self.fetch(bus);
    }
    fn exec_inc(&mut self, bus: &mut impl CpuBus) {
        let op = |x: u8, _: &mut Flags| x.wrapping_add(1);
        self.exec_rmw(bus, op);
    }
    fn exec_inx(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => {
                let value = self.x.wrapping_add(1);
                self.set_regular_flags(value);
                self.x = value;
            }
            1 => self.fetch(bus),
            _ => unreachable!(),
        }
    }
    fn exec_iny(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => {
                let value = self.y.wrapping_add(1);
                self.set_regular_flags(value);
                self.y = value;
            }
            1 => self.fetch(bus),
            _ => unreachable!(),
        }
    }
    fn exec_jmp(&mut self, bus: &mut impl CpuBus) {
        debug_assert_eq!(self.state.opcode_cycles, 0);
        self.pc = self.state.address;
        self.fetch(bus);
    }
    fn exec_jsr(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => {
                self.pc -= 1;
                self.push(self.pch(), bus);
            }
            1 => {
                self.push(self.pcl(), bus);
                self.pc = self.state.address;
            }
            2 => {
                self.fetch(bus);
            }
            _ => unreachable!(),
        }
    }
    fn exec_lda(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => {
                self.a = bus.data();
                self.set_regular_flags(self.a);
                self.fetch(bus);
            }
            _ => unreachable!(),
        }
    }
    fn exec_ldx(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => {
                self.x = bus.data();
                self.set_regular_flags(self.x);
                self.fetch(bus);
            }
            _ => unreachable!(),
        }
    }
    fn exec_ldy(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => {
                self.y = bus.data();
                self.set_regular_flags(self.y);
                self.fetch(bus);
            }
            _ => unreachable!(),
        }
    }
    fn exec_lsr(&mut self, bus: &mut impl CpuBus) {
        let op = |x, flags: &mut Flags| {
            flags.carry = x & 1 != 0;
            x >> 1
        };

        self.exec_rmw(bus, op);
    }
    fn exec_nop(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => (),
            1 => self.fetch(bus),
            _ => unreachable!(),
        }
    }
    fn exec_ora(&mut self, bus: &mut impl CpuBus) {
        debug_assert_eq!(
            self.state.opcode_cycles, 0,
            "EOR should never be executed with cycle != 0"
        );

        self.a |= bus.data();
        self.set_regular_flags(self.a);

        self.fetch(bus);
    }
    fn exec_pha(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => (),
            1 => {
                self.push(self.a, bus);
            }
            2 => self.fetch(bus),
            _ => unreachable!(),
        }
    }
    fn exec_php(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => (),
            1 => {
                self.push(self.flags.to_byte(true), bus);
            }
            2 => self.fetch(bus),
            _ => unreachable!(),
        }
    }
    fn exec_pla(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => (),
            1 => {
                self.pop(bus);
            }
            2 => {
                self.a = bus.data();
                self.set_regular_flags(self.a);
                self.fetch(bus);
            }
            _ => unreachable!(),
        }
    }
    fn exec_plp(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => (),
            1 => {
                self.pop(bus);
            }
            2 => {
                self.flags = Flags::from_byte(bus.data());
                self.fetch(bus);
            }
            _ => unreachable!(),
        }
    }
    fn exec_sbc(&mut self, bus: &mut impl CpuBus) {
        debug_assert_eq!(
            self.state.opcode_cycles, 0,
            "SBC should never be executed with cycle != 0"
        );

        let b = bus.data();
        let (a, carry) = self.a.borrowing_sub(b, !self.flags.carry);

        let signed_a = self.a.sign_cast();
        let signed_b = b.sign_cast();
        let (_, overflow) = signed_a.borrowing_sub(signed_b, self.flags.carry);

        self.a = a;
        self.flags.carry = !carry;
        self.flags.overflow = overflow;
        self.set_regular_flags(a);

        self.fetch(bus);
    }
    fn exec_sec(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => self.flags.carry = true,
            1 => self.fetch(bus),
            _ => unreachable!(),
        }
    }
    fn exec_sed(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => self.flags.decimal = true,
            1 => self.fetch(bus),
            _ => unreachable!(),
        }
    }
    fn exec_sei(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => self.flags.irq_disable = true,
            1 => self.fetch(bus),
            _ => unreachable!(),
        }
    }
    fn exec_sta(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => {
                self.write(self.state.address, self.a, bus);
            }
            1 => self.fetch(bus),
            _ => unreachable!(),
        }
    }
    fn exec_stx(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => {
                self.write(self.state.address, self.x, bus);
            }
            1 => self.fetch(bus),
            _ => unreachable!(),
        }
    }
    fn exec_sty(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => {
                self.write(self.state.address, self.y, bus);
            }
            1 => self.fetch(bus),
            _ => unreachable!(),
        }
    }
    fn exec_transfer(&mut self, value: u8, into: TransferTarget, bus: &mut impl CpuBus) {
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
            1 => self.fetch(bus),
            _ => unreachable!(),
        }
    }
    fn exec_rol(&mut self, bus: &mut impl CpuBus) {
        let op = |x, flags: &mut Flags| {
            let old_carry = flags.carry as u8;
            flags.carry = x & 128 != 0;
            (x << 1) | old_carry
        };

        self.exec_rmw(bus, op);
    }
    fn exec_ror(&mut self, bus: &mut impl CpuBus) {
        let op = |x, flags: &mut Flags| {
            let old_carry = (flags.carry as u8) << 7;
            flags.carry = x & 1 != 0;
            (x >> 1) | old_carry
        };

        self.exec_rmw(bus, op);
    }
    fn exec_rts(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => (),
            1 => (),
            2 => {
                self.pop(bus);
            }
            3 => {
                self.start_address_operand(bus.data());
                self.pop(bus);
            }
            4 => {
                self.finish_address_operand(bus.data());
            }
            5 => {
                self.pc = self.address() + 1;
                self.fetch(bus);
            }
            _ => unreachable!(),
        }
    }
    fn exec_rti(&mut self, bus: &mut impl CpuBus) {
        match self.state.opcode_cycles {
            0 => (),
            1 => (),
            2 => {
                self.pop(bus);
            }
            3 => {
                self.flags = Flags::from_byte(bus.data());
                self.pop(bus);
            }
            4 => {
                self.start_address_operand(bus.data());
                self.pop(bus);
            }
            5 => {
                self.finish_address_operand(bus.data());
                self.pc = self.address();
                self.fetch(bus);
            }
            _ => unreachable!(),
        }
    }

    fn exec_rmw(&mut self, bus: &mut impl CpuBus, op: fn(u8, &mut Flags) -> u8) {
        if self.state.address_mode == AddressMode::Accumulator {
            match self.state.opcode_cycles {
                0 => {
                    self.a = op(self.a, &mut self.flags);
                    self.set_regular_flags(self.a);
                }
                1 => self.fetch(bus),
                _ => unreachable!(),
            }
        } else {
            match self.state.opcode_cycles {
                0 => {
                    self.write(self.address(), bus.data(), bus);
                }
                1 => {
                    let old_value = bus.data();
                    let value = op(old_value, &mut self.flags);
                    self.set_regular_flags(value);
                    self.write(self.address(), value, bus);
                }
                2 => self.fetch(bus),
                _ => unreachable!(),
            }
        }
    }

    fn set_regular_flags(&mut self, value: u8) {
        self.flags.zero = value == 0;
        self.flags.negative = value > 127;
    }

    fn read(&mut self, address: u16, bus: &mut impl CpuBus) {
        bus.set_address(address);
    }
    fn write(&mut self, address: u16, value: u8, bus: &mut impl CpuBus) {
        bus.set_address(address);
        bus.set_data(value);
        bus.set_read(false);
    }
    fn push(&mut self, value: u8, bus: &mut impl CpuBus) {
        let address = 0x100 + self.sp as u16;
        self.sp = self.sp.wrapping_sub(1);
        self.write(address, value, bus);
    }
    fn pop(&mut self, bus: &mut impl CpuBus) {
        self.sp = self.sp.wrapping_add(1);
        let address = 0x100 + self.sp as u16;
        self.read(address, bus);
    }
    fn fetch(&mut self, bus: &mut impl CpuBus) {
        if self.nmi_pending {
            self.nmi_pending = false;
            self.sync_state(Opcode::BRK, AddressMode::Implied);
            self.state.break_mode = BreakMode::nmi();
        } else if self.irq_pending && !self.flags.irq_disable {
            self.sync_state(Opcode::BRK, AddressMode::Implied);
            self.state.break_mode = BreakMode::irq();
        } else {
            self.read_pc_byte(bus);
            bus.set_sync(true);
        }
    }
    fn read_pc_byte(&mut self, bus: &mut impl CpuBus) {
        self.read(self.pc, bus);
        self.pc += 1;
    }
    fn start_address_operand(&mut self, byte: u8) {
        self.state.address = byte as u16;
    }
    fn finish_address_operand(&mut self, byte: u8) {
        self.state.address |= (byte as u16) << 8;
    }
    fn add_address(&mut self, offset: u8) -> bool {
        let low = (self.address() & 0xFF) as u8;
        let high = (self.address() >> 8) as u8;
        let (new_low, carry) = low.overflowing_add(offset);
        let new_high = if !carry { high } else { high.wrapping_add(1) };
        let new_address = (new_low as u16) | (new_high as u16) << 8;
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
    fn read_address(&mut self, bus: &mut impl CpuBus) {
        self.read(self.address(), bus);
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

pub trait CpuBus {
    type Backup;

    fn address(&self) -> u16;
    fn set_address(&mut self, addr: u16);

    fn data(&self) -> u8;
    fn set_data(&mut self, data: u8);

    fn read(&self) -> bool;
    fn set_read(&mut self, read: bool);
    fn sync(&self) -> bool;
    fn set_sync(&mut self, sync: bool);
    fn halted(&self) -> bool;
    fn set_halted(&mut self, halted: bool);
    fn ready(&self) -> bool;
    fn irq(&self) -> bool;
    fn nmi(&self) -> bool;
    fn reset(&self) -> bool;

    fn backup(&self) -> Self::Backup;
    fn restore(&mut self, backup: Self::Backup);
}
