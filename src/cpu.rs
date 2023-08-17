use self::instruction::{decode, AddrMode, Op};
use std::u8;

pub mod instruction;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cpu6502 {
    pins: CpuPins,
    meta: Meta,

    a: u8,
    x: u8,
    y: u8,
    sp: u8,
    pc: u16,
    status: Status,

    interrupts: Interrupts,
    break_mode: BreakMode,

    op: Op,
    addr_mode: AddrMode,
}
impl Cpu6502 {
    pub fn init() -> Self {
        Self {
            pins: CpuPins::init(),
            meta: Meta::init(),

            a: 0,
            x: 0,
            y: 0,
            sp: 0,
            pc: 0,
            status: Status::init(),
            interrupts: Interrupts::init(),
            break_mode: BreakMode::Reset,
            op: Op::BRK,
            addr_mode: AddrMode::Implied,
        }
    }

    pub fn exec(&mut self, bus: &mut impl Bus6502) {
        if self.meta.jammed() {
            self.be_jammed(bus);
            return;
        }

        self.fetch(bus);
        let (addr, val) = self.eval_addr_mode(bus);
        self.execute_op(addr, val, bus);
    }
    fn be_jammed(&mut self, bus: &mut impl Bus6502) {
        if self.pins.rst() {
            self.interrupts = Interrupts::init();
            self.meta.set_jammed(false);
        }

        self.advance_cycle(false, bus);
    }
    fn fetch(&mut self, bus: &mut impl Bus6502) {
        if self.interrupts.reset() {
            self.break_mode = BreakMode::Reset;
            self.interrupts.clear();
        } else if self.interrupts.nmi() {
            self.break_mode = BreakMode::Nmi;
            self.interrupts.clear_nmi();
        } else if self.interrupts.irq() {
            self.break_mode = BreakMode::Irq;
        } else {
            self.break_mode = BreakMode::Break;
        }

        self.config_read(self.pc);
        self.pins.set_sync(true);
        self.cycle(bus);
        self.pins.set_sync(false);

        if self.break_mode != BreakMode::Break {
            self.op = Op::BRK;
            self.addr_mode = AddrMode::Implied;
        } else {
            self.pc += 1;
            (self.op, self.addr_mode) = decode(self.pins.data());
        }
    }

