use std::ops::Add;

#[derive(Debug)]
pub struct Cpu {
    register_a: u8,
    register_x: u8,
    register_y: u8,
    program_counter: u16,
    stack_pointer: u8,
    status: StatusFlags,
    memory: [u8; 0x10000],
}

#[derive(Debug)]
struct StatusFlags {
    carry: bool,
    zero: bool,
    irq: bool,
    decimal: bool,
    break_cmd: u8,
    overflow: bool,
    negative: bool,
}

impl From<StatusFlags> for u8 {
    fn from(status: StatusFlags) -> Self {
        (status.carry as u8) << 0
            | (status.zero as u8) << 1
            | (status.irq as u8) << 2
            | (status.decimal as u8) << 3
            | (status.break_cmd as u8) << 4
            | (status.overflow as u8) << 6
            | (status.negative as u8) << 7
    }
}

impl From<u8> for StatusFlags {
    fn from(status: u8) -> Self {
        StatusFlags {
            carry: status & CARRY_FLAG != 0,
            zero: status & ZERO_FLAG != 0,
            irq: status & IRQ_FLAG != 0,
            decimal: status & DECIMAL_FLAG != 0,
            break_cmd: (status & BREAK_CMD) >> 4,
            overflow: status & OVERFLOW_FLAG != 0,
            negative: status & NEGATIVE_FLAG != 0,
        }
    }
}

const CARRY_FLAG: u8 = 0x1 << 0;
const ZERO_FLAG: u8 = 0x1 << 1;
const IRQ_FLAG: u8 = 0x1 << 2;
const DECIMAL_FLAG: u8 = 0x1 << 3;
const BREAK_CMD: u8 = 0x3 << 4;
const OVERFLOW_FLAG: u8 = 0x1 << 6;
const NEGATIVE_FLAG: u8 = 0x1 << 7;

const SIGN_MASK: u8 = 0x1 << 7;
const RESET_ADDR: u16 = 0xFFFC;

#[derive(Debug)]
enum AddressingMode {
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    IndirectX,
    IndirectY,
    NoneAddressing,
}

#[derive(Debug)]
struct Instruction {
    opcode: u8,
    mnemonic: &'static str,
    addressing_mode: AddressingMode,
    bytes: u8,
    duration: u8,
}

impl Instruction {
    pub fn new(
        opcode: u8,
        mnemonic: &'static str,
        bytes: u8,
        duration: u8,
        addressing_mode: AddressingMode,
    ) -> Self {
        Instruction {
            opcode,
            mnemonic,
            bytes,
            duration,
            addressing_mode,
        }
    }
}

