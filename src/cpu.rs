mod instr;

use crate::bus::Bus;
use bitbash::bitfield;
use instr::AddressingMode;

pub struct Cpu<'a> {
    pub register_a: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub program_counter: u16,
    pub stack_pointer: u8,
    pub status: StatusReg,
    pub bus: Bus<'a>,
    pub mnemonic: String,
    pub cycles: u8,
    nmi_seen: bool,
    quit_on_brk: bool,
}

bitfield! {
    #[derive(Copy, Clone)]
    pub struct StatusReg(pub u8);
    pub new();

    field carry:       bool = [0];
    field zero:        bool = [1];
    field irq_disable: bool = [2];
    field decimal:     bool = [3];
    field break_cmd:   bool = [4];
    field unused:      bool = [5];
    field overflow:    bool = [6];
    field negative:    bool = [7];
}

const SIGN_MASK: u8 = 0x1 << 7;
const RESET_ADDR: u16 = 0xFFFC;
const STACK_PAGE: u16 = 0x0100;

impl<'a> Cpu<'a> {
    pub fn new(bus: Bus<'a>) -> Self {
        Self {
            register_a: 0,
            register_x: 0,
            register_y: 0,
            program_counter: 0,
            stack_pointer: 0,
            status: StatusReg(0).with_unused(true).with_irq_disable(true),
            bus,
            mnemonic: "".to_owned(),
            cycles: 0,
            nmi_seen: false,
            quit_on_brk: false,
        }
    }

    fn update_zero_neg(&mut self, val: u8) {
        self.status.set_zero(val == 0);
        self.status.set_negative(val >= 128);
    }

    /// Pushes a 8-bit value onto the stack, decrementing stack pointer
    fn push_stack(&mut self, data: u8) {
        self.bus.write(STACK_PAGE | self.stack_pointer as u16, data);
        self.stack_pointer = self.stack_pointer.wrapping_sub(1);
    }

    /// Pushes a 16-bit value onto the stack, decrementing stack pointer
    fn push_stack_u16(&mut self, data: u16) {
        self.push_stack((data >> 8) as u8);
        self.push_stack((data & 0x00FF) as u8);
    }

    /// Pulls a 8-bit value from the stack, incrementing stack pointer
    fn pull_stack(&mut self) -> u8 {
        self.stack_pointer = self.stack_pointer.wrapping_add(1);
        self.bus.read(STACK_PAGE | self.stack_pointer as u16)
    }

    /// Pulls a 16-bit value from the stack, incrementing stack pointer
    fn pull_stack_u16(&mut self) -> u16 {
        let lsb = self.pull_stack() as u16;
        let msb = self.pull_stack() as u16;
        (msb << 8) | lsb
    }

    // Used for testing
    pub fn _setup(&mut self, prog: Vec<u8>) {
        for (idx, item) in prog.iter().enumerate() {
            self.bus.write(0x600 + idx as u16, *item);
        }
        self.program_counter = 0x600;
        self.quit_on_brk = true;
    }

    fn get_operand_addr(&mut self, mode: &AddressingMode) -> u16 {
        match mode {
            // Data is the parameter
            AddressingMode::Immediate => self.program_counter,

            // Data is in page zero i.e. 0x0000 - 0x00FF, at the index indicated by parameter
            AddressingMode::ZeroPage => self.bus.read(self.program_counter) as u16,

            // Data is in page zero, at the index indicated by parameter + X
            AddressingMode::ZeroPageX => {
                let addr = self.bus.read(self.program_counter);
                addr.wrapping_add(self.register_x) as u16
            }

            // Data is in page zero, at the index indicated by parameter + Y
            AddressingMode::ZeroPageY => {
                let addr = self.bus.read(self.program_counter);
                addr.wrapping_add(self.register_y) as u16
            }

            // Data is in the address indicated by 2-byte parameter
            AddressingMode::Absolute => self.bus.read_u16(self.program_counter),

            // Data is in the address indicated by 2-byte parameter incremented by X
            AddressingMode::AbsoluteX => {
                let addr = self.bus.read_u16(self.program_counter);
                let msb = addr & 0xFF00;
                let addr = addr.wrapping_add(self.register_x as u16);
                if msb != addr & 0xFF00 {
                    self.bus.tick(1);
                }
                addr
            }

            // Data is in the address indicated by 2-byte parameter incremented by X
            // No extra tick from page miss
            AddressingMode::AbsoluteXNoPlus => {
                let addr = self.bus.read_u16(self.program_counter);
                addr.wrapping_add(self.register_x as u16)
            }

            // Data is in the address indicated by 2-byte parameter incremented by Y
            AddressingMode::AbsoluteY => {
                let addr = self.bus.read_u16(self.program_counter);
                let msb = addr & 0xFF00;
                let addr = addr.wrapping_add(self.register_y as u16);
                if msb != addr & 0xFF00 {
                    self.bus.tick(1);
                }
                addr
            }

            // Data is in the address indicated by 2-byte parameter incremented by Y
            // No extra tick from page miss
            AddressingMode::AbsoluteYNoPlus => {
                let addr = self.bus.read_u16(self.program_counter);
                addr.wrapping_add(self.register_y as u16)
            }

            // Data is in address indicated by pointer indicated by (parameter + X)
            AddressingMode::IndirectX => {
                let param = self.bus.read(self.program_counter);
                let lsb = self.bus.read(param.wrapping_add(self.register_x) as u16) as u16;
                let msb = self
                    .bus
                    .read(param.wrapping_add(self.register_x).wrapping_add(1) as u16)
                    as u16;
                msb << 8 | lsb
            }

            // Data is in address indicated by (pointer indicated by parameter) + Y
            AddressingMode::IndirectY => {
                let param = self.bus.read(self.program_counter);
                let lsb = self.bus.read(param as u16) as u16;
                let msb = self.bus.read(param.wrapping_add(1) as u16) as u16;
                let addr = (msb << 8 | lsb).wrapping_add(self.register_y as u16);

                if addr >> 8 != msb {
                    self.bus.tick(1);
                }
                addr
            }

            // Data is in address indicated by (pointer indicated by parameter) + Y
            // No extra tick from page miss
            AddressingMode::IndirectYNoPlus => {
                let param = self.bus.read(self.program_counter);
                let lsb = self.bus.read(param as u16) as u16;
                let msb = self.bus.read(param.wrapping_add(1) as u16) as u16;
                (msb << 8 | lsb).wrapping_add(self.register_y as u16)
            }

            AddressingMode::None => panic!("mode {:?} is not supported", mode),
        }
    }