    fn eval_addr_mode(&mut self, bus: &mut impl Bus6502) -> (u16, u8) {
        match self.addr_mode {
            AddrMode::Implied => self.exec_implied_mode(bus),
            AddrMode::Accumulator => self.exec_accumulator_mode(bus),
            AddrMode::Immediate => self.exec_immediate_mode(bus),
            AddrMode::Relative => self.exec_relative_mode(bus),
            AddrMode::Zero => self.exec_zero_mode(bus),
            AddrMode::ZeroX => self.exec_zero_index_mode(self.x, bus),
            AddrMode::ZeroY => self.exec_zero_index_mode(self.y, bus),
            AddrMode::Absolute => self.exec_absolute_mode(bus),
            AddrMode::AbsoluteX => self.exec_absolute_index_mode(self.x, bus),
            AddrMode::AbsoluteY => self.exec_absolute_index_mode(self.y, bus),
            AddrMode::Indirect => self.exec_indirect_mode(bus),
            AddrMode::XIndirect => self.exec_xindirect_mode(bus),
            AddrMode::IndirectY => self.exec_indirect_y_mode(bus),
        }
    }
    fn exec_implied_mode(&mut self, bus: &mut impl Bus6502) -> (u16, u8) {
        self.read(self.pc, bus);
        (0, 0)
    }
    fn exec_accumulator_mode(&mut self, bus: &mut impl Bus6502) -> (u16, u8) {
        self.read(self.pc, bus);
        (0, self.a)
    }
    fn exec_immediate_mode(&mut self, bus: &mut impl Bus6502) -> (u16, u8) {
        let val = self.read_pc_byte(bus);
        (0, val)
    }
    fn exec_relative_mode(&mut self, bus: &mut impl Bus6502) -> (u16, u8) {
        let val = self.read_pc_byte(bus);
        (0, val)
    }
    fn exec_zero_mode(&mut self, bus: &mut impl Bus6502) -> (u16, u8) {
        let addr = self.read_pc_byte(bus) as u16;

        let val = if self.op.reads_operand() {
            self.read(addr, bus)
        } else {
            0
        };

        if self.op.is_rmw() {
            self.write(addr, val, bus);
        }

        (addr, val)
    }
    fn exec_zero_index_mode(&mut self, index: u8, bus: &mut impl Bus6502) -> (u16, u8) {
        let addr = self.read_pc_byte(bus);
        let _ = self.read(addr as u16, bus);
        let addr = addr.wrapping_add(index) as u16;

        let val = if self.op.reads_operand() {
            self.read(addr, bus)
        } else {
            0
        };
        if self.op.is_rmw() {
            self.write(addr, val, bus);
        }

        (addr, val)
    }
    fn exec_absolute_mode(&mut self, bus: &mut impl Bus6502) -> (u16, u8) {
        let low = self.read_pc_byte(bus) as u16;
        let high = self.read_pc_byte(bus) as u16;
        let addr = low | high << 8;
        let val = if self.op.reads_operand() {
            self.read(addr, bus)
        } else {
            0
        };
        if self.op.is_rmw() {
            self.write(addr, val, bus);
        }

        (addr, val)
    }
    fn exec_absolute_index_mode(&mut self, index: u8, bus: &mut impl Bus6502) -> (u16, u8) {
        let low = self.read_pc_byte(bus);
        let high = self.read_pc_byte(bus);
        let (low, carry) = low.overflowing_add(index);
        let wrong_address = (low as u16) | (high as u16) << 8;
        let high = if carry { high.wrapping_add(1) } else { high };
        let wrong_value = self.read(wrong_address, bus);

        let addr = (low as u16) | (high as u16) << 8;

        let read = self.op.reads_operand();
        let write = self.op.writes_operand();
        let rmw = self.op.is_rmw();

        if rmw {
            let val = self.read(addr, bus);
            self.write(addr, val, bus);
            (addr, val)
        } else if read {
            if !carry {
                return (addr, wrong_value);
            };
            let val = self.read(addr, bus);
            (addr, val)
        } else if write {
            (addr, wrong_value)
        } else {
            unreachable!()
        }
    }
    fn exec_indirect_mode(&mut self, bus: &mut impl Bus6502) -> (u16, u8) {
        let low = self.read_pc_byte(bus);
        let high = self.read_pc_byte(bus);
        let low_inc = low.wrapping_add(1);

        let addr_low = self.read((low as u16) | (high as u16) << 8, bus);
        let addr_high = self.read((low_inc as u16) | (high as u16) << 8, bus);

        let addr = (addr_low as u16) | (addr_high as u16) << 8;
        (addr, 0)
    }
    fn exec_xindirect_mode(&mut self, bus: &mut impl Bus6502) -> (u16, u8) {
        let offset = self.read_pc_byte(bus);
        self.read(offset as u16, bus);

        let ptr = offset.wrapping_add(self.x);
        let low = self.read(ptr as u16, bus);
        let high = self.read(ptr.wrapping_add(1) as u16, bus);
        let addr = (low as u16) | (high as u16) << 8;

        let val = if self.op.reads_operand() {
            self.read(addr, bus)
        } else {
            0
        };
        if self.op.is_rmw() {
            self.write(addr, val, bus);
        }

        (addr, val)
    }
    fn exec_indirect_y_mode(&mut self, bus: &mut impl Bus6502) -> (u16, u8) {
        let zero_ptr = self.read_pc_byte(bus);
        let low = self.read(zero_ptr as u16, bus);
        let high = self.read(zero_ptr.wrapping_add(1) as u16, bus);
        let (low, carry) = low.overflowing_add(self.y);
        let wrong_addr = (low as u16) | (high as u16) << 8;
        let high = if carry { high.wrapping_add(1) } else { high };
        let addr = (low as u16) | (high as u16) << 8;
        let wrong_val = self.read(wrong_addr, bus);

        let read = self.op.reads_operand();
        let write = self.op.writes_operand();
        let rmw = self.op.is_rmw();

        if rmw {
            let val = self.read(addr, bus);
            self.write(addr, val, bus);
            (addr, val)
        } else if read {
            if !carry {
                return (addr, wrong_val);
            };
            let val = self.read(addr, bus);
            (addr, val)
        } else if write {
            (addr, wrong_val)
        } else {
            unreachable!()
        }
    }