lazy_static::lazy_static! {
    static ref INSTRUCTIONS: Vec<Instruction> = vec![
        Instruction::new(0x00, "BRK", 1, 7, AddressingMode::NoneAddressing),

        // Loads
        Instruction::new(0xA9, "LDA", 2, 2, AddressingMode::Immediate),
        Instruction::new(0xA5, "LDA", 2, 2, AddressingMode::ZeroPage),
        Instruction::new(0xB5, "LDA", 2, 2, AddressingMode::ZeroPageX),
        Instruction::new(0xAD, "LDA", 3, 4, AddressingMode::Absolute),
        Instruction::new(0xBD, "LDA", 3, 4, AddressingMode::AbsoluteX), // +1 if page crossed
        Instruction::new(0xB9, "LDA", 3, 4, AddressingMode::AbsoluteY), // +1 if page crossed
        Instruction::new(0xA1, "LDA", 2, 6, AddressingMode::IndirectX),
        Instruction::new(0xB1, "LDA", 2, 5, AddressingMode::IndirectY), // +1 if page crossed

        // Increments
        Instruction::new(0xE6, "INC", 2, 5, AddressingMode::ZeroPage),
        Instruction::new(0xF6, "INC", 2, 6, AddressingMode::ZeroPageX),
        Instruction::new(0xEE, "INC", 3, 6, AddressingMode::Absolute),
        Instruction::new(0xFE, "INC", 3, 7, AddressingMode::AbsoluteX),
        Instruction::new(0xE8, "INX", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0xC8, "INY", 1, 2, AddressingMode::NoneAddressing),

        // Transfers
        Instruction::new(0xAA, "TAX", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0xA8, "TAY", 1, 2, AddressingMode::NoneAddressing),

        // Addition
        Instruction::new(0x69, "ADC", 2, 2, AddressingMode::Immediate),
        Instruction::new(0x65, "ADC", 2, 3, AddressingMode::ZeroPage),
        Instruction::new(0x75, "ADC", 2, 4, AddressingMode::ZeroPageX),
        Instruction::new(0x6D, "ADC", 3, 4, AddressingMode::Absolute),
        Instruction::new(0x7D, "ADC", 3, 4, AddressingMode::AbsoluteX), // +1 if page crossed
        Instruction::new(0x79, "ADC", 3, 4, AddressingMode::AbsoluteY), // +1 if page crossed
        Instruction::new(0x61, "ADC", 2, 6, AddressingMode::IndirectX),
        Instruction::new(0x71, "ADC", 2, 5, AddressingMode::IndirectY), // +1 if page crossed

        // Logical AND
        Instruction::new(0x29, "AND", 2, 2, AddressingMode::Immediate),
        Instruction::new(0x25, "AND", 2, 3, AddressingMode::ZeroPage),
        Instruction::new(0x35, "AND", 2, 4, AddressingMode::ZeroPageX),
        Instruction::new(0x2D, "AND", 3, 4, AddressingMode::Absolute),
        Instruction::new(0x3D, "AND", 3, 4, AddressingMode::AbsoluteX), // +1 if page crossed
        Instruction::new(0x39, "AND", 3, 4, AddressingMode::AbsoluteY), // +1 if page crossed
        Instruction::new(0x21, "AND", 2, 6, AddressingMode::IndirectX),
        Instruction::new(0x31, "AND", 2, 5, AddressingMode::IndirectY), // +1 if page crossed

        // Arithmetic shift left
        Instruction::new(0x0A, "ASL", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0x06, "ASL", 2, 5, AddressingMode::ZeroPage),
        Instruction::new(0x16, "ASL", 2, 6, AddressingMode::ZeroPageX),
        Instruction::new(0x0E, "ASL", 3, 6, AddressingMode::Absolute),
        Instruction::new(0x1E, "ASL", 3, 7, AddressingMode::AbsoluteX), // +1 if page crossed

        Instruction::new(0x24, "BIT", 2, 3, AddressingMode::ZeroPage),
        Instruction::new(0x2C, "BIT", 3, 4, AddressingMode::Absolute),

        // Branches - +1 duration if branch succeeds, +1 if page crossed
        Instruction::new(0x90, "BCC", 2, 2, AddressingMode::NoneAddressing),
        Instruction::new(0xB0, "BCS", 2, 2, AddressingMode::NoneAddressing),
        Instruction::new(0xF0, "BEQ", 2, 2, AddressingMode::NoneAddressing),
        Instruction::new(0x30, "BMI", 2, 2, AddressingMode::NoneAddressing),
        Instruction::new(0xD0, "BNE", 2, 2, AddressingMode::NoneAddressing),
        Instruction::new(0x10, "BPL", 2, 2, AddressingMode::NoneAddressing),
        Instruction::new(0x50, "BVC", 2, 2, AddressingMode::NoneAddressing),
        Instruction::new(0x70, "BVS", 2, 2, AddressingMode::NoneAddressing),

        // Flag interaction
        Instruction::new(0x18, "CLC", 1, 2, AddressingMode::NoneAddressing),
    ];
}

impl Cpu {
    pub fn new() -> Cpu {
        Cpu {
            register_a: 0,
            register_x: 0,
            register_y: 0,
            program_counter: 0,
            stack_pointer: 0,
            status: StatusFlags {
                carry: false,
                zero: false,
                irq: false,
                decimal: false,
                break_cmd: 0,
                overflow: false,
                negative: false,
            },
            memory: [0; 0x10000],
        }
    }

    fn reset(&mut self) {
        self.register_a = 0;
        self.register_x = 0;
        self.register_y = 0;
        self.status = StatusFlags {
            carry: false,
            zero: false,
            irq: false,
            decimal: false,
            break_cmd: 0,
            overflow: false,
            negative: false,
        };

        self.program_counter = self.read_mem_u16(RESET_ADDR);
    }

    fn update_zero_neg(&mut self, val: u8) {
        self.status.zero = val == 0;
        self.status.negative = val >= 128;
    }