    // Used for testing
    fn _run(&mut self) {
        self.run_with_callback(|_| {});
    }

    fn nmi(&mut self) {
        // println!("In NMI");
        self.push_stack_u16(self.program_counter);
        self.push_stack(self.status.0);
        self.status.set_irq_disable(true);

        self.bus.tick(7);
        let target = self.bus.read_u16(0xFFFA);
        self.program_counter = target;
    }

    fn irq(&mut self) {
        // println!("In IRQ");
        self.push_stack_u16(self.program_counter);
        self.push_stack(self.status.0);
        self.status.set_irq_disable(true);

        self.bus.tick(7);
        let target = self.bus.read_u16(0xFFFE);
        self.program_counter = target;
    }

    pub fn reset(&mut self) {
        self.register_a = 0;
        self.register_x = 0;
        self.register_y = 0;
        self.stack_pointer = 0xfd;
        self.status = StatusReg(0).with_irq_disable(true).with_unused(true);

        self.program_counter = self.bus.read_u16(RESET_ADDR);
    }

    pub fn run_with_callback<F>(&mut self, mut callback: F)
    where
        F: FnMut(&mut Cpu),
    {
        let mut instructions = instr::INSTRUCTIONS.clone();
        instructions.sort_unstable_by_key(|k| k.opcode);

        loop {
            let op = self.bus.read(self.program_counter);

            let instruction = instructions[op as usize];

            self.mnemonic = instruction.mnemonic.to_owned();
            self.cycles = instruction.duration;

            callback(self);

            self.program_counter += 1;

            match instruction.mnemonic {
                "ADC" => self.adc(&instruction.addressing_mode, false),
                "ANC" => self.anc(&instruction.addressing_mode),
                "AND" => self.and(&instruction.addressing_mode),
                "ASL" => self.asl(&instruction.addressing_mode),
                "BCC" => self.bcc(),
                "BCS" => self.bcs(),
                "BEQ" => self.beq(),
                "BIT" => self.bit(&instruction.addressing_mode),
                "BMI" => self.bmi(),
                "BNE" => self.bne(),
                "BPL" => self.bpl(),
                "BRK" => {
                    if self.quit_on_brk {
                        return;
                    } else {
                        self.brk()
                    }
                }
                "BVC" => self.bvc(),
                "BVS" => self.bvs(),
                "CLC" => self.status.set_carry(false),
                "CLD" => self.status.set_decimal(false),
                "CLI" => self.status.set_irq_disable(false),
                "CLV" => self.status.set_overflow(false),
                "CMP" => self.compare(self.register_a, &instruction.addressing_mode),
                "CPX" => self.compare(self.register_x, &instruction.addressing_mode),
                "CPY" => self.compare(self.register_y, &instruction.addressing_mode),
                "DEC" => self.dec(&instruction.addressing_mode),
                "DEX" => self.dex(),
                "DEY" => self.dey(),
                "EOR" => self.eor(&instruction.addressing_mode),
                "HLT" => return,
                "INC" => self.inc(&instruction.addressing_mode),
                "INX" => self.inx(),
                "INY" => self.iny(),
                "JMP" => self.jmp(&instruction.addressing_mode),
                "JSR" => self.jsr(),
                "LDA" => self.lda(&instruction.addressing_mode),
                "LDX" => self.ldx(&instruction.addressing_mode),
                "LDY" => self.ldy(&instruction.addressing_mode),
                "LSR" => self.lsr(&instruction.addressing_mode),
                "NOP" => (),
                "ORA" => self.ora(&instruction.addressing_mode),
                "PHA" => self.push_stack(self.register_a),
                "PHP" => self.push_stack(self.status.with_break_cmd(true).0),
                "PLA" => {
                    self.register_a = self.pull_stack();
                    self.update_zero_neg(self.register_a)
                }
                "PLP" => self.status.0 = self.pull_stack() & 0xEF | 0x20,
                "ROL" => self.rol(&instruction.addressing_mode),
                "ROR" => self.ror(&instruction.addressing_mode),
                "RTI" => self.rti(),
                "RTS" => self.rts(),
                "SBC" => self.adc(&instruction.addressing_mode, true),
                "SEC" => self.status.set_carry(true),
                "SED" => self.status.set_decimal(true),
                "SEI" => self.status.set_irq_disable(true),
                "STA" => {
                    let addr = self.get_operand_addr(&instruction.addressing_mode);
                    self.bus.write(addr, self.register_a)
                }
                "STX" => {
                    let addr = self.get_operand_addr(&instruction.addressing_mode);
                    self.bus.write(addr, self.register_x)
                }
                "STY" => {
                    let addr = self.get_operand_addr(&instruction.addressing_mode);
                    self.bus.write(addr, self.register_y)
                }
                "TAX" => self.tax(),
                "TAY" => self.tay(),
                "TSX" => self.tsx(),
                "TXA" => self.txa(),
                "TXS" => self.txs(),
                "TYA" => self.tya(),

                // Unofficial opcodes
                "LAX" => self.lax(&instruction.addressing_mode),
                "SAX" => self.sax(&instruction.addressing_mode),
                "DCP" => self.dcp(&instruction.addressing_mode),
                "ISB" => self.isb(&instruction.addressing_mode),
                "SLO" => self.slo(&instruction.addressing_mode),
                "RLA" => self.rla(&instruction.addressing_mode),
                "SRE" => self.sre(&instruction.addressing_mode),
                "RRA" => self.rra(&instruction.addressing_mode),

                // Should never happen
                _ => panic!("Uncrecognized mnemonic {}", instruction.mnemonic),
            }

            self.bus.tick(instruction.duration);

            // Don't increment program counter for some instructions
            match instruction.mnemonic {
                "JMP" | "JSR" => (),
                _ => self.program_counter += (instruction.bytes - 1) as u16,
            }

            if self.bus.nmi_active() && !self.nmi_seen {
                self.nmi_seen = true;
                self.nmi();
            } else {
                self.nmi_seen = self.bus.nmi_active();
            }

            if !self.status.irq_disable() && self.bus.irq_active() {
                self.irq();
            }
        }
    }
}