    fn execute_op(&mut self, addr: u16, val: u8, bus: &mut impl Bus6502) {
        use Op::*;
        match self.op {
            ADC => self.exec_adc(val),
            AND => self.exec_and(val),
            ASL => self.exec_asl(addr, val, bus),
            BCC => self.exec_branch(!self.status.carry(), val, bus),
            BCS => self.exec_branch(self.status.carry(), val, bus),
            BEQ => self.exec_branch(self.status.zero(), val, bus),
            BIT => self.exec_bit(val),
            BMI => self.exec_branch(self.status.negative(), val, bus),
            BNE => self.exec_branch(!self.status.zero(), val, bus),
            BPL => self.exec_branch(!self.status.negative(), val, bus),
            BRK => self.exec_brk(bus),
            BVC => self.exec_branch(!self.status.overflow(), val, bus),
            BVS => self.exec_branch(self.status.overflow(), val, bus),
            CLC => self.status.set_carry(false),
            CLD => self.status.set_decimal(false),
            CLV => self.status.set_overflow(false),
            CMP => self.exec_cmp(self.a, val),
            CPX => self.exec_cmp(self.x, val),
            CPY => self.exec_cmp(self.y, val),
            DEC => self.exec_dec(addr, val, bus),
            DEX => self.exec_dex(),
            DEY => self.exec_dey(),
            EOR => self.exec_eor(val),
            INC => self.exec_inc(addr, val, bus),
            INX => self.exec_inx(),
            INY => self.exec_iny(),
            JAM => self.exec_jam(),
            JMP => self.pc = addr,
            JSR => self.exec_jsr(addr, bus),
            LDA => self.exec_lda(val),
            LDX => self.exec_ldx(val),
            LDY => self.exec_ldy(val),
            LSR => self.exec_lsr(addr, val, bus),
            NOP => (),
            ORA => self.exec_ora(val),
            PHA => self.exec_pha(bus),
            PHP => self.exec_php(bus),
            PLA => self.exec_pla(bus),
            PLP => self.exec_plp(bus),
            ROL => self.exec_rol(addr, val, bus),
            ROR => self.exec_ror(addr, val, bus),
            RTI => self.exec_rti(bus),
            RTS => self.exec_rts(bus),
            SBC => self.exec_sbc(val),
            SEC => self.status.set_carry(true),
            SED => self.status.set_decimal(true),
            SEI => self.status.set_irq_disable(true),
            STA => self.exec_sta(addr, bus),
            STX => self.exec_stx(addr, bus),
            STY => self.exec_sty(addr, bus),
            TXS => self.sp = self.x,
            TAX => self.exec_tax(),
            TAY => self.exec_tay(),
            TSX => self.exec_tsx(),
            TXA => self.exec_txa(),
            TYA => self.exec_tya(),

            DCP => self.exec_dcp(addr, val, bus),
            ISC => self.exec_isc(addr, val, bus),
            LAX => self.exec_lax(val),
            SAX => self.exec_sax(addr, bus),
            RLA => self.exec_rla(addr, val, bus),
            RRA => self.exec_rra(addr, val, bus),
            SLO => self.exec_slo(addr, val, bus),
            SRE => self.exec_sre(addr, val, bus),
            op => todo!("Operation {op:?} is not implemented"),
        }
    }
    fn exec_adc(&mut self, val: u8) {
        self.do_adc(val);
    }
    fn exec_and(&mut self, val: u8) {
        self.a &= val;
        self.set_common_flags(self.a);
    }
    fn exec_asl(&mut self, addr: u16, val: u8, bus: &mut impl Bus6502) {
        self.status.set_carry(val & 128 != 0);
        let res = val << 1;
        self.set_common_flags(res);
        self.write_rmw_result(addr, res, bus);
    }
    fn exec_bit(&mut self, val: u8) {
        let negative = val & 128 != 0;
        let overflow = val & 64 != 0;
        self.status.set_negative(negative);
        self.status.set_overflow(overflow);
        self.status.set_zero(self.a & val == 0)
    }
    fn exec_branch(&mut self, c: bool, val: u8, bus: &mut impl Bus6502) {
        if !c {
            return;
        };

        self.read(self.pc, bus);
        let (low, carry) = self.pcl().overflowing_add_signed(val as i8);
        self.pc = u16::from_le_bytes([low, self.pch()]);
        if !carry {
            return;
        };

        self.read(self.pc, bus);
        let pch = self.pch().wrapping_add(1);
        self.pc = u16::from_le_bytes([low, pch]);
    }
    fn exec_brk(&mut self, bus: &mut impl Bus6502) {
        if self.break_mode.increment_pc() {
            self.pc = self.pc.wrapping_add(1);
        }

        let suppress_writes = self.break_mode.suppress_writes();
        self.push_in_brk(suppress_writes, self.pch(), bus);
        self.push_in_brk(suppress_writes, self.pcl(), bus);
        let b_flag = self.break_mode.set_brk_flag();
        self.push_in_brk(suppress_writes, self.status.to_pushable_bits(b_flag), bus);
        self.status.set_irq_disable(true);

        let vector = self.break_mode.vector();
        let low = self.read(vector, bus);
        let high = self.read(vector + 1, bus);
        self.pc = (low as u16) | (high as u16) << 8;
    }
    fn exec_cmp(&mut self, a: u8, b: u8) {
        let (res, carry) = a.overflowing_sub(b);
        self.status.set_carry(!carry);
        self.set_common_flags(res);
    }
    fn exec_dec(&mut self, addr: u16, val: u8, bus: &mut impl Bus6502) {
        let val = val.wrapping_sub(1);
        self.set_common_flags(val);
        self.write_rmw_result(addr, val, bus);
    }
    fn exec_dex(&mut self) {
        self.x = self.x.wrapping_sub(1);
        self.set_common_flags(self.x);
    }
    fn exec_dey(&mut self) {
        self.y = self.y.wrapping_sub(1);
        self.set_common_flags(self.y);
    }
    fn exec_eor(&mut self, val: u8) {
        self.a ^= val;
        self.set_common_flags(self.a);
    }
    fn exec_inc(&mut self, addr: u16, val: u8, bus: &mut impl Bus6502) {
        let val = val.wrapping_add(1);
        self.set_common_flags(val);
        self.write_rmw_result(addr, val, bus);
    }
    fn exec_inx(&mut self) {
        self.x = self.x.wrapping_add(1);
        self.set_common_flags(self.x)
    }
    fn exec_iny(&mut self) {
        self.y = self.y.wrapping_add(1);
        self.set_common_flags(self.y)
    }
    fn exec_jam(&mut self) {
        self.meta.set_jammed(true);
        self.config_read(self.pc);
    }
    fn exec_jsr(&mut self, addr: u16, bus: &mut impl Bus6502) {
        self.read(self.sp as u16 + 0x100, bus);
        let pc = self.pc.wrapping_sub(1);
        let pcl = pc as u8;
        let pch = (pc >> 8) as u8;
        self.push(pch, bus);
        self.push(pcl, bus);
        self.pc = addr;
    }
    fn exec_lda(&mut self, val: u8) {
        self.a = val;
        self.set_common_flags(val);
    }
    fn exec_ldx(&mut self, val: u8) {
        self.x = val;
        self.set_common_flags(val);
    }
    fn exec_ldy(&mut self, val: u8) {
        self.y = val;
        self.set_common_flags(val);
    }
    fn exec_lsr(&mut self, addr: u16, val: u8, bus: &mut impl Bus6502) {
        self.status.set_carry(val & 1 != 0);
        let res = val >> 1;
        self.set_common_flags(res);
        self.write_rmw_result(addr, res, bus);
    }
    fn exec_ora(&mut self, val: u8) {
        self.a |= val;
        self.set_common_flags(self.a);
    }
    fn exec_pha(&mut self, bus: &mut impl Bus6502) {
        self.push(self.a, bus);
    }
    fn exec_php(&mut self, bus: &mut impl Bus6502) {
        let bits = self.status.to_pushable_bits(true);
        self.push(bits, bus);
    }
    fn exec_pla(&mut self, bus: &mut impl Bus6502) {
        self.read(self.sp as u16 + 0x100, bus);
        self.sp = self.sp.wrapping_add(1);

        self.a = self.read(self.sp as u16 + 0x100, bus);
        self.set_common_flags(self.a);
    }
    fn exec_plp(&mut self, bus: &mut impl Bus6502) {
        self.read(self.sp as u16 + 0x100, bus);
        self.sp = self.sp.wrapping_add(1);

        let bits = self.read(self.sp as u16 + 0x100, bus);
        self.status = Status::from_pushable_bits(bits);
    }
    fn exec_rol(&mut self, addr: u16, val: u8, bus: &mut impl Bus6502) {
        let new_carry = val & 128 != 0;
        let old_carry = if self.status.carry() { 1 } else { 0 };
        let res = (val << 1) | old_carry;

        self.status.set_carry(new_carry);
        self.set_common_flags(res);
        self.write_rmw_result(addr, res, bus);
    }
    fn exec_ror(&mut self, addr: u16, val: u8, bus: &mut impl Bus6502) {
        let new_carry = val & 1 != 0;
        let old_carry = if self.status.carry() { 128 } else { 0 };
        let res = (val >> 1) | old_carry;

        self.status.set_carry(new_carry);
        self.set_common_flags(res);
        self.write_rmw_result(addr, res, bus);
    }
    fn exec_rti(&mut self, bus: &mut impl Bus6502) {
        self.read(self.sp as u16 + 0x100, bus);
        self.sp = self.sp.wrapping_add(1);

        self.status = Status::from_pushable_bits(self.read(self.sp as u16 + 0x100, bus));
        self.sp = self.sp.wrapping_add(1);
        let pcl = self.read(self.sp as u16 + 0x100, bus);
        self.sp = self.sp.wrapping_add(1);
        let pch = self.read(self.sp as u16 + 0x100, bus);

        self.pc = (pcl as u16) | (pch as u16) << 8;
    }
    fn exec_rts(&mut self, bus: &mut impl Bus6502) {
        self.read(self.sp as u16 + 0x100, bus);
        self.sp = self.sp.wrapping_add(1);

        let pcl = self.read(self.sp as u16 + 0x100, bus);
        self.sp = self.sp.wrapping_add(1);
        let pch = self.read(self.sp as u16 + 0x100, bus);

        self.pc = (pcl as u16) | (pch as u16) << 8;
        self.read(self.pc, bus);
        self.pc = self.pc.wrapping_add(1);
    }
    fn exec_sbc(&mut self, val: u8) {
        self.do_sbc(val)
    }
    fn exec_sta(&mut self, addr: u16, bus: &mut impl Bus6502) {
        self.write(addr, self.a, bus);
    }
    fn exec_stx(&mut self, addr: u16, bus: &mut impl Bus6502) {
        self.write(addr, self.x, bus);
    }
    fn exec_sty(&mut self, addr: u16, bus: &mut impl Bus6502) {
        self.write(addr, self.y, bus);
    }
    fn exec_tax(&mut self) {
        self.x = self.a;
        self.set_common_flags(self.x);
    }
    fn exec_tay(&mut self) {
        self.y = self.a;
        self.set_common_flags(self.y);
    }
    fn exec_tsx(&mut self) {
        self.x = self.sp;
        self.set_common_flags(self.x);
    }
    fn exec_txa(&mut self) {
        self.a = self.x;
        self.set_common_flags(self.a);
    }
    fn exec_tya(&mut self) {
        self.a = self.y;
        self.set_common_flags(self.a);
    }