    fn read_mem(&self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }

    fn read_mem_u16(&self, addr: u16) -> u16 {
        let int_bytes = &self.memory[addr as usize..=(addr + 1) as usize];
        u16::from_le_bytes(int_bytes.try_into().unwrap())
    }

    fn write_mem(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize] = data;
    }

    fn write_mem_u16(&mut self, addr: u16, data: u16) {
        self.memory[(addr as usize)..=((addr + 1) as usize)].copy_from_slice(&data.to_le_bytes());
    }

    fn setup(&mut self, rom: Vec<u8>) {
        self.memory[0x8000..0x8000 + rom.len()].copy_from_slice(&rom);
        self.write_mem_u16(RESET_ADDR, 0x8000);
        self.reset();
    }

    pub fn setup_and_run(&mut self, rom: Vec<u8>) {
        self.setup(rom);
        self.run();
    }

    fn get_operand_addr(&self, mode: &AddressingMode) -> u16 {
        match mode {
            // Data is the parameter
            AddressingMode::Immediate => self.program_counter,

            // Data is in page zero i.e. 0x0000 - 0x00FF, at the index indicated by parameter
            AddressingMode::ZeroPage => self.read_mem(self.program_counter) as u16,

            // Data is in page zero, at the index indicated by parameter + X
            AddressingMode::ZeroPageX => {
                let addr = self.read_mem(self.program_counter);
                addr.wrapping_add(self.register_x) as u16
            }

            // Data is in page zero, at the index indicated by parameter + Y
            AddressingMode::ZeroPageY => {
                let addr = self.read_mem(self.program_counter);
                addr.wrapping_add(self.register_y) as u16
            }

            // Data is in the address indicated by 2-byte parameter
            AddressingMode::Absolute => self.read_mem_u16(self.program_counter),

            // Data is in the address indicated by 2-byte parameter incremented by X
            AddressingMode::AbsoluteX => {
                let addr = self.read_mem_u16(self.program_counter);
                addr.wrapping_add(self.register_x as u16)
            }

            // Data is in the address indicated by 2-byte parameter incremented by Y
            AddressingMode::AbsoluteY => {
                let addr = self.read_mem_u16(self.program_counter);
                addr.wrapping_add(self.register_y as u16)
            }

            // Data is in address indicated by pointer indicated by (parameter + X)
            AddressingMode::IndirectX => {
                let param = self.read_mem(self.program_counter);
                let lsb = self.read_mem(param.wrapping_add(self.register_x) as u16) as u16;
                let msb = self.read_mem(param.wrapping_add(self.register_x).wrapping_add(1) as u16)
                    as u16;
                msb << 8 | lsb
            }

            // Data is in address indicated by (pointer indicated by parameter) + Y
            AddressingMode::IndirectY => {
                let param = self.read_mem(self.program_counter);
                let lsb = self.read_mem(param as u16) as u16;
                let msb = self.read_mem(param.wrapping_add(1) as u16) as u16;
                (msb << 8 | lsb).wrapping_add(self.register_y as u16)
            }

            AddressingMode::NoneAddressing => panic!("mode {:?} is not supported", mode),
        }
    }

    fn adc(&mut self, mode: &AddressingMode) {
        let operand = self.read_mem(self.get_operand_addr(mode));
        let carry = if self.status.carry { 1 } else { 0 };

        let orig_a = self.register_a;
        self.register_a = orig_a.wrapping_add(operand).wrapping_add(carry);

        // Overflow if both inputs are different sign than result
        self.status.overflow =
            (orig_a ^ self.register_a) & (operand ^ self.register_a) & SIGN_MASK == SIGN_MASK;

        // Carry if new value is smaller, or value from operand was 0xFF and carry was set
        self.status.carry = self.register_a < orig_a || self.register_a == orig_a && carry > 0;

        self.update_zero_neg(self.register_a);
    }

    fn and(&mut self, mode: &AddressingMode) {
        let operand = self.read_mem(self.get_operand_addr(mode));
        self.register_a = operand & self.register_a;
        self.update_zero_neg(self.register_a);
    }

    fn asl(&mut self, mode: &AddressingMode) {
        // NoneAddressing works directly on accumulator
        // MSB shifts to carry bit
        match mode {
            &AddressingMode::NoneAddressing => {
                self.status.carry = self.register_a & SIGN_MASK == SIGN_MASK;
                self.register_a = self.register_a << 1;
                self.update_zero_neg(self.register_a);
            }
            _ => {
                let addr = self.get_operand_addr(mode);
                let operand = self.read_mem(addr);
                self.status.carry = operand & SIGN_MASK == SIGN_MASK;
                let result = operand << 1;
                self.write_mem(addr, result);
                self.update_zero_neg(result);
            }
        }
    }

    fn branch_relative(&mut self) {
        let offset = ((self.read_mem(self.program_counter) as i8) as i16) as u16;
        self.program_counter = self.program_counter.wrapping_add(offset);
    }

    fn bcc(&mut self) {
        if !self.status.carry {
            self.branch_relative();
        }
    }

    fn bcs(&mut self) {
        if self.status.carry {
            self.branch_relative();
        }
    }

    fn beq(&mut self) {
        if self.status.zero {
            self.branch_relative();
        }
    }

    fn bmi(&mut self) {
        if self.status.negative {
            self.branch_relative();
        }
    }

    fn bne(&mut self) {
        if !self.status.zero {
            self.branch_relative();
        }
    }

    fn bpl(&mut self) {
        if !self.status.negative {
            self.branch_relative();
        }
    }

    fn bvc(&mut self) {
        if !self.status.overflow {
            self.branch_relative();
        }
    }

    fn bvs(&mut self) {
        if self.status.overflow {
            self.branch_relative();
        }
    }

    fn bit(&mut self, mode: &AddressingMode) {
        let result = self.register_a & self.read_mem(self.get_operand_addr(mode));
        self.status.zero = result == 0;
        self.status.overflow = result & OVERFLOW_FLAG == OVERFLOW_FLAG; // store bit 6
        self.status.negative = result >= 128; // and bit 7
    }

    fn inc(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_addr(mode);
        let new_val = self.read_mem(addr).wrapping_add(1);
        self.write_mem(addr, new_val);
        self.update_zero_neg(new_val);
    }

    fn inx(&mut self) {
        self.register_x = self.register_x.wrapping_add(1);
        self.update_zero_neg(self.register_x);
    }

    fn lda(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_addr(mode);
        self.register_a = self.read_mem(addr);
        self.update_zero_neg(self.register_a);
    }

    fn tax(&mut self) {
        self.register_x = self.register_a;
        self.update_zero_neg(self.register_x);
    }

    pub fn run(&mut self) {
        loop {
            println!("Executing at address 0x{:x}", self.program_counter);
            let op = self.read_mem(self.program_counter);
            self.program_counter += 1;

            let instruction: &Instruction = INSTRUCTIONS.iter().find(|&x| x.opcode == op).unwrap();
            println!("{:?}", instruction);

            let mut increment_pc = true;

            match instruction.mnemonic {
                "ADC" => self.adc(&instruction.addressing_mode),
                "AND" => self.and(&instruction.addressing_mode),
                "ASL" => self.asl(&instruction.addressing_mode),
                "BCC" => self.bcc(),
                "BCS" => self.bcs(),
                "BEQ" => self.beq(),
                "BIT" => self.bit(&instruction.addressing_mode),
                "BMI" => self.bmi(),
                "BNE" => self.bne(),
                "BPL" => self.bpl(),
                "BRK" => return,
                "BVC" => self.bvc(),
                "BVS" => self.bvs(),
                "CLC" => self.status.carry = false,
                "INC" => self.inc(&instruction.addressing_mode),
                "INX" => self.inx(),
                "LDA" => self.lda(&instruction.addressing_mode),
                "TAX" => self.tax(),
                // not yet implemented
                _ => todo!(),
            }

            if increment_pc {
                self.program_counter += (instruction.bytes - 1) as u16;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_lda_immediate() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xa9, 0x7F, 0x00]);
        cpu.run();
        assert_eq!(cpu.register_a, 0x7F);
        assert!(!cpu.status.zero);
        assert!(!cpu.status.negative);
    }

    #[test]
    fn test_lda_immediate_zero() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xa9, 0x00, 0x00]);
        cpu.run();
        assert_eq!(cpu.register_a, 0x00);
        assert!(cpu.status.zero);
        assert!(!cpu.status.negative);
    }

    #[test]
    fn test_lda_immediate_neg() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xa9, 0xFF, 0x00]);
        cpu.run();
        assert_eq!(cpu.register_a, 0xFF);
        assert!(!cpu.status.zero);
        assert!(cpu.status.negative);
    }

    #[test]
    fn test_tax_a_to_x() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xAA, 0x00]);
        cpu.register_a = 0xFF;
        cpu.run();
        assert_eq!(cpu.register_x, 0xFF);
        assert!(!cpu.status.zero);
        assert!(cpu.status.negative);
    }

    #[test]
    fn test_inx_x_to_nonzero() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xE8, 0x00]);
        cpu.register_x = 0x56;
        cpu.run();
        assert_eq!(cpu.register_x, 0x57);
        assert!(!cpu.status.zero);
        assert!(!cpu.status.negative);
    }

    #[test]
    fn test_inx_x_to_negative() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xE8, 0x00]);
        cpu.register_x = 0x7F;
        cpu.run();
        assert_eq!(cpu.register_x, 0x80);
        assert!(!cpu.status.zero);
        assert!(cpu.status.negative);
    }

    #[test]
    fn test_inx_x_to_zero() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xE8, 0x00]);
        cpu.register_x = 0xFF;
        cpu.run();
        assert_eq!(cpu.register_x, 0x0);
        assert!(cpu.status.zero);
        assert!(!cpu.status.negative);
    }

    #[test]
    fn test_lda_zeropage() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xa5, 0x4, 0x00]);
        cpu.memory[0x4] = 0x56;
        cpu.run();
        assert_eq!(cpu.register_a, 0x56);
    }

    #[test]
    fn test_lda_zeropagex() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xb5, 0x4, 0x00]);
        cpu.register_x = 5;
        cpu.register_y = 6;
        cpu.memory[0x9] = 0x56;
        cpu.run();
        assert_eq!(cpu.register_a, 0x56);
    }

    #[test]
    fn test_lda_absolute() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xbd, 0x4, 0x5, 0x00]);
        cpu.register_x = 5;
        cpu.register_y = 6;
        cpu.memory[0x0504 + 5] = 0x56;
        cpu.run();
        assert_eq!(cpu.register_a, 0x56);
    }

    #[test]
    fn test_lda_absolutex() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xb9, 0x4, 0x5, 0x00]);
        cpu.register_x = 5;
        cpu.register_y = 6;
        cpu.memory[0x0504 + 6] = 0x56;
        cpu.run();
        assert_eq!(cpu.register_a, 0x56);
    }

    #[test]
    fn test_lda_indirectx() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xa1, 0x4, 0x00]);
        cpu.register_x = 5;
        cpu.register_y = 8;
        cpu.memory[9] = 0x23;
        cpu.memory[10] = 0x24;
        cpu.memory[0x2423] = 0x56;
        cpu.run();
        assert_eq!(cpu.register_a, 0x56);
    }

    #[test]
    fn test_lda_indirecty() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xb1, 0x4, 0x00]);
        cpu.register_x = 5;
        cpu.register_y = 8;
        cpu.memory[4] = 0x23;
        cpu.memory[5] = 0x24;
        cpu.memory[0x2423 + 8] = 0x56;
        cpu.run();
        assert_eq!(cpu.register_a, 0x56);
    }

    #[test]
    fn test_inc_zeropagex() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xf6, 0xFF, 0x00]);
        cpu.register_x = 5;
        cpu.register_y = 8;
        cpu.memory[4] = 0x7f;
        cpu.run();
        assert_eq!(cpu.memory[4], 0x80);
        assert!(!cpu.status.zero);
        assert!(cpu.status.negative);
    }

    #[test]
    fn test_adc_set_carry() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x69, 0xa0, 0x00]);
        cpu.register_a = 0xc0;
        cpu.register_x = 5;
        cpu.register_y = 8;
        cpu.run();
        assert_eq!(cpu.register_a, 0x60);
        assert!(!cpu.status.zero);
        assert!(!cpu.status.negative);
        assert!(cpu.status.carry);
        assert!(cpu.status.overflow);
    }

    #[test]
    fn test_adc_overflow_with_carry() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x69, 0x50, 0x00]);
        cpu.register_a = 0x30;
        cpu.register_x = 5;
        cpu.register_y = 8;
        cpu.status.carry = true;
        cpu.run();
        assert_eq!(cpu.register_a, 0x81);
        assert!(!cpu.status.zero);
        assert!(cpu.status.negative);
        assert!(!cpu.status.carry);
        assert!(cpu.status.overflow);
    }

    #[test]
    fn test_and_immediate() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x29, 0xaa, 0x00]);
        cpu.register_a = 0xf0;
        cpu.run();
        assert_eq!(cpu.register_a, 0xa0);
        assert!(!cpu.status.zero);
        assert!(cpu.status.negative);
    }

    #[test]
    fn test_asl_acc() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x0a, 0x00]);
        cpu.register_a = 0xAA;
        cpu.run();
        assert_eq!(cpu.register_a, 0x54);
        assert!(!cpu.status.zero);
        assert!(!cpu.status.negative);
        assert!(cpu.status.carry);
    }

    #[test]
    fn test_asl_absolute() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x0e, 0xaa, 0x55, 0x00]);
        cpu.memory[0x55aa] = 0x55;
        cpu.run();
        assert_eq!(cpu.memory[0x55aa], 0xaa);
        assert!(!cpu.status.zero);
        assert!(cpu.status.negative);
        assert!(!cpu.status.carry);
    }

    #[test]
    fn test_bcc_carry_clear_positive() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x90, 0x15, 0x00]);
        cpu.run();
        // Address of next instruction (2) + jump offset (0x15) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x8018);
    }

    #[test]
    fn test_bcs_bcc_carry_clear_negative_jump() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xB0, 0x15, 0x90, 0xFA, 0x00]);
        cpu.run();
        // Address of next instruction (2) - jump offset (0x6) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x7FFF);
    }

    #[test]
    fn test_bcc_bcs_carry_set() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x90, 0x15, 0xB0, 0x15, 0x00]);
        cpu.status.carry = true;
        cpu.run();
        // Address of next instruction (4) + jump offset (0x15) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x801a);
    }

    #[test]
    fn test_beq_bne_zero_clear() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xF0, 0x15, 0xD0, 0xFA, 0x00]);
        cpu.run();
        // Address of next instruction (2) - jump offset (0x6) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x7FFF);
    }

    #[test]
    fn test_bne_beq_zero_set() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xD0, 0x15, 0xF0, 0x15, 0x00]);
        cpu.status.zero = true;
        cpu.run();
        // Address of next instruction (4) + jump offset (0x15) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x801a);
    }

    #[test]
    fn test_bit_nonzero() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x24, 0x00, 0x00]);
        cpu.memory[0] = 0xFF;
        cpu.register_a = 0xC0;
        cpu.run();
        assert!(!cpu.status.zero);
        assert!(cpu.status.negative);
        assert!(cpu.status.overflow);
    }

    #[test]
    fn test_bit_zero() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x24, 0x00, 0x00]);
        cpu.memory[0] = 0xF0;
        cpu.register_a = 0x0F;
        cpu.run();
        assert!(cpu.status.zero);
        assert!(!cpu.status.negative);
        assert!(!cpu.status.overflow);
    }

    #[test]
    fn test_bmi_bpl_negative_clear() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x30, 0x15, 0x10, 0xFA, 0x00]);
        cpu.run();
        // Address of next instruction (2) - jump offset (0x6) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x7FFF);
    }

    #[test]
    fn test_bpl_bmi_negative_set() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x10, 0x15, 0x30, 0x15, 0x00]);
        cpu.status.negative = true;
        cpu.run();
        // Address of next instruction (4) + jump offset (0x15) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x801a);
    }

    #[test]
    fn test_bvs_bvc_overflow_clear() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x70, 0x15, 0x50, 0xFA, 0x00]);
        cpu.run();
        // Address of next instruction (2) - jump offset (0x6) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x7FFF);
    }

    #[test]
    fn test_bvc_bvs_overflow_set() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x50, 0x15, 0x70, 0x15, 0x00]);
        cpu.status.overflow = true;
        cpu.run();
        // Address of next instruction (4) + jump offset (0x15) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x801a);
    }

    #[test]
    fn test_clc() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x18, 0x00]);
        cpu.status.carry = true;
        cpu.run();
        assert!(!cpu.status.carry);
    }
}