// Individual instruction behaviour is implemented here
impl<'a> Cpu<'a> {
    fn adc(&mut self, mode: &AddressingMode, sbc: bool) {
        let addr = self.get_operand_addr(mode);
        let operand = if sbc {
            !self.bus.read(addr)
        } else {
            self.bus.read(addr)
        };

        let carry = if self.status.carry() { 1 } else { 0 };

        let orig_a = self.register_a;
        self.register_a = orig_a.wrapping_add(operand).wrapping_add(carry);

        // Overflow if both inputs are different sign than result
        self.status.set_overflow(
            (orig_a ^ self.register_a) & (operand ^ self.register_a) & SIGN_MASK != 0,
        );

        // Carry if new value is smaller, or value from operand was 0xFF and carry was set
        self.status
            .set_carry(self.register_a < orig_a || self.register_a == orig_a && carry > 0);

        self.update_zero_neg(self.register_a);
    }

    fn anc(&mut self, mode: &AddressingMode) {
        self.and(mode);
        self.status.set_carry(self.status.negative());
    }

    fn and(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_addr(mode);
        self.register_a &= self.bus.read(addr);
        self.update_zero_neg(self.register_a);
    }

    fn asl(&mut self, mode: &AddressingMode) {
        // NoneAddressing works directly on accumulator
        // MSB shifts to carry bit
        match mode {
            &AddressingMode::None => {
                self.status.set_carry(self.register_a & SIGN_MASK != 0);
                self.register_a <<= 1;
                self.update_zero_neg(self.register_a);
            }
            _ => {
                let addr = self.get_operand_addr(mode);
                let mut operand = self.bus.read(addr);
                self.status.set_carry(operand & SIGN_MASK != 0);
                operand <<= 1;
                self.bus.write(addr, operand);
                self.update_zero_neg(operand);
            }
        }
    }

    fn branch_relative(&mut self) {
        let offset = ((self.bus.read(self.program_counter) as i8) as i16) as u16;
        let old_pc = self.program_counter;
        self.program_counter = self.program_counter.wrapping_add(offset);
        if old_pc & 0xFF00 != self.program_counter & 0xFF00 {
            self.bus.tick(1);
        }
        self.bus.tick(1);
    }

    fn bcc(&mut self) {
        if !self.status.carry() {
            self.branch_relative();
        }
    }

    fn bcs(&mut self) {
        if self.status.carry() {
            self.branch_relative();
        }
    }

    fn beq(&mut self) {
        if self.status.zero() {
            self.branch_relative();
        }
    }

    fn bmi(&mut self) {
        if self.status.negative() {
            self.branch_relative();
        }
    }

    fn bne(&mut self) {
        if !self.status.zero() {
            self.branch_relative();
        }
    }

    fn bpl(&mut self) {
        if !self.status.negative() {
            self.branch_relative();
        }
    }

    fn bvc(&mut self) {
        if !self.status.overflow() {
            self.branch_relative();
        }
    }

    fn bvs(&mut self) {
        if self.status.overflow() {
            self.branch_relative();
        }
    }

    fn bit(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_addr(mode);
        let operand = self.bus.read(addr);
        self.status.set_zero(self.register_a & operand == 0);
        self.status.set_overflow(operand & 0x1 << 6 != 0); // store bit 6
        self.status.set_negative(operand & 0x1 << 7 != 0); // and bit 7
    }

    fn brk(&mut self) {
        self.push_stack_u16(self.program_counter.wrapping_add(1));
        self.push_stack(self.status.0 | 0x10);
        let target = self.bus.read_u16(0xFFFE);
        self.program_counter = target;
    }

    fn compare(&mut self, source: u8, mode: &AddressingMode) {
        let addr = self.get_operand_addr(mode);
        let operand = self.bus.read(addr);
        self.status.set_carry(source >= operand);
        self.status.set_zero(source == operand);
        self.status
            .set_negative(source.wrapping_sub(operand) & SIGN_MASK != 0);
    }

    fn dec(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_addr(mode);
        let new_val = self.bus.read(addr).wrapping_sub(1);
        self.bus.write(addr, new_val);
        self.update_zero_neg(new_val);
    }

    fn dex(&mut self) {
        self.register_x = self.register_x.wrapping_sub(1);
        self.update_zero_neg(self.register_x);
    }

    fn dey(&mut self) {
        self.register_y = self.register_y.wrapping_sub(1);
        self.update_zero_neg(self.register_y);
    }

    fn eor(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_addr(mode);
        self.register_a ^= self.bus.read(addr);
        self.update_zero_neg(self.register_a);
    }

    fn inc(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_addr(mode);
        let old_val = self.bus.read(addr);
        let new_val = old_val.wrapping_add(1);
        self.bus.write(addr, new_val);
        self.update_zero_neg(new_val);
    }

    fn inx(&mut self) {
        self.register_x = self.register_x.wrapping_add(1);
        self.update_zero_neg(self.register_x);
    }