    fn exec_dcp(&mut self, addr: u16, val: u8, bus: &mut impl Bus6502) {
        let r = val.wrapping_sub(1);

        let (cmp_r, carry) = self.a.overflowing_sub(r);
        self.status.set_carry(!carry);
        self.set_common_flags(cmp_r);
        self.write_rmw_result(addr, r, bus);
    }
    fn exec_isc(&mut self, addr: u16, val: u8, bus: &mut impl Bus6502) {
        let val = val.wrapping_add(1);
        self.do_sbc(val);
        self.write_rmw_result(addr, val, bus);
    }
    fn exec_lax(&mut self, val: u8) {
        self.a = val;
        self.x = val;
        self.set_common_flags(val);
    }
    fn exec_sax(&mut self, addr: u16, bus: &mut impl Bus6502) {
        let val = self.a & self.x;
        self.write(addr, val, bus);
    }
    fn exec_rla(&mut self, addr: u16, val: u8, bus: &mut impl Bus6502) {
        let val_low = if self.status.carry() { 1 } else { 0 };
        self.status.set_carry(val & 128 != 0);
        let val = (val << 1) | val_low;

        self.a &= val;
        self.set_common_flags(self.a);
        self.write_rmw_result(addr, val, bus);
    }
    fn exec_rra(&mut self, addr: u16, val: u8, bus: &mut impl Bus6502) {
        let val_low = if self.status.carry() { 128 } else { 0 };
        self.status.set_carry(val & 1 != 0);
        let val = (val >> 1) | val_low;

        self.do_adc(val);
        self.write_rmw_result(addr, val, bus);
    }
    fn exec_slo(&mut self, addr: u16, val: u8, bus: &mut impl Bus6502) {
        self.status.set_carry(val & 128 != 0);
        let val = val << 1;
        self.a |= val;
        self.set_common_flags(self.a);
        self.write_rmw_result(addr, val, bus);
    }
    fn exec_sre(&mut self, addr: u16, val: u8, bus: &mut impl Bus6502) {
        self.status.set_carry(val & 1 != 0);
        let val = val >> 1;

        self.a ^= val;
        self.set_common_flags(self.a);
        self.write_rmw_result(addr, val, bus);
    }

    fn do_adc(&mut self, val: u8) {
        let (res, carry) = self.a.carrying_add(val, self.status.carry());

        let (_, overflow) = (self.a as i8).overflowing_add(val as i8);

        self.a = res;
        self.set_common_flags(self.a);
        self.status.set_carry(carry);
        self.status.set_overflow(overflow);
    }
    fn do_sbc(&mut self, val: u8) {
        let (res, borrow) = self.a.borrowing_sub(val, !self.status.carry());

        let (_, overflow) = (self.a as i8).borrowing_sub(val as i8, !self.status.carry());

        self.status.set_overflow(overflow);
        self.status.set_carry(!borrow);
        self.set_common_flags(res);
        self.a = res;
    }

    fn write_rmw_result(&mut self, addr: u16, val: u8, bus: &mut impl Bus6502) {
        match self.addr_mode {
            AddrMode::Accumulator => self.a = val,
            _ => self.write(addr, val, bus),
        }
    }

    fn config_read_pc_byte(&mut self) {
        self.config_read(self.pc);
        self.pc += 1;
    }
    fn read_pc_byte(&mut self, bus: &mut impl Bus6502) -> u8 {
        self.config_read_pc_byte();
        self.cycle(bus);
        self.pins.data()
    }

    fn config_read(&mut self, address: u16) {
        self.pins.set_address(address);
        self.pins.set_read(true);
    }
    fn config_write(&mut self, address: u16, data: u8) {
        self.pins.set_address(address);
        self.pins.set_data(data);
        self.pins.set_read(false);
    }
    fn config_push(&mut self, suppress: bool, data: u8) {
        self.pins.set_address(self.sp as u16 + 0x100);
        self.pins.set_data(data);
        self.sp = self.sp.wrapping_sub(1);
        self.pins.set_read(suppress);
    }
    fn push_in_brk(&mut self, suppress: bool, data: u8, bus: &mut impl Bus6502) {
        self.config_push(suppress, data);
        self.cycle(bus);
    }
    fn push(&mut self, data: u8, bus: &mut impl Bus6502) {
        self.push_in_brk(false, data, bus);
    }