    fn iny(&mut self) {
        self.register_y = self.register_y.wrapping_add(1);
        self.update_zero_neg(self.register_y);
    }

    fn jmp(&mut self, mode: &AddressingMode) {
        self.program_counter = match mode {
            AddressingMode::Absolute => self.bus.read_u16(self.program_counter),
            AddressingMode::None => {
                // 6502 reads MSB of indirect operand from the wrong address.
                // If operand is 0x30ff, address is read from 0x30ff and 0x3000
                // instead of 0x30ff and 0x3100
                let operand_addr = self.bus.read_u16(self.program_counter);
                let correct_operand = self.bus.read_u16(operand_addr);

                if operand_addr & 0x00FF == 0x00FF {
                    let wrong_msb = (self.bus.read(operand_addr & 0xFF00) as u16) << 8;
                    wrong_msb | correct_operand & 0x00FF
                } else {
                    correct_operand
                }
            }
            _ => panic!("Unsupported addressing mode for JMP!"),
        };
    }

    fn jsr(&mut self) {
        self.push_stack_u16(self.program_counter + 1);
        self.program_counter = self.bus.read_u16(self.program_counter);
    }

    fn lda(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_addr(mode);
        self.register_a = self.bus.read(addr);
        self.update_zero_neg(self.register_a);
    }

    fn ldx(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_addr(mode);
        self.register_x = self.bus.read(addr);
        self.update_zero_neg(self.register_x);
    }

    fn ldy(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_addr(mode);
        self.register_y = self.bus.read(addr);
        self.update_zero_neg(self.register_y);
    }

    fn lsr(&mut self, mode: &AddressingMode) {
        // NoneAddressing works directly on accumulator
        // LSB shifts to carry bit
        match mode {
            &AddressingMode::None => {
                self.status.set_carry(self.register_a & 0x1 != 0);
                self.register_a >>= 1;
                self.update_zero_neg(self.register_a);
            }
            _ => {
                let addr = self.get_operand_addr(mode);
                let mut operand = self.bus.read(addr);
                self.status.set_carry(operand & 0x1 != 0);
                operand >>= 1;
                self.bus.write(addr, operand);
                self.update_zero_neg(operand);
            }
        }
    }

    fn ora(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_addr(mode);
        self.register_a |= self.bus.read(addr);
        self.update_zero_neg(self.register_a);
    }

    fn rol(&mut self, mode: &AddressingMode) {
        // NoneAddressing works directly on accumulator
        // Carry bit shifts to LSB, MSB shifts to carry bit
        match mode {
            &AddressingMode::None => {
                let carry_in = if self.status.carry() { 0x01 } else { 0x00 };
                self.status.set_carry(self.register_a & SIGN_MASK != 0);
                self.register_a <<= 1;
                self.register_a |= carry_in;
                self.update_zero_neg(self.register_a);
            }
            _ => {
                let addr = self.get_operand_addr(mode);
                let mut operand = self.bus.read(addr);
                let carry_in = if self.status.carry() { 0x01 } else { 0x00 };
                self.status.set_carry(operand & SIGN_MASK != 0);
                operand <<= 1;
                operand |= carry_in;
                self.bus.write(addr, operand);
                self.update_zero_neg(operand);
            }
        }
    }

    fn ror(&mut self, mode: &AddressingMode) {
        // NoneAddressing works directly on accumulator
        // Carry bit shifts to MSB, LSB shifts to carry bit
        match mode {
            &AddressingMode::None => {
                let carry_in = if self.status.carry() { 0x80 } else { 0x00 };
                self.status.set_carry(self.register_a & 0x01 != 0);
                self.register_a >>= 1;
                self.register_a |= carry_in;
                self.update_zero_neg(self.register_a);
            }
            _ => {
                let addr = self.get_operand_addr(mode);
                let mut operand = self.bus.read(addr);
                let carry_in = if self.status.carry() { 0x80 } else { 0x00 };
                self.status.set_carry(operand & 0x01 != 0);
                operand >>= 1;
                operand |= carry_in;
                self.bus.write(addr, operand);
                self.update_zero_neg(operand);
            }
        }
    }

    fn rti(&mut self) {
        self.status.0 = self.pull_stack() & 0xEF | 0x20;
        self.program_counter = self.pull_stack_u16();
        // println!("Returning from exception");
    }

    fn rts(&mut self) {
        self.program_counter = self.pull_stack_u16().wrapping_add(1);
    }

    fn tax(&mut self) {
        self.register_x = self.register_a;
        self.update_zero_neg(self.register_x);
    }

    fn tay(&mut self) {
        self.register_y = self.register_a;
        self.update_zero_neg(self.register_y);
    }

    fn tsx(&mut self) {
        self.register_x = self.stack_pointer;
        self.update_zero_neg(self.register_x);
    }

    fn txa(&mut self) {
        self.register_a = self.register_x;
        self.update_zero_neg(self.register_a);
    }

    fn txs(&mut self) {
        self.stack_pointer = self.register_x;
    }

    fn tya(&mut self) {
        self.register_a = self.register_y;
        self.update_zero_neg(self.register_a);
    }

    // Unofficial opcodes below this

    fn lax(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_addr(mode);
        self.register_a = self.bus.read(addr);
        self.register_x = self.register_a;
        self.update_zero_neg(self.register_x);
    }

    fn sax(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_addr(mode);
        self.bus.write(addr, self.register_x & self.register_a);
    }

    fn dcp(&mut self, mode: &AddressingMode) {
        self.dec(mode);
        self.compare(self.register_a, mode);
    }

    fn isb(&mut self, mode: &AddressingMode) {
        self.inc(mode);
        self.adc(mode, true);
    }

    fn slo(&mut self, mode: &AddressingMode) {
        self.asl(mode);
        self.ora(mode);
    }