    fn read(&mut self, address: u16, bus: &mut impl Bus6502) -> u8 {
        self.config_read(address);
        self.cycle(bus);
        self.pins.data()
    }
    fn write(&mut self, addr: u16, val: u8, bus: &mut impl Bus6502) {
        self.config_write(addr, val);
        self.cycle(bus);
    }

    fn set_common_flags(&mut self, val: u8) {
        self.status.set_zero(val == 0);
        self.status.set_negative(val >= 128);
    }

    fn pch(&self) -> u8 {
        (self.pc >> 8) as u8
    }
    fn pcl(&self) -> u8 {
        self.pc as u8
    }

    fn cycle(&mut self, bus: &mut impl Bus6502) {
        self.advance_cycle(true, bus);
    }
    fn advance_cycle(&mut self, poll: bool, bus: &mut impl Bus6502) {
        loop {
            if poll {
                self.poll_interrupts();
            }
            self.update_meta_latches();
            bus.cycle(self);

            let write = !self.pins.read();
            let ready = !self.pins.not_ready();
            // The 6502 cannot halt on a write cycle
            if write || ready {
                break;
            }
            self.pins.set_halt(true);
        }

        self.pins.set_halt(false);
    }
    fn poll_interrupts(&mut self) {
        self.interrupts.set_irq(self.pins.irq());
        self.interrupts
            .or_nmi(self.pins.nmi() && !self.meta.last_nmi());
        self.interrupts.or_reset(self.pins.rst());
    }
    fn update_meta_latches(&mut self) {
        self.meta.set_last_nmi(self.pins.nmi());
        self.meta.set_last_rst(self.pins.rst());
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
    pub fn status(&self) -> Status {
        self.status
    }
    pub fn jammed(&self) -> bool {
        self.meta.jammed()
    }

    pub fn poke_pc(&mut self, pc: u16) {
        self.interrupts.clear();
        self.pc = pc;
    }
    pub fn pins_mut(&mut self) -> &mut CpuPins {
        &mut self.pins
    }
    pub fn pins(&self) -> CpuPins {
        self.pins
    }
    pub fn instruction(&self) -> (Op, AddrMode) {
        (self.op, self.addr_mode)
    }
    pub fn is_doing_interrupt(&self) -> bool {
        self.break_mode != BreakMode::Break
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum BreakMode {
    Break,
    Irq,
    Nmi,
    Reset,
}
impl BreakMode {
    fn increment_pc(self) -> bool {
        match self {
            Self::Break => true,
            _ => false,
        }
    }
    fn suppress_writes(self) -> bool {
        match self {
            Self::Break => false,
            Self::Irq => false,
            Self::Nmi => false,
            Self::Reset => true,
        }
    }
    fn set_brk_flag(self) -> bool {
        matches!(self, BreakMode::Break)
    }
    fn vector(self) -> u16 {
        match self {
            Self::Nmi => 0xFFFA,
            Self::Reset => 0xFFFC,
            Self::Break | Self::Irq => 0xFFFE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Meta(u8);
impl Meta {
    pub fn init() -> Self {
        Self(0)
    }

    pub fn jammed(self) -> bool {
        self.0 & (1 << Self::JAMMED) != 0
    }
    pub fn last_nmi(self) -> bool {
        self.0 & (1 << Self::LAST_NMI) != 0
    }
    pub fn last_rst(self) -> bool {
        self.0 & (1 << Self::LAST_RST) != 0
    }

    pub fn set_jammed(&mut self, jammed: bool) {
        let mask = 1 << Self::JAMMED;
        self.0 &= !mask;
        self.0 |= (jammed as u8) * mask
    }
    pub fn set_last_nmi(&mut self, last_nmi: bool) {
        let mask = 1 << Self::LAST_NMI;
        self.0 &= !mask;
        self.0 |= (last_nmi as u8) * mask
    }
    pub fn set_last_rst(&mut self, last_rst: bool) {
        let mask = 1 << Self::LAST_RST;
        self.0 &= !mask;
        self.0 |= (last_rst as u8) * mask
    }

    const JAMMED: u8 = 0;
    const LAST_NMI: u8 = 1;
    const LAST_RST: u8 = 2;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Interrupts(u8);
impl Interrupts {
    pub fn init() -> Self {
        let bits = 1 << Self::RESET;
        Self(bits)
    }

    pub fn irq(self) -> bool {
        let bit = 1 << Self::IRQ;
        (self.0 & bit) != 0
    }
    pub fn nmi(self) -> bool {
        let bit = 1 << Self::NMI;
        (self.0 & bit) != 0
    }
    pub fn reset(self) -> bool {
        let bit = 1 << Self::RESET;
        (self.0 & bit) != 0
    }

    pub fn set_irq(&mut self, irq: bool) {
        let mask = 1 << Self::IRQ;
        self.0 &= !mask;
        self.0 |= mask * irq as u8;
    }
    pub fn or_nmi(&mut self, nmi: bool) {
        let mask = 1 << Self::NMI;
        self.0 |= if nmi { mask } else { 0 };
    }
    pub fn or_reset(&mut self, reset: bool) {
        let mask = 1 << Self::RESET;
        self.0 |= if reset { mask } else { 0 };
    }

    pub fn clear_nmi(&mut self) {
        self.0 &= !(1 << Self::NMI);
    }

    pub fn clear(&mut self) {
        self.0 = 0;
    }

    const IRQ: u8 = 0;
    const NMI: u8 = 1;
    const RESET: u8 = 2;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Status(u8);
impl Status {
    pub fn init() -> Self {
        let mut ret = Self(0);
        ret.set_irq_disable(true);
        ret
    }

    pub fn carry(self) -> bool {
        (self.0 >> Self::CARRY) & 1 != 0
    }
    pub fn zero(self) -> bool {
        (self.0 >> Self::ZERO) & 1 != 0
    }
    pub fn irq_disable(self) -> bool {
        (self.0 >> Self::IRQ_DISABLE) & 1 != 0
    }
    pub fn decimal(self) -> bool {
        (self.0 >> Self::DECIMAL) & 1 != 0
    }
    pub fn overflow(self) -> bool {
        (self.0 >> Self::OVERFLOW) & 1 != 0
    }
    pub fn negative(self) -> bool {
        (self.0 >> Self::NEGATIVE) & 1 != 0
    }

    pub fn set_carry(&mut self, carry: bool) {
        let bit = 1 << Self::CARRY;
        self.0 &= !bit;
        self.0 |= bit * carry as u8;
    }
    pub fn set_zero(&mut self, zero: bool) {
        let bit = 1 << Self::ZERO;
        self.0 &= !bit;
        self.0 |= bit * zero as u8;
    }
    pub fn set_irq_disable(&mut self, irq_disable: bool) {
        let bit = 1 << Self::IRQ_DISABLE;
        self.0 &= !bit;
        self.0 |= bit * irq_disable as u8;
    }
    pub fn set_decimal(&mut self, decimal: bool) {
        let bit = 1 << Self::DECIMAL;
        self.0 &= !bit;
        self.0 |= bit * decimal as u8;
    }
    pub fn set_overflow(&mut self, overflow: bool) {
        let bit = 1 << Self::OVERFLOW;
        self.0 &= !bit;
        self.0 |= bit * overflow as u8;
    }
    pub fn set_negative(&mut self, negative: bool) {
        let bit = 1 << Self::NEGATIVE;
        self.0 &= !bit;
        self.0 |= bit * negative as u8;
    }

    pub fn bits(&self) -> u8 {
        self.0
    }

    pub fn to_pushable_bits(self, brk: bool) -> u8 {
        let brk = if brk { 1 << Self::BREAK } else { 0 };
        let set = 1 << Self::SET;
        self.0 | brk | set
    }
    pub fn from_pushable_bits(bits: u8) -> Self {
        let mask = 0b11001111;
        Self(bits & mask)
    }

    const CARRY: u8 = 0;
    const ZERO: u8 = 1;
    const IRQ_DISABLE: u8 = 2;
    const DECIMAL: u8 = 3;
    const BREAK: u8 = 4;
    const SET: u8 = 5;
    const OVERFLOW: u8 = 6;
    const NEGATIVE: u8 = 7;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CpuPins(u32);
impl CpuPins {
    pub fn init() -> Self {
        Self(0)
    }

    pub fn address(self) -> u16 {
        (self.0 >> Self::ADDRESS) as u16
    }
    pub fn data(self) -> u8 {
        (self.0 >> Self::DATA) as u8
    }
    pub fn read(self) -> bool {
        self.0 & (1 << Self::READ) != 0
    }
    pub fn not_ready(self) -> bool {
        self.0 & (1 << Self::NOT_READY) != 0
    }
    pub fn halt(self) -> bool {
        self.0 & (1 << Self::NOT_READY) != 0
    }
    pub fn irq(self) -> bool {
        self.0 & (1 << Self::IRQ) != 0
    }
    pub fn nmi(self) -> bool {
        self.0 & (1 << Self::NMI) != 0
    }
    pub fn rst(self) -> bool {
        self.0 & (1 << Self::RST) != 0
    }
    pub fn sync(self) -> bool {
        self.0 & (1 << Self::SYNC) != 0
    }

    pub fn set_address(&mut self, address: u16) {
        let not_mask = 0xFFFF << Self::ADDRESS;
        let mask = !not_mask;
        let unmasked = self.0 & mask;
        let address = (address as u32) << Self::ADDRESS;
        self.0 = unmasked | address;
    }
    pub fn set_data(&mut self, data: u8) {
        let not_mask = 0xFF << Self::DATA;
        let mask = !not_mask;
        let unmasked = self.0 & mask;
        let address = (data as u32) << Self::DATA;
        self.0 = unmasked | address;
    }
    pub fn set_read(&mut self, read: bool) {
        let bit = 1 << Self::READ;
        self.0 &= !bit;
        self.0 |= bit * read as u32;
    }
    pub fn set_not_ready(&mut self, not_ready: bool) {
        let bit = 1 << Self::NOT_READY;
        self.0 &= !bit;
        self.0 |= bit * not_ready as u32;
    }
    pub fn set_halt(&mut self, halt: bool) {
        let bit = 1 << Self::HALT;
        self.0 &= !bit;
        self.0 |= bit * halt as u32;
    }
    pub fn set_irq(&mut self, irq: bool) {
        let bit = 1 << Self::IRQ;
        self.0 &= !bit;
        self.0 |= bit * irq as u32;
    }
    pub fn set_nmi(&mut self, nmi: bool) {
        let bit = 1 << Self::NMI;
        self.0 &= !bit;
        self.0 |= bit * nmi as u32;
    }
    pub fn set_rst(&mut self, rst: bool) {
        let bit = 1 << Self::RST;
        self.0 &= !bit;
        self.0 |= bit * rst as u32;
    }
    pub fn set_sync(&mut self, sync: bool) {
        let bit = 1 << Self::SYNC;
        self.0 &= !bit;
        self.0 |= bit * sync as u32;
    }

    const ADDRESS: u32 = 0;
    const DATA: u32 = 16;
    const READ: u32 = 24;
    const NOT_READY: u32 = 25;
    const HALT: u32 = 26;
    const IRQ: u32 = 27;
    const NMI: u32 = 28;
    const RST: u32 = 29;
    const SYNC: u32 = 30;
}

pub trait Bus6502 {
    /// Called by the CPU whenever it completes a cycle so that external devices can update themselves.
    fn cycle(&mut self, cpu: &mut Cpu6502);
}