    fn rla(&mut self, mode: &AddressingMode) {
        self.rol(mode);
        self.and(mode);
    }

    fn sre(&mut self, mode: &AddressingMode) {
        self.lsr(mode);
        self.eor(mode);
    }

    fn rra(&mut self, mode: &AddressingMode) {
        self.ror(mode);
        self.adc(mode, false);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::cartridge::mappers::*;
    use crate::cartridge::Cartridge;
    use std::sync::mpsc;

    fn dummy_cart() -> Cartridge {
        Cartridge {
            mapper: get_mapper(0, vec![0; 0x4000], vec![0; 0x2000], 0, Mirroring::Vertical)
                .unwrap(),
        }
    }

    fn dummy_bus() -> Bus<'static> {
        let (tx, _) = mpsc::channel::<Vec<f32>>();
        Bus::new(dummy_cart(), tx, |_, _| ())
    }

    #[test]
    fn test_lda_immediate() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xa9, 0x7F]);
        cpu._run();
        assert_eq!(cpu.register_a, 0x7F);
        assert!(!cpu.status.zero());
        assert!(!cpu.status.negative());
    }

    #[test]
    fn test_lda_immediate_zero() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xa9, 0x00]);
        cpu._run();
        assert_eq!(cpu.register_a, 0x00);
        assert!(cpu.status.zero());
        assert!(!cpu.status.negative());
    }

    #[test]
    fn test_lda_immediate_neg() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xa9, 0xFF]);
        cpu._run();
        assert_eq!(cpu.register_a, 0xFF);
        assert!(!cpu.status.zero());
        assert!(cpu.status.negative());
    }

    #[test]
    fn test_tax_a_to_x() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xAA]);
        cpu.register_a = 0xFF;
        cpu._run();
        assert_eq!(cpu.register_x, 0xFF);
        assert!(!cpu.status.zero());
        assert!(cpu.status.negative());
    }

    #[test]
    fn test_inx_x_to_nonzero() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xE8]);
        cpu.register_x = 0x56;
        cpu._run();
        assert_eq!(cpu.register_x, 0x57);
        assert!(!cpu.status.zero());
        assert!(!cpu.status.negative());
    }

    #[test]
    fn test_inx_x_to_negative() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xE8]);
        cpu.register_x = 0x7F;
        cpu._run();
        assert_eq!(cpu.register_x, 0x80);
        assert!(!cpu.status.zero());
        assert!(cpu.status.negative());
    }

    #[test]
    fn test_inx_x_to_zero() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xE8]);
        cpu.register_x = 0xFF;
        cpu._run();
        assert_eq!(cpu.register_x, 0x0);
        assert!(cpu.status.zero());
        assert!(!cpu.status.negative());
    }

    #[test]
    fn test_iny() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xc8]);
        cpu.register_y = 0x50;
        cpu._run();
        assert_eq!(cpu.register_y, 0x51);
        assert!(!cpu.status.zero());
        assert!(!cpu.status.negative());
    }

    #[test]
    fn test_lda_zeropage() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xa5, 0x4]);
        cpu.bus.write(0x4, 0x56);
        cpu._run();
        assert_eq!(cpu.register_a, 0x56);
    }

    #[test]
    fn test_lda_zeropagex() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xb5, 0x4]);
        cpu.register_x = 5;
        cpu.register_y = 6;
        cpu.bus.write(0x9, 0x56);
        cpu._run();
        assert_eq!(cpu.register_a, 0x56);
    }

    #[test]
    fn test_lda_absolute() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xbd, 0x4, 0x5]);
        cpu.register_x = 5;
        cpu.register_y = 6;
        cpu.bus.write(0x0504 + 5, 0x56);
        cpu._run();
        assert_eq!(cpu.register_a, 0x56);
    }

    #[test]
    fn test_lda_absolutex() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xbd, 0x4, 0x5]);
        cpu.register_x = 5;
        cpu.register_y = 6;
        cpu.bus.write(0x0504 + 5, 0x56);
        cpu._run();
        assert_eq!(cpu.register_a, 0x56);
    }

    #[test]
    fn test_lda_absolutey() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xb9, 0x4, 0x5]);
        cpu.register_x = 5;
        cpu.register_y = 6;
        cpu.bus.write(0x0504 + 6, 0x56);
        cpu._run();
        assert_eq!(cpu.register_a, 0x56);
    }

    #[test]
    fn test_lda_indirectx() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xa1, 0x4]);
        cpu.register_x = 5;
        cpu.register_y = 8;
        cpu.bus.write(9, 0x23);
        cpu.bus.write(10, 0x14);
        cpu.bus.write(0x1423, 0x56);
        cpu._run();
        assert_eq!(cpu.register_a, 0x56);
    }

    #[test]
    fn test_lda_indirecty() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xb1, 0x4]);
        cpu.register_x = 5;
        cpu.register_y = 8;
        cpu.bus.write(4, 0x23);
        cpu.bus.write(5, 0x14);
        cpu.bus.write(0x1423 + 8, 0x56);
        cpu._run();
        assert_eq!(cpu.register_a, 0x56);
    }

    #[test]
    fn test_inc_zeropagex() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xf6, 0xFF]);
        cpu.register_x = 5;
        cpu.register_y = 8;
        cpu.bus.write(4, 0x7f);
        cpu._run();
        assert_eq!(cpu.bus.read(4), 0x80);
        assert!(!cpu.status.zero());
        assert!(cpu.status.negative());
    }

    #[test]
    fn test_adc_set_carry() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x69, 0xa0]);
        cpu.register_a = 0xc0;
        cpu.register_x = 5;
        cpu.register_y = 8;
        cpu._run();
        assert_eq!(cpu.register_a, 0x60);
        assert!(!cpu.status.zero());
        assert!(!cpu.status.negative());
        assert!(cpu.status.carry());
        assert!(cpu.status.overflow());
    }

    #[test]
    fn test_adc_overflow_with_carry() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x69, 0x50]);
        cpu.register_a = 0x30;
        cpu.register_x = 5;
        cpu.register_y = 8;
        cpu.status.set_carry(true);
        cpu._run();
        assert_eq!(cpu.register_a, 0x81); // 80 + 48 + 1 = negative number
        assert!(!cpu.status.zero());
        assert!(cpu.status.negative());
        assert!(!cpu.status.carry());
        assert!(cpu.status.overflow());
    }

    #[test]
    fn test_and_immediate() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x29, 0xaa]);
        cpu.register_a = 0xf0;
        cpu._run();
        assert_eq!(cpu.register_a, 0xa0);
        assert!(!cpu.status.zero());
        assert!(cpu.status.negative());
    }

    #[test]
    fn test_asl_acc() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x0a]);
        cpu.register_a = 0xAA;
        cpu._run();
        assert_eq!(cpu.register_a, 0x54);
        assert!(!cpu.status.zero());
        assert!(!cpu.status.negative());
        assert!(cpu.status.carry());
    }

    #[test]
    fn test_asl_absolute() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x0e, 0xaa, 0x05]);
        cpu.bus.write(0x05aa, 0x55);
        cpu._run();
        assert_eq!(cpu.bus.read(0x05aa), 0xaa);
        assert!(!cpu.status.zero());
        assert!(cpu.status.negative());
        assert!(!cpu.status.carry());
    }

    #[test]
    fn test_bcc_carry_clear_positive() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x90, 0x15]);
        cpu._run();
        // Address of next instruction (2) + jump offset (0x15) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x618);
    }

    #[test]
    fn test_bcs_bcc_carry_clear_negative_jump() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xB0, 0x15, 0x90, 0xFA]);
        cpu._run();
        // Address of next instruction (2) - jump offset (0x6) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x5FF);
    }

    #[test]
    fn test_bcc_bcs_carry_set() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x90, 0x15, 0xB0, 0x15]);
        cpu.status.set_carry(true);
        cpu._run();
        // Address of next instruction (4) + jump offset (0x15) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x61a);
    }

    #[test]
    fn test_beq_bne_zero_clear() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xF0, 0x15, 0xD0, 0xFA]);
        cpu._run();
        // Address of next instruction (2) - jump offset (0x6) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x5FF);
    }

    #[test]
    fn test_bne_beq_zero_set() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xD0, 0x15, 0xF0, 0x15]);
        cpu.status.set_zero(true);
        cpu._run();
        // Address of next instruction (4) + jump offset (0x15) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x61a);
    }

    #[test]
    fn test_bit_nonzero() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x24, 0x00]);
        cpu.bus.write(0, 0xFF);
        cpu.register_a = 0xC0;
        cpu._run();
        assert!(!cpu.status.zero());
        assert!(cpu.status.negative());
        assert!(cpu.status.overflow());
    }

    #[test]
    fn test_bit_zero() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x24, 0x00]);
        cpu.bus.write(0, 0xF0);
        cpu.register_a = 0x0F;
        cpu._run();
        assert!(cpu.status.zero());
        assert!(cpu.status.negative());
        assert!(cpu.status.overflow());
    }

    #[test]
    fn test_bmi_bpl_negative_clear() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x30, 0x15, 0x10, 0xFA]);
        cpu._run();
        // Address of next instruction (2) - jump offset (0x6) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x5FF);
    }

    #[test]
    fn test_bpl_bmi_negative_set() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x10, 0x15, 0x30, 0x15]);
        cpu.status.set_negative(true);
        cpu._run();
        // Address of next instruction (4) + jump offset (0x15) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x61a);
    }

    #[test]
    fn test_bvs_bvc_overflow_clear() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x70, 0x15, 0x50, 0xFA]);
        cpu._run();
        // Address of next instruction (2) - jump offset (0x6) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x5FF);
    }

    #[test]
    fn test_bvc_bvs_overflow_set() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x50, 0x15, 0x70, 0x15]);
        cpu.status.set_overflow(true);
        cpu._run();
        // Address of next instruction (4) + jump offset (0x15) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x61a);
    }

    #[test]
    fn test_clc() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x18]);
        cpu.status.set_carry(true);
        cpu._run();
        assert!(!cpu.status.carry());
    }

    #[test]
    fn test_cld() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xd8]);
        cpu.status.set_decimal(true);
        cpu._run();
        assert!(!cpu.status.decimal());
    }

    #[test]
    fn test_cli() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x58]);
        cpu.status.set_irq_disable(true);
        cpu._run();
        assert!(!cpu.status.irq_disable());
    }

    #[test]
    fn test_clv() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xb8]);
        cpu.status.set_overflow(true);
        cpu._run();
        assert!(!cpu.status.overflow());
    }

    #[test]
    fn test_cmp_immediate_a_greater() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xC9, 0x10]);
        cpu.register_a = 0x20;
        cpu._run();
        assert!(cpu.status.carry());
        assert!(!cpu.status.zero());
        assert!(!cpu.status.negative());
    }

    #[test]
    fn test_cmp_immediate_equal() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xC9, 0xc0]);
        cpu.register_a = 0xc0;
        cpu._run();
        assert!(cpu.status.carry());
        assert!(cpu.status.zero());
        assert!(!cpu.status.negative());
    }

    #[test]
    fn test_cmp_immediate_a_less() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xC9, 0x20]);
        cpu.register_a = 0x10;
        cpu._run();
        assert!(!cpu.status.carry());
        assert!(!cpu.status.zero());
        assert!(cpu.status.negative());
    }

    #[test]
    fn test_cpx_immediate_x_greater() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xe0, 0x10]);
        cpu.register_x = 0x20;
        cpu._run();
        assert!(cpu.status.carry());
        assert!(!cpu.status.zero());
        assert!(!cpu.status.negative());
    }

    #[test]
    fn test_cpy_immediate_y_less() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xC0, 0x20]);
        cpu.register_y = 0x10;
        cpu._run();
        assert!(!cpu.status.carry());
        assert!(!cpu.status.zero());
        assert!(cpu.status.negative());
    }

    #[test]
    fn test_dec_zeropage() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xc6, 0x50]);
        cpu.register_x = 5;
        cpu.register_y = 8;
        cpu.bus.write(0x50, 0x01);
        cpu._run();
        assert_eq!(cpu.bus.read(0x50), 0x0);
        assert!(cpu.status.zero());
        assert!(!cpu.status.negative());
    }

    #[test]
    fn test_dex_zeropage() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xca]);
        cpu.register_x = 0x80;
        cpu._run();
        assert_eq!(cpu.register_x, 0x7F);
        assert!(!cpu.status.zero());
        assert!(!cpu.status.negative());
    }

    #[test]
    fn test_dey_zeropage() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x88]);
        cpu.register_y = 0x81;
        cpu._run();
        assert_eq!(cpu.register_y, 0x80);
        assert!(!cpu.status.zero());
        assert!(cpu.status.negative());
    }

    #[test]
    fn test_eor_immediate_zero() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x49, 0xaa]);
        cpu.register_a = 0xaa;
        cpu._run();
        assert_eq!(cpu.register_a, 0x00);
        assert!(cpu.status.zero());
        assert!(!cpu.status.negative());
    }

    #[test]
    fn test_eor_immediate_nonzero() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x49, 0xaa]);
        cpu.register_a = 0xa5;
        cpu._run();
        assert_eq!(cpu.register_a, 0x0F);
        assert!(!cpu.status.zero());
        assert!(!cpu.status.negative());
    }

    #[test]
    fn test_jmp_absolute() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x4c, 0x23, 0x01]);
        cpu._run();
        assert_eq!(cpu.program_counter, 0x0124);
    }

    #[test]
    fn test_jmp_indirect() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x6c, 0x23, 0x01]);
        cpu.bus.write(0x123, 0x44);
        cpu.bus.write(0x124, 0x02);
        cpu._run();
        assert_eq!(cpu.program_counter, 0x0245);
    }

    #[test]
    fn test_jsr() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        // First jump to some address
        cpu._setup(vec![0x4c, 0x20, 0x04]);
        // From there jump to subroutine
        cpu.stack_pointer = 0x8;
        cpu.bus.write(0x0420, 0x20);
        cpu.bus.write(0x0421, 0x04); // Jump target is BRK
        cpu.bus.write(0x0422, 0x06);
        cpu._run();
        assert_eq!(cpu.program_counter, 0x0605); // one cycle added from BRK
        assert_eq!(cpu.stack_pointer, 0x6);
        assert_eq!(cpu.bus.read(0x108), 0x04);
        assert_eq!(cpu.bus.read(0x107), 0x22);
    }

    #[test]
    fn test_ldx_zeropagey() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xb6, 0x70]);
        cpu.register_y = 0xf;
        cpu.bus.write(0x7f, 0x90);
        cpu._run();
        assert_eq!(cpu.register_x, 0x90);
        assert!(!cpu.status.zero());
        assert!(cpu.status.negative());
    }

    #[test]
    fn test_ldy_immediate() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xa0, 0x00]);
        cpu.register_y = 0xff;
        cpu._run();
        assert_eq!(cpu.register_y, 0x00);
        assert!(cpu.status.zero());
        assert!(!cpu.status.negative());
    }

    #[test]
    fn test_lsr_to_zero() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x4a]);
        cpu.register_a = 0x01;
        cpu._run();
        assert_eq!(cpu.register_a, 0x00);
        assert!(cpu.status.carry());
        assert!(cpu.status.zero());
        assert!(!cpu.status.negative());
    }

    #[test]
    fn test_nop() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xea, 0xea, 0xea]);
        cpu._run();
        assert_eq!(cpu.program_counter, 0x604);
    }

    #[test]
    fn test_ora_immediate() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x09, 0xa2]);
        cpu.register_a = 0x55;
        cpu._run();
        assert_eq!(cpu.register_a, 0xf7);
        assert!(!cpu.status.zero());
        assert!(cpu.status.negative());
    }

    #[test]
    fn test_pha() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x48]);
        cpu.stack_pointer = 0xfc;
        cpu.register_a = 0x55;
        cpu._run();
        assert_eq!(cpu.bus.read(0x01fc), 0x55);
        assert_eq!(cpu.stack_pointer, 0xfb);
    }

    #[test]
    fn test_php() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x08]);
        cpu.stack_pointer = 0x00;
        cpu.status.set_carry(true);
        cpu.status.set_negative(true);
        cpu._run();
        assert_eq!(cpu.bus.read(0x0100), 0xb5);
        assert_eq!(cpu.stack_pointer, 0xff);
    }

    #[test]
    fn test_pla() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x68]);
        cpu.stack_pointer = 0xfc;
        cpu.bus.write(0x01fd, 0x55);
        cpu._run();
        assert_eq!(cpu.register_a, 0x55);
        assert_eq!(cpu.stack_pointer, 0xfd);
    }

    #[test]
    fn test_plp() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x28]);
        cpu.stack_pointer = 0xff;
        cpu.bus.write(0x0100, 0x81);
        cpu._run();
        assert!(cpu.status.carry());
        assert!(cpu.status.negative());
        assert_eq!(cpu.stack_pointer, 0x00);
    }

    #[test]
    fn test_rol_a_carry_in() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x2a]);
        cpu.register_a = 0x42;
        cpu.status.set_carry(true);
        cpu._run();
        assert!(!cpu.status.carry());
        assert!(cpu.status.negative());
        assert_eq!(cpu.register_a, 0x85);
    }

    #[test]
    fn test_rol_zeropage_carry_out() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x26, 0x01]);
        cpu.bus.write(0x0001, 0x87);
        cpu._run();
        assert!(cpu.status.carry());
        assert!(!cpu.status.negative());
        assert_eq!(cpu.bus.read(0x0001), 0x0E);
    }

    #[test]
    fn test_ror_a_carry_in() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x6a]);
        cpu.register_a = 0x42;
        cpu.status.set_carry(true);
        cpu._run();
        assert!(!cpu.status.carry());
        assert!(cpu.status.negative());
        assert_eq!(cpu.register_a, 0xa1);
    }

    #[test]
    fn test_ror_zeropage_carry_out() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x66, 0x01]);
        cpu.bus.write(0x0001, 0x87);
        cpu._run();
        assert!(cpu.status.carry());
        assert!(!cpu.status.negative());
        assert_eq!(cpu.bus.read(0x0001), 0x43);
    }

    #[test]
    fn test_rti() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x40]);
        cpu.stack_pointer = 0x5;
        cpu.bus.write(0x0106, 0x81);
        cpu.bus.write(0x0107, 0x20);
        cpu.bus.write(0x0108, 0x13);
        cpu._run();
        assert!(cpu.status.carry());
        assert!(cpu.status.negative());
        assert_eq!(cpu.stack_pointer, 0x8);
        assert_eq!(cpu.program_counter, 0x1321);
    }

    #[test]
    fn test_rts() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x60]);
        cpu.stack_pointer = 0xfe;
        cpu.bus.write(0x01ff, 0x20);
        cpu.bus.write(0x0100, 0x0b);
        cpu._run();
        assert_eq!(cpu.stack_pointer, 0x00);
        assert_eq!(cpu.program_counter, 0x0b22);
    }

    #[test]
    fn test_sbc_keep_carry() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xe9, 0x10]);
        cpu.register_a = 0x31;
        cpu.status.set_carry(true);
        cpu._run();
        assert_eq!(cpu.register_a, 0x21);
        assert!(cpu.status.carry()); // no overflow so carry should stay
        assert!(!cpu.status.negative());
        assert!(!cpu.status.overflow());
        assert!(!cpu.status.zero());
    }

    #[test]
    fn test_sbc_no_carry_to_zero() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xe9, 0x30]);
        cpu.register_a = 0x31;
        cpu._run();
        assert_eq!(cpu.register_a, 0x00);
        assert!(cpu.status.carry());
        assert!(!cpu.status.negative());
        assert!(!cpu.status.overflow());
        assert!(cpu.status.zero());
    }

    #[test]
    fn test_sbc_consume_carry() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xe9, 0x40]);
        cpu.register_a = 0x30;
        cpu.status.set_carry(true);
        cpu._run();
        assert_eq!(cpu.register_a, 0xf0);
        assert!(!cpu.status.carry()); // carry should be consumed
        assert!(cpu.status.negative());
        assert!(!cpu.status.overflow());
        assert!(!cpu.status.zero());
    }

    #[test]
    fn test_sbc_overflow() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xe9, 0x10]);
        cpu.register_a = 0x88; // -120
        cpu.status.set_carry(true);
        cpu._run();
        assert_eq!(cpu.register_a, 0x78); // -120-16 turns into +120
        assert!(cpu.status.carry()); // does not consume carry
        assert!(!cpu.status.negative());
        assert!(cpu.status.overflow());
        assert!(!cpu.status.zero());
    }

    #[test]
    fn test_sec() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x38]);
        cpu._run();
        assert!(cpu.status.carry());
    }

    #[test]
    fn test_sed() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xf8]);
        cpu.status.set_decimal(true);
        cpu._run();
        assert!(cpu.status.decimal());
    }

    #[test]
    fn test_sei() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x78]);
        cpu._run();
        assert!(cpu.status.irq_disable());
    }

    #[test]
    fn test_sta() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x85, 0x01]);
        cpu.register_a = 0x78;
        cpu._run();
        assert_eq!(cpu.bus.read(0x01), 0x78);
    }

    #[test]
    fn test_stx() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x86, 0x01]);
        cpu.register_x = 0x78;
        cpu._run();
        assert_eq!(cpu.bus.read(0x01), 0x78);
    }

    #[test]
    fn test_sty() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x84, 0x01]);
        cpu.register_y = 0x78;
        cpu._run();
        assert_eq!(cpu.bus.read(0x01), 0x78);
    }

    #[test]
    fn test_tay() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xa8]);
        cpu.register_a = 0;
        cpu.register_y = 0x78;
        cpu._run();
        assert_eq!(cpu.register_y, 0x00);
        assert!(!cpu.status.negative());
        assert!(cpu.status.zero());
    }

    #[test]
    fn test_tsx() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0xba]);
        cpu.stack_pointer = 0xa5;
        cpu._run();
        assert_eq!(cpu.register_x, 0xa5);
        assert!(cpu.status.negative());
        assert!(!cpu.status.zero());
    }

    #[test]
    fn test_txa() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x8a]);
        cpu.register_x = 0xa5;
        cpu._run();
        assert_eq!(cpu.register_a, 0xa5);
        assert!(cpu.status.negative());
        assert!(!cpu.status.zero());
    }

    #[test]
    fn test_txs() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x9a]);
        cpu.register_x = 0x55;
        cpu.status.set_zero(true);
        cpu.status.set_negative(true);
        cpu._run();
        assert_eq!(cpu.stack_pointer, 0x55);
        assert!(cpu.status.negative()); // does not affect flags
        assert!(cpu.status.zero());
    }

    #[test]
    fn test_tya() {
        let bus = dummy_bus();
        let mut cpu = Cpu::new(bus);
        cpu._setup(vec![0x98]);
        cpu.register_y = 0xa5;
        cpu._run();
        assert_eq!(cpu.register_a, 0xa5);
        assert!(cpu.status.negative());
        assert!(!cpu.status.zero());
    }
}
