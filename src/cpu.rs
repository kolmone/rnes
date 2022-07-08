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

#[derive(Debug, Copy, Clone)]
struct StatusFlags {
    carry: bool,
    zero: bool,
    irq_disable: bool,
    decimal: bool,
    break_cmd: bool,
    overflow: bool,
    negative: bool,
}

impl From<StatusFlags> for u8 {
    fn from(status: StatusFlags) -> Self {
        (status.carry as u8) << 0
            | (status.zero as u8) << 1
            | (status.irq_disable as u8) << 2
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
            irq_disable: status & IRQ_DISABLE_FLAG != 0,
            decimal: status & DECIMAL_FLAG != 0,
            break_cmd: status & BREAK_CMD != 0,
            overflow: status & OVERFLOW_FLAG != 0,
            negative: status & NEGATIVE_FLAG != 0,
        }
    }
}

const CARRY_FLAG: u8 = 0x1 << 0;
const ZERO_FLAG: u8 = 0x1 << 1;
const IRQ_DISABLE_FLAG: u8 = 0x1 << 2;
const DECIMAL_FLAG: u8 = 0x1 << 3;
const BREAK_CMD: u8 = 0x1 << 4;
const OVERFLOW_FLAG: u8 = 0x1 << 6;
const NEGATIVE_FLAG: u8 = 0x1 << 7;

const SIGN_MASK: u8 = 0x1 << 7;
const RESET_ADDR: u16 = 0xFFFC;
const STACK_PAGE: u16 = 0x0100;

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
        Instruction::new(0xEA, "NOP", 1, 2, AddressingMode::NoneAddressing),

        // Load A
        Instruction::new(0xA9, "LDA", 2, 2, AddressingMode::Immediate),
        Instruction::new(0xA5, "LDA", 2, 2, AddressingMode::ZeroPage),
        Instruction::new(0xB5, "LDA", 2, 2, AddressingMode::ZeroPageX),
        Instruction::new(0xAD, "LDA", 3, 4, AddressingMode::Absolute),
        Instruction::new(0xBD, "LDA", 3, 4, AddressingMode::AbsoluteX), // +1 if page crossed
        Instruction::new(0xB9, "LDA", 3, 4, AddressingMode::AbsoluteY), // +1 if page crossed
        Instruction::new(0xA1, "LDA", 2, 6, AddressingMode::IndirectX),
        Instruction::new(0xB1, "LDA", 2, 5, AddressingMode::IndirectY), // +1 if page crossed

        // Load X
        Instruction::new(0xA2, "LDX", 2, 2, AddressingMode::Immediate),
        Instruction::new(0xA6, "LDX", 2, 2, AddressingMode::ZeroPage),
        Instruction::new(0xB6, "LDX", 2, 2, AddressingMode::ZeroPageY),
        Instruction::new(0xAE, "LDX", 3, 4, AddressingMode::Absolute),
        Instruction::new(0xBE, "LDX", 3, 4, AddressingMode::AbsoluteY), // +1 if page crossed

        // Load Y
        Instruction::new(0xA0, "LDY", 2, 2, AddressingMode::Immediate),
        Instruction::new(0xA4, "LDY", 2, 2, AddressingMode::ZeroPage),
        Instruction::new(0xB4, "LDY", 2, 2, AddressingMode::ZeroPageX),
        Instruction::new(0xAC, "LDY", 3, 4, AddressingMode::Absolute),
        Instruction::new(0xBC, "LDY", 3, 4, AddressingMode::AbsoluteX), // +1 if page crossed

        // Store A
        Instruction::new(0x85, "STA", 2, 3, AddressingMode::ZeroPage),
        Instruction::new(0x95, "STA", 2, 4, AddressingMode::ZeroPageX),
        Instruction::new(0x8D, "STA", 3, 4, AddressingMode::Absolute),
        Instruction::new(0x9D, "STA", 3, 5, AddressingMode::AbsoluteX),
        Instruction::new(0x99, "STA", 3, 5, AddressingMode::AbsoluteY),
        Instruction::new(0x81, "STA", 2, 6, AddressingMode::IndirectX),
        Instruction::new(0x91, "STA", 2, 6, AddressingMode::IndirectY),

        // Store X
        Instruction::new(0x86, "STX", 2, 3, AddressingMode::ZeroPage),
        Instruction::new(0x96, "STX", 2, 4, AddressingMode::ZeroPageY),
        Instruction::new(0x8E, "STX", 3, 4, AddressingMode::Absolute),

        // Store Y
        Instruction::new(0x84, "STY", 2, 3, AddressingMode::ZeroPage),
        Instruction::new(0x94, "STY", 2, 4, AddressingMode::ZeroPageY),
        Instruction::new(0x8C, "STY", 3, 4, AddressingMode::Absolute),

        // Increments
        Instruction::new(0xE6, "INC", 2, 5, AddressingMode::ZeroPage),
        Instruction::new(0xF6, "INC", 2, 6, AddressingMode::ZeroPageX),
        Instruction::new(0xEE, "INC", 3, 6, AddressingMode::Absolute),
        Instruction::new(0xFE, "INC", 3, 7, AddressingMode::AbsoluteX),
        Instruction::new(0xE8, "INX", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0xC8, "INY", 1, 2, AddressingMode::NoneAddressing),

        // Decrements
        Instruction::new(0xC6, "DEC", 2, 5, AddressingMode::ZeroPage),
        Instruction::new(0xD6, "DEC", 2, 6, AddressingMode::ZeroPageX),
        Instruction::new(0xCE, "DEC", 3, 6, AddressingMode::Absolute),
        Instruction::new(0xDE, "DEC", 3, 7, AddressingMode::AbsoluteX),
        Instruction::new(0xCA, "DEX", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0x88, "DEY", 1, 2, AddressingMode::NoneAddressing),

        // Transfers
        Instruction::new(0xAA, "TAX", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0xA8, "TAY", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0xBA, "TSX", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0x8A, "TXA", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0x9A, "TXS", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0x98, "TYA", 1, 2, AddressingMode::NoneAddressing),

        // Pushes & pulls
        Instruction::new(0x48, "PHA", 1, 3, AddressingMode::NoneAddressing),
        Instruction::new(0x08, "PHP", 1, 3, AddressingMode::NoneAddressing),
        Instruction::new(0x68, "PLA", 1, 4, AddressingMode::NoneAddressing),
        Instruction::new(0x28, "PLP", 1, 4, AddressingMode::NoneAddressing),

        // Addition
        Instruction::new(0x69, "ADC", 2, 2, AddressingMode::Immediate),
        Instruction::new(0x65, "ADC", 2, 3, AddressingMode::ZeroPage),
        Instruction::new(0x75, "ADC", 2, 4, AddressingMode::ZeroPageX),
        Instruction::new(0x6D, "ADC", 3, 4, AddressingMode::Absolute),
        Instruction::new(0x7D, "ADC", 3, 4, AddressingMode::AbsoluteX), // +1 if page crossed
        Instruction::new(0x79, "ADC", 3, 4, AddressingMode::AbsoluteY), // +1 if page crossed
        Instruction::new(0x61, "ADC", 2, 6, AddressingMode::IndirectX),
        Instruction::new(0x71, "ADC", 2, 5, AddressingMode::IndirectY), // +1 if page crossed

        // Substraction
        Instruction::new(0xE9, "SBC", 2, 2, AddressingMode::Immediate),
        Instruction::new(0xE5, "SBC", 2, 3, AddressingMode::ZeroPage),
        Instruction::new(0xF5, "SBC", 2, 4, AddressingMode::ZeroPageX),
        Instruction::new(0xED, "SBC", 3, 4, AddressingMode::Absolute),
        Instruction::new(0xFD, "SBC", 3, 4, AddressingMode::AbsoluteX), // +1 if page crossed
        Instruction::new(0xF9, "SBC", 3, 4, AddressingMode::AbsoluteY), // +1 if page crossed
        Instruction::new(0xE1, "SBC", 2, 6, AddressingMode::IndirectX),
        Instruction::new(0xF1, "SBC", 2, 5, AddressingMode::IndirectY), // +1 if page crossed

        // Logical AND
        Instruction::new(0x29, "AND", 2, 2, AddressingMode::Immediate),
        Instruction::new(0x25, "AND", 2, 3, AddressingMode::ZeroPage),
        Instruction::new(0x35, "AND", 2, 4, AddressingMode::ZeroPageX),
        Instruction::new(0x2D, "AND", 3, 4, AddressingMode::Absolute),
        Instruction::new(0x3D, "AND", 3, 4, AddressingMode::AbsoluteX), // +1 if page crossed
        Instruction::new(0x39, "AND", 3, 4, AddressingMode::AbsoluteY), // +1 if page crossed
        Instruction::new(0x21, "AND", 2, 6, AddressingMode::IndirectX),
        Instruction::new(0x31, "AND", 2, 5, AddressingMode::IndirectY), // +1 if page crossed

        // Logical exclusive OR
        Instruction::new(0x49, "EOR", 2, 2, AddressingMode::Immediate),
        Instruction::new(0x45, "EOR", 2, 3, AddressingMode::ZeroPage),
        Instruction::new(0x55, "EOR", 2, 4, AddressingMode::ZeroPageX),
        Instruction::new(0x4D, "EOR", 3, 4, AddressingMode::Absolute),
        Instruction::new(0x5D, "EOR", 3, 4, AddressingMode::AbsoluteX), // +1 if page crossed
        Instruction::new(0x59, "EOR", 3, 4, AddressingMode::AbsoluteY), // +1 if page crossed
        Instruction::new(0x41, "EOR", 2, 6, AddressingMode::IndirectX),
        Instruction::new(0x51, "EOR", 2, 5, AddressingMode::IndirectY), // +1 if page crossed

        // Logical OR
        Instruction::new(0x09, "ORA", 2, 2, AddressingMode::Immediate),
        Instruction::new(0x05, "ORA", 2, 3, AddressingMode::ZeroPage),
        Instruction::new(0x15, "ORA", 2, 4, AddressingMode::ZeroPageX),
        Instruction::new(0x0D, "ORA", 3, 4, AddressingMode::Absolute),
        Instruction::new(0x1D, "ORA", 3, 4, AddressingMode::AbsoluteX), // +1 if page crossed
        Instruction::new(0x19, "ORA", 3, 4, AddressingMode::AbsoluteY), // +1 if page crossed
        Instruction::new(0x01, "ORA", 2, 6, AddressingMode::IndirectX),
        Instruction::new(0x11, "ORA", 2, 5, AddressingMode::IndirectY), // +1 if page crossed

        // Arithmetic shift left
        Instruction::new(0x0A, "ASL", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0x06, "ASL", 2, 5, AddressingMode::ZeroPage),
        Instruction::new(0x16, "ASL", 2, 6, AddressingMode::ZeroPageX),
        Instruction::new(0x0E, "ASL", 3, 6, AddressingMode::Absolute),
        Instruction::new(0x1E, "ASL", 3, 7, AddressingMode::AbsoluteX), // +1 if page crossed

        // Logical shift right
        Instruction::new(0x4A, "LSR", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0x46, "LSR", 2, 5, AddressingMode::ZeroPage),
        Instruction::new(0x56, "LSR", 2, 6, AddressingMode::ZeroPageX),
        Instruction::new(0x4E, "LSR", 3, 6, AddressingMode::Absolute),
        Instruction::new(0x5E, "LSR", 3, 7, AddressingMode::AbsoluteX), // +1 if page crossed

        // Rotate left
        Instruction::new(0x2A, "ROL", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0x26, "ROL", 2, 5, AddressingMode::ZeroPage),
        Instruction::new(0x36, "ROL", 2, 6, AddressingMode::ZeroPageX),
        Instruction::new(0x2E, "ROL", 3, 6, AddressingMode::Absolute),
        Instruction::new(0x3E, "ROL", 3, 7, AddressingMode::AbsoluteX), // +1 if page crossed

        // Rotate right
        Instruction::new(0x6A, "ROR", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0x66, "ROR", 2, 5, AddressingMode::ZeroPage),
        Instruction::new(0x76, "ROR", 2, 6, AddressingMode::ZeroPageX),
        Instruction::new(0x6E, "ROR", 3, 6, AddressingMode::Absolute),
        Instruction::new(0x7E, "ROR", 3, 7, AddressingMode::AbsoluteX), // +1 if page crossed

        // Check bits (with logical AND)
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

        // Jumps
        Instruction::new(0x4c, "JMP", 3, 3, AddressingMode::Absolute),
        Instruction::new(0x6c, "JMP", 3, 5, AddressingMode::NoneAddressing),
        Instruction::new(0x20, "JSR", 3, 6, AddressingMode::Absolute),

        // Returns
        Instruction::new(0x40, "RTI", 1, 6, AddressingMode::NoneAddressing),
        Instruction::new(0x60, "RTS", 1, 6, AddressingMode::NoneAddressing),

        // Flag interaction
        Instruction::new(0x18, "CLC", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0xD8, "CLD", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0x58, "CLI", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0xB8, "CLV", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0x38, "SEC", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0xF8, "SED", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0x78, "SEI", 1, 2, AddressingMode::NoneAddressing),

        // Compares
        Instruction::new(0xC9, "CMP", 2, 2, AddressingMode::Immediate),
        Instruction::new(0xC5, "CMP", 2, 3, AddressingMode::ZeroPage),
        Instruction::new(0xD5, "CMP", 2, 4, AddressingMode::ZeroPageX),
        Instruction::new(0xCD, "CMP", 3, 4, AddressingMode::Absolute),
        Instruction::new(0xDD, "CMP", 3, 4, AddressingMode::AbsoluteX), // +1 if page crossed
        Instruction::new(0xD9, "CMP", 3, 4, AddressingMode::AbsoluteY), // +1 if page crossed
        Instruction::new(0xC1, "CMP", 2, 6, AddressingMode::IndirectX),
        Instruction::new(0xD1, "CMP", 2, 5, AddressingMode::IndirectY), // +1 if page crossed
        Instruction::new(0xE0, "CPX", 2, 2, AddressingMode::Immediate),
        Instruction::new(0xE4, "CPX", 2, 3, AddressingMode::ZeroPage),
        Instruction::new(0xEC, "CPX", 3, 4, AddressingMode::Absolute),
        Instruction::new(0xC0, "CPY", 2, 2, AddressingMode::Immediate),
        Instruction::new(0xC4, "CPY", 2, 3, AddressingMode::ZeroPage),
        Instruction::new(0xCC, "CPY", 3, 4, AddressingMode::Absolute),
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
                irq_disable: false,
                decimal: false,
                break_cmd: false,
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
            irq_disable: false,
            decimal: false,
            break_cmd: false,
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

    /// Pushes a 8-bit value onto the stack, decrementing stack pointer
    fn push_stack(&mut self, data: u8) {
        self.write_mem(STACK_PAGE | self.stack_pointer as u16, data);
        self.stack_pointer = self.stack_pointer.wrapping_sub(1);
    }

    /// Pushes a 16-bit value onto the stack, decrementing stack pointer
    fn push_stack_u16(&mut self, data: u16) {
        self.stack_pointer = self.stack_pointer.wrapping_sub(1);
        self.write_mem_u16(STACK_PAGE | self.stack_pointer as u16, data);
        self.stack_pointer = self.stack_pointer.wrapping_sub(1);
    }

    /// Pulls a 8-bit value from the stack, incrementing stack pointer
    fn pull_stack(&mut self) -> u8 {
        self.stack_pointer = self.stack_pointer.wrapping_add(1);
        self.read_mem(STACK_PAGE | self.stack_pointer as u16)
    }

    /// Pulls a 16-bit value from the stack, incrementing stack pointer
    fn pull_stack_u16(&mut self) -> u16 {
        self.stack_pointer = self.stack_pointer.wrapping_add(1);
        let lsb = self.read_mem(STACK_PAGE | self.stack_pointer as u16) as u16;
        self.stack_pointer = self.stack_pointer.wrapping_add(1);
        lsb | (self.read_mem(STACK_PAGE | self.stack_pointer as u16) as u16) << 8
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
            (orig_a ^ self.register_a) & (operand ^ self.register_a) & SIGN_MASK != 0;

        // Carry if new value is smaller, or value from operand was 0xFF and carry was set
        self.status.carry = self.register_a < orig_a || self.register_a == orig_a && carry > 0;

        self.update_zero_neg(self.register_a);
    }

    fn and(&mut self, mode: &AddressingMode) {
        self.register_a &= self.read_mem(self.get_operand_addr(mode));
        self.update_zero_neg(self.register_a);
    }

    fn asl(&mut self, mode: &AddressingMode) {
        // NoneAddressing works directly on accumulator
        // MSB shifts to carry bit
        match mode {
            &AddressingMode::NoneAddressing => {
                self.status.carry = self.register_a & SIGN_MASK != 0;
                self.register_a <<= 1;
                self.update_zero_neg(self.register_a);
            }
            _ => {
                let addr = self.get_operand_addr(mode);
                let mut operand = self.read_mem(addr);
                self.status.carry = operand & SIGN_MASK != 0;
                operand <<= 1;
                self.write_mem(addr, operand);
                self.update_zero_neg(operand);
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

    fn compare(&mut self, source: u8, mode: &AddressingMode) {
        let operand = self.read_mem(self.get_operand_addr(mode));
        self.status.carry = source >= operand;
        self.status.zero = source == operand;
        self.status.negative = source.wrapping_sub(operand) & SIGN_MASK != 0;
    }

    fn dec(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_addr(mode);
        let new_val = self.read_mem(addr).wrapping_sub(1);
        self.write_mem(addr, new_val);
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
        self.register_a ^= self.read_mem(self.get_operand_addr(mode));
        self.update_zero_neg(self.register_a);
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

    fn iny(&mut self) {
        self.register_y = self.register_x.wrapping_add(1);
        self.update_zero_neg(self.register_y);
    }

    fn jmp(&mut self, mode: &AddressingMode) {
        self.program_counter = match mode {
            AddressingMode::Absolute => self.read_mem_u16(self.program_counter),
            AddressingMode::NoneAddressing => {
                self.read_mem_u16(self.read_mem_u16(self.program_counter))
            }
            _ => panic!("Unsupported addressing mode for JMP!"),
        };
    }

    fn jsr(&mut self) {
        self.push_stack_u16(self.program_counter + 1);
        self.program_counter = self.read_mem_u16(self.program_counter);
    }

    fn lda(&mut self, mode: &AddressingMode) {
        self.register_a = self.read_mem(self.get_operand_addr(mode));
        self.update_zero_neg(self.register_a);
    }

    fn ldx(&mut self, mode: &AddressingMode) {
        self.register_x = self.read_mem(self.get_operand_addr(mode));
        self.update_zero_neg(self.register_x);
    }

    fn ldy(&mut self, mode: &AddressingMode) {
        self.register_y = self.read_mem(self.get_operand_addr(mode));
        self.update_zero_neg(self.register_y);
    }

    fn lsr(&mut self, mode: &AddressingMode) {
        // NoneAddressing works directly on accumulator
        // LSB shifts to carry bit
        match mode {
            &AddressingMode::NoneAddressing => {
                self.status.carry = self.register_a & 0x1 != 0;
                self.register_a >>= 1;
                self.update_zero_neg(self.register_a);
            }
            _ => {
                let addr = self.get_operand_addr(mode);
                let mut operand = self.read_mem(addr);
                self.status.carry = operand & 0x1 != 0;
                operand >>= 1;
                self.write_mem(addr, operand);
                self.update_zero_neg(operand);
            }
        }
    }

    fn ora(&mut self, mode: &AddressingMode) {
        self.register_a |= self.read_mem(self.get_operand_addr(mode));
        self.update_zero_neg(self.register_a);
    }

    fn rol(&mut self, mode: &AddressingMode) {
        // NoneAddressing works directly on accumulator
        // Carry bit shifts to LSB, MSB shifts to carry bit
        match mode {
            &AddressingMode::NoneAddressing => {
                let carry_in = if self.status.carry { 0x01 } else { 0x00 };
                self.status.carry = self.register_a & SIGN_MASK != 0;
                self.register_a <<= 1;
                self.register_a |= carry_in;
                self.update_zero_neg(self.register_a);
            }
            _ => {
                let addr = self.get_operand_addr(mode);
                let mut operand = self.read_mem(addr);
                let carry_in = if self.status.carry { 0x01 } else { 0x00 };
                self.status.carry = operand & SIGN_MASK != 0;
                operand <<= 1;
                operand |= carry_in;
                self.write_mem(addr, operand);
                self.update_zero_neg(operand);
            }
        }
    }

    fn ror(&mut self, mode: &AddressingMode) {
        // NoneAddressing works directly on accumulator
        // Carry bit shifts to MSB, LSB shifts to carry bit
        match mode {
            &AddressingMode::NoneAddressing => {
                let carry_in = if self.status.carry { 0x80 } else { 0x00 };
                self.status.carry = self.register_a & 0x80 != 0;
                self.register_a >>= 1;
                self.register_a |= carry_in;
                self.update_zero_neg(self.register_a);
            }
            _ => {
                let addr = self.get_operand_addr(mode);
                let mut operand = self.read_mem(addr);
                let carry_in = if self.status.carry { 0x80 } else { 0x00 };
                self.status.carry = operand & 0x80 != 0;
                operand >>= 1;
                operand |= carry_in;
                self.write_mem(addr, operand);
                self.update_zero_neg(operand);
            }
        }
    }

    fn rti(&mut self) {
        self.status = self.pull_stack().into();
        self.program_counter = self.pull_stack_u16().wrapping_add(1);
    }

    fn rts(&mut self) {
        self.program_counter = self.pull_stack_u16().wrapping_add(1);
    }

    fn sbc(&mut self, mode: &AddressingMode) {
        let operand = self.read_mem(self.get_operand_addr(mode));
        let carry = if self.status.carry { 0 } else { 0xFF };

        let operand_neg = (!operand).wrapping_add(1);

        let orig_a = self.register_a;
        self.register_a = orig_a.wrapping_add(operand_neg).wrapping_add(carry);

        // Overflow if both inputs are different sign than result
        self.status.overflow =
            (orig_a ^ self.register_a) & (operand_neg ^ self.register_a) & SIGN_MASK != 0;

        // Carry if new value is smaller, or current value is same as original and carry was set
        self.status.carry = self.register_a < orig_a || self.register_a == orig_a && carry > 0;

        self.update_zero_neg(self.register_a);
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

    pub fn run(&mut self) {
        loop {
            println!("Executing at address 0x{:x}", self.program_counter);
            let op = self.read_mem(self.program_counter);
            self.program_counter += 1;

            let instruction = INSTRUCTIONS.iter().find(|x| x.opcode == op).unwrap();
            println!("{:?}", instruction);

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
                "CLD" => self.status.decimal = false,
                "CLI" => self.status.irq_disable = false,
                "CLV" => self.status.overflow = false,
                "CMP" => self.compare(self.register_a, &instruction.addressing_mode),
                "CPX" => self.compare(self.register_x, &instruction.addressing_mode),
                "CPY" => self.compare(self.register_y, &instruction.addressing_mode),
                "DEC" => self.dec(&instruction.addressing_mode),
                "DEX" => self.dex(),
                "DEY" => self.dey(),
                "EOR" => self.eor(&instruction.addressing_mode),
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
                "PHP" => self.push_stack(self.status.into()),
                "PLA" => self.register_a = self.pull_stack(),
                "PLP" => self.status = self.pull_stack().into(),
                "ROL" => self.rol(&instruction.addressing_mode),
                "ROR" => self.ror(&instruction.addressing_mode),
                "RTI" => self.rti(),
                "RTS" => self.rts(),
                "SBC" => self.sbc(&instruction.addressing_mode),
                "SEC" => self.status.carry = true,
                "SED" => self.status.decimal = true,
                "SEI" => self.status.irq_disable = true,
                "STA" => self.write_mem(
                    self.get_operand_addr(&instruction.addressing_mode),
                    self.register_a,
                ),
                "STX" => self.write_mem(
                    self.get_operand_addr(&instruction.addressing_mode),
                    self.register_x,
                ),
                "STY" => self.write_mem(
                    self.get_operand_addr(&instruction.addressing_mode),
                    self.register_y,
                ),
                "TAX" => self.tax(),
                "TAY" => self.tay(),
                "TSX" => self.tsx(),
                "TXA" => self.txa(),
                "TXS" => self.txs(),
                "TYA" => self.tya(),
                // not yet implemented
                _ => todo!(),
            }

            // Don't increment program counter for some instructions
            match instruction.mnemonic {
                "JMP" | "JSR" => (),
                _ => self.program_counter += (instruction.bytes - 1) as u16,
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
        cpu.setup(vec![0xa9, 0x7F]);
        cpu.run();
        assert_eq!(cpu.register_a, 0x7F);
        assert!(!cpu.status.zero);
        assert!(!cpu.status.negative);
    }

    #[test]
    fn test_lda_immediate_zero() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xa9, 0x00]);
        cpu.run();
        assert_eq!(cpu.register_a, 0x00);
        assert!(cpu.status.zero);
        assert!(!cpu.status.negative);
    }

    #[test]
    fn test_lda_immediate_neg() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xa9, 0xFF]);
        cpu.run();
        assert_eq!(cpu.register_a, 0xFF);
        assert!(!cpu.status.zero);
        assert!(cpu.status.negative);
    }

    #[test]
    fn test_tax_a_to_x() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xAA]);
        cpu.register_a = 0xFF;
        cpu.run();
        assert_eq!(cpu.register_x, 0xFF);
        assert!(!cpu.status.zero);
        assert!(cpu.status.negative);
    }

    #[test]
    fn test_inx_x_to_nonzero() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xE8]);
        cpu.register_x = 0x56;
        cpu.run();
        assert_eq!(cpu.register_x, 0x57);
        assert!(!cpu.status.zero);
        assert!(!cpu.status.negative);
    }

    #[test]
    fn test_inx_x_to_negative() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xE8]);
        cpu.register_x = 0x7F;
        cpu.run();
        assert_eq!(cpu.register_x, 0x80);
        assert!(!cpu.status.zero);
        assert!(cpu.status.negative);
    }

    #[test]
    fn test_inx_x_to_zero() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xE8]);
        cpu.register_x = 0xFF;
        cpu.run();
        assert_eq!(cpu.register_x, 0x0);
        assert!(cpu.status.zero);
        assert!(!cpu.status.negative);
    }

    #[test]
    fn test_iny() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xc8]);
        cpu.register_y = 0x50;
        cpu.run();
        assert_eq!(cpu.register_y, 0x1);
        assert!(!cpu.status.zero);
        assert!(!cpu.status.negative);
    }

    #[test]
    fn test_lda_zeropage() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xa5, 0x4]);
        cpu.memory[0x4] = 0x56;
        cpu.run();
        assert_eq!(cpu.register_a, 0x56);
    }

    #[test]
    fn test_lda_zeropagex() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xb5, 0x4]);
        cpu.register_x = 5;
        cpu.register_y = 6;
        cpu.memory[0x9] = 0x56;
        cpu.run();
        assert_eq!(cpu.register_a, 0x56);
    }

    #[test]
    fn test_lda_absolute() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xbd, 0x4, 0x5]);
        cpu.register_x = 5;
        cpu.register_y = 6;
        cpu.memory[0x0504 + 5] = 0x56;
        cpu.run();
        assert_eq!(cpu.register_a, 0x56);
    }

    #[test]
    fn test_lda_absolutex() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xbd, 0x4, 0x5]);
        cpu.register_x = 5;
        cpu.register_y = 6;
        cpu.memory[0x0504 + 5] = 0x56;
        cpu.run();
        assert_eq!(cpu.register_a, 0x56);
    }

    #[test]
    fn test_lda_absolutey() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xb9, 0x4, 0x5]);
        cpu.register_x = 5;
        cpu.register_y = 6;
        cpu.memory[0x0504 + 6] = 0x56;
        cpu.run();
        assert_eq!(cpu.register_a, 0x56);
    }

    #[test]
    fn test_lda_indirectx() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xa1, 0x4]);
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
        cpu.setup(vec![0xb1, 0x4]);
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
        cpu.setup(vec![0xf6, 0xFF]);
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
        cpu.setup(vec![0x69, 0xa0]);
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
        cpu.setup(vec![0x69, 0x50]);
        cpu.register_a = 0x30;
        cpu.register_x = 5;
        cpu.register_y = 8;
        cpu.status.carry = true;
        cpu.run();
        assert_eq!(cpu.register_a, 0x81); // 80 + 48 + 1 = negative number
        assert!(!cpu.status.zero);
        assert!(cpu.status.negative);
        assert!(!cpu.status.carry);
        assert!(cpu.status.overflow);
    }

    #[test]
    fn test_and_immediate() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x29, 0xaa]);
        cpu.register_a = 0xf0;
        cpu.run();
        assert_eq!(cpu.register_a, 0xa0);
        assert!(!cpu.status.zero);
        assert!(cpu.status.negative);
    }

    #[test]
    fn test_asl_acc() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x0a]);
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
        cpu.setup(vec![0x0e, 0xaa, 0x55]);
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
        cpu.setup(vec![0x90, 0x15]);
        cpu.run();
        // Address of next instruction (2) + jump offset (0x15) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x8018);
    }

    #[test]
    fn test_bcs_bcc_carry_clear_negative_jump() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xB0, 0x15, 0x90, 0xFA]);
        cpu.run();
        // Address of next instruction (2) - jump offset (0x6) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x7FFF);
    }

    #[test]
    fn test_bcc_bcs_carry_set() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x90, 0x15, 0xB0, 0x15]);
        cpu.status.carry = true;
        cpu.run();
        // Address of next instruction (4) + jump offset (0x15) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x801a);
    }

    #[test]
    fn test_beq_bne_zero_clear() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xF0, 0x15, 0xD0, 0xFA]);
        cpu.run();
        // Address of next instruction (2) - jump offset (0x6) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x7FFF);
    }

    #[test]
    fn test_bne_beq_zero_set() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xD0, 0x15, 0xF0, 0x15]);
        cpu.status.zero = true;
        cpu.run();
        // Address of next instruction (4) + jump offset (0x15) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x801a);
    }

    #[test]
    fn test_bit_nonzero() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x24, 0x00]);
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
        cpu.setup(vec![0x24, 0x00]);
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
        cpu.setup(vec![0x30, 0x15, 0x10, 0xFA]);
        cpu.run();
        // Address of next instruction (2) - jump offset (0x6) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x7FFF);
    }

    #[test]
    fn test_bpl_bmi_negative_set() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x10, 0x15, 0x30, 0x15]);
        cpu.status.negative = true;
        cpu.run();
        // Address of next instruction (4) + jump offset (0x15) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x801a);
    }

    #[test]
    fn test_bvs_bvc_overflow_clear() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x70, 0x15, 0x50, 0xFA]);
        cpu.run();
        // Address of next instruction (2) - jump offset (0x6) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x7FFF);
    }

    #[test]
    fn test_bvc_bvs_overflow_set() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x50, 0x15, 0x70, 0x15]);
        cpu.status.overflow = true;
        cpu.run();
        // Address of next instruction (4) + jump offset (0x15) + 1 (BRK instruction)
        assert_eq!(cpu.program_counter, 0x801a);
    }

    #[test]
    fn test_clc() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x18]);
        cpu.status.carry = true;
        cpu.run();
        assert!(!cpu.status.carry);
    }

    #[test]
    fn test_cld() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xd8]);
        cpu.status.decimal = true;
        cpu.run();
        assert!(!cpu.status.decimal);
    }

    #[test]
    fn test_cli() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x58]);
        cpu.status.irq_disable = true;
        cpu.run();
        assert!(!cpu.status.irq_disable);
    }

    #[test]
    fn test_clv() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xb8]);
        cpu.status.overflow = true;
        cpu.run();
        assert!(!cpu.status.overflow);
    }

    #[test]
    fn test_cmp_immediate_a_greater() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xC9, 0x10]);
        cpu.register_a = 0x20;
        cpu.run();
        assert!(cpu.status.carry);
        assert!(!cpu.status.zero);
        assert!(!cpu.status.negative);
    }

    #[test]
    fn test_cmp_immediate_equal() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xC9, 0xc0]);
        cpu.register_a = 0xc0;
        cpu.run();
        assert!(cpu.status.carry);
        assert!(cpu.status.zero);
        assert!(!cpu.status.negative);
    }

    #[test]
    fn test_cmp_immediate_a_less() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xC9, 0x20]);
        cpu.register_a = 0x10;
        cpu.run();
        assert!(!cpu.status.carry);
        assert!(!cpu.status.zero);
        assert!(cpu.status.negative);
    }

    #[test]
    fn test_cpx_immediate_x_greater() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xe0, 0x10]);
        cpu.register_x = 0x20;
        cpu.run();
        assert!(cpu.status.carry);
        assert!(!cpu.status.zero);
        assert!(!cpu.status.negative);
    }

    #[test]
    fn test_cpy_immediate_y_less() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xC0, 0x20]);
        cpu.register_y = 0x10;
        cpu.run();
        assert!(!cpu.status.carry);
        assert!(!cpu.status.zero);
        assert!(cpu.status.negative);
    }

    #[test]
    fn test_dec_zeropage() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xc6, 0x50]);
        cpu.register_x = 5;
        cpu.register_y = 8;
        cpu.memory[0x50] = 0x01;
        cpu.run();
        assert_eq!(cpu.memory[0x50], 0x0);
        assert!(cpu.status.zero);
        assert!(!cpu.status.negative);
    }

    #[test]
    fn test_dex_zeropage() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xca]);
        cpu.register_x = 0x80;
        cpu.run();
        assert_eq!(cpu.register_x, 0x7F);
        assert!(!cpu.status.zero);
        assert!(!cpu.status.negative);
    }

    #[test]
    fn test_dey_zeropage() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x88]);
        cpu.register_y = 0x81;
        cpu.run();
        assert_eq!(cpu.register_y, 0x80);
        assert!(!cpu.status.zero);
        assert!(cpu.status.negative);
    }

    #[test]
    fn test_eor_immediate_zero() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x49, 0xaa]);
        cpu.register_a = 0xaa;
        cpu.run();
        assert_eq!(cpu.register_a, 0x00);
        assert!(cpu.status.zero);
        assert!(!cpu.status.negative);
    }

    #[test]
    fn test_eor_immediate_nonzero() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x49, 0xaa]);
        cpu.register_a = 0xa5;
        cpu.run();
        assert_eq!(cpu.register_a, 0x0F);
        assert!(!cpu.status.zero);
        assert!(!cpu.status.negative);
    }

    #[test]
    fn test_jmp_absolute() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x4c, 0x20, 0x43]);
        cpu.run();
        assert_eq!(cpu.program_counter, 0x4321);
    }

    #[test]
    fn test_jmp_indirect() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x6c, 0x21, 0x43]);
        cpu.memory[0x4321] = 0x33;
        cpu.memory[0x4322] = 0x12;
        cpu.run();
        assert_eq!(cpu.program_counter, 0x1234);
    }

    #[test]
    fn test_jsr() {
        let mut cpu = Cpu::new();
        // First jump to some address
        cpu.setup(vec![0x4c, 0x20, 0x43]);
        // From there jump to subroutine
        cpu.stack_pointer = 0x8;
        cpu.memory[0x4320] = 0x20;
        cpu.memory[0x4321] = 0x04; // Jump target is BRK
        cpu.memory[0x4322] = 0x00;
        cpu.run();
        assert_eq!(cpu.program_counter, 0x0005); // one cycle added from BRK
        assert_eq!(cpu.stack_pointer, 0x6);
        assert_eq!(cpu.memory[0x108], 0x43);
        assert_eq!(cpu.memory[0x107], 0x22);
    }

    #[test]
    fn test_ldx_zeropagey() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xb6, 0x70]);
        cpu.register_y = 0xf;
        cpu.memory[0x7f] = 0x90;
        cpu.run();
        assert_eq!(cpu.register_x, 0x90);
        assert!(!cpu.status.zero);
        assert!(cpu.status.negative);
    }

    #[test]
    fn test_ldy_immediate() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xa0, 0x00]);
        cpu.register_y = 0xff;
        cpu.run();
        assert_eq!(cpu.register_y, 0x00);
        assert!(cpu.status.zero);
        assert!(!cpu.status.negative);
    }

    #[test]
    fn test_lsr_to_zero() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x4a]);
        cpu.register_a = 0x01;
        cpu.run();
        assert_eq!(cpu.register_a, 0x00);
        assert!(cpu.status.carry);
        assert!(cpu.status.zero);
        assert!(!cpu.status.negative);
    }

    #[test]
    fn test_nop() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xea, 0xea, 0xea]);
        cpu.run();
        assert_eq!(cpu.program_counter, 0x8004);
    }

    #[test]
    fn test_ora_immediate() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x09, 0xa2]);
        cpu.register_a = 0x55;
        cpu.run();
        assert_eq!(cpu.register_a, 0xf7);
        assert!(!cpu.status.zero);
        assert!(cpu.status.negative);
    }

    #[test]
    fn test_pha() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x48]);
        cpu.stack_pointer = 0xfc;
        cpu.register_a = 0x55;
        cpu.run();
        assert_eq!(cpu.memory[0x01fc], 0x55);
        assert_eq!(cpu.stack_pointer, 0xfb);
    }

    #[test]
    fn test_php() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x08]);
        cpu.stack_pointer = 0x00;
        cpu.status.carry = true;
        cpu.status.negative = true;
        cpu.run();
        assert_eq!(cpu.memory[0x0100], 0x81);
        assert_eq!(cpu.stack_pointer, 0xff);
    }

    #[test]
    fn test_pla() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x68]);
        cpu.stack_pointer = 0xfc;
        cpu.memory[0x01fd] = 0x55;
        cpu.run();
        assert_eq!(cpu.register_a, 0x55);
        assert_eq!(cpu.stack_pointer, 0xfd);
    }

    #[test]
    fn test_plp() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x28]);
        cpu.stack_pointer = 0xff;
        cpu.memory[0x0100] = 0x81;
        cpu.run();
        assert!(cpu.status.carry);
        assert!(cpu.status.negative);
        assert_eq!(cpu.stack_pointer, 0x00);
    }

    #[test]
    fn test_rol_a_carry_in() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x2a]);
        cpu.register_a = 0x42;
        cpu.status.carry = true;
        cpu.run();
        assert!(!cpu.status.carry);
        assert!(cpu.status.negative);
        assert_eq!(cpu.register_a, 0x85);
    }

    #[test]
    fn test_rol_zeropage_carry_out() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x26, 0x01]);
        cpu.memory[0x0001] = 0x87;
        cpu.run();
        assert!(cpu.status.carry);
        assert!(!cpu.status.negative);
        assert_eq!(cpu.memory[0x0001], 0x0E);
    }

    #[test]
    fn test_ror_a_carry_in() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x6a]);
        cpu.register_a = 0x42;
        cpu.status.carry = true;
        cpu.run();
        assert!(!cpu.status.carry);
        assert!(cpu.status.negative);
        assert_eq!(cpu.register_a, 0xa1);
    }

    #[test]
    fn test_ror_zeropage_carry_out() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x66, 0x01]);
        cpu.memory[0x0001] = 0x87;
        cpu.run();
        assert!(cpu.status.carry);
        assert!(!cpu.status.negative);
        assert_eq!(cpu.memory[0x0001], 0x43);
    }

    #[test]
    fn test_rti() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x40]);
        cpu.stack_pointer = 0x5;
        cpu.memory[0x0106] = 0x81;
        cpu.memory[0x0107] = 0x20;
        cpu.memory[0x0108] = 0x43;
        cpu.run();
        assert!(cpu.status.carry);
        assert!(cpu.status.negative);
        assert_eq!(cpu.stack_pointer, 0x08);
        assert_eq!(cpu.program_counter, 0x4322);
    }

    #[test]
    fn test_rts() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x60]);
        cpu.stack_pointer = 0xfe;
        cpu.memory[0x01ff] = 0x20;
        cpu.memory[0x0100] = 0x43;
        cpu.run();
        assert_eq!(cpu.stack_pointer, 0x00);
        assert_eq!(cpu.program_counter, 0x4322);
    }

    #[test]
    fn test_sbc_keep_carry() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xe9, 0x10]);
        cpu.register_a = 0x31;
        cpu.status.carry = true;
        cpu.run();
        assert_eq!(cpu.register_a, 0x21);
        assert!(cpu.status.carry); // no overflow so carry should stay
        assert!(!cpu.status.negative);
        assert!(!cpu.status.overflow);
        assert!(!cpu.status.zero);
    }

    #[test]
    fn test_sbc_no_carry_to_zero() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xe9, 0x30]);
        cpu.register_a = 0x31;
        cpu.run();
        assert_eq!(cpu.register_a, 0x00);
        assert!(cpu.status.carry);
        assert!(!cpu.status.negative);
        assert!(!cpu.status.overflow);
        assert!(cpu.status.zero);
    }

    #[test]
    fn test_sbc_consume_carry() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xe9, 0x40]);
        cpu.register_a = 0x30;
        cpu.status.carry = true;
        cpu.run();
        assert_eq!(cpu.register_a, 0xf0);
        assert!(!cpu.status.carry); // carry should be consumed
        assert!(cpu.status.negative);
        assert!(!cpu.status.overflow);
        assert!(!cpu.status.zero);
    }

    #[test]
    fn test_sbc_overflow() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xe9, 0x10]);
        cpu.register_a = 0x88; // -120
        cpu.status.carry = true;
        cpu.run();
        assert_eq!(cpu.register_a, 0x78); // -120-16 turns into +120
        assert!(cpu.status.carry); // does not consume carry
        assert!(!cpu.status.negative);
        assert!(cpu.status.overflow);
        assert!(!cpu.status.zero);
    }

    #[test]
    fn test_sec() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x38]);
        cpu.run();
        assert!(cpu.status.carry);
    }

    #[test]
    fn test_sed() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xf8]);
        cpu.status.decimal = true;
        cpu.run();
        assert!(cpu.status.decimal);
    }

    #[test]
    fn test_sei() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x78]);
        cpu.run();
        assert!(cpu.status.irq_disable);
    }

    #[test]
    fn test_sta() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x85, 0x01]);
        cpu.register_a = 0x78;
        cpu.run();
        assert_eq!(cpu.memory[0x01], 0x78);
    }

    #[test]
    fn test_stx() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x86, 0x01]);
        cpu.register_x = 0x78;
        cpu.run();
        assert_eq!(cpu.memory[0x01], 0x78);
    }

    #[test]
    fn test_sty() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x84, 0x01]);
        cpu.register_y = 0x78;
        cpu.run();
        assert_eq!(cpu.memory[0x01], 0x78);
    }

    #[test]
    fn test_tay() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xa8]);
        cpu.register_a = 0;
        cpu.register_y = 0x78;
        cpu.run();
        assert_eq!(cpu.register_y, 0x00);
        assert!(!cpu.status.negative);
        assert!(cpu.status.zero);
    }

    #[test]
    fn test_tsx() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xba]);
        cpu.stack_pointer = 0xa5;
        cpu.run();
        assert_eq!(cpu.register_x, 0xa5);
        assert!(cpu.status.negative);
        assert!(!cpu.status.zero);
    }

    #[test]
    fn test_txa() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x8a]);
        cpu.register_x = 0xa5;
        cpu.run();
        assert_eq!(cpu.register_a, 0xa5);
        assert!(cpu.status.negative);
        assert!(!cpu.status.zero);
    }

    #[test]
    fn test_txs() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x9a]);
        cpu.register_x = 0x55;
        cpu.status.zero = true;
        cpu.status.negative = true;
        cpu.run();
        assert_eq!(cpu.stack_pointer, 0x55);
        assert!(cpu.status.negative); // does not affect flags
        assert!(cpu.status.zero);
    }

    #[test]
    fn test_tya() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0x98]);
        cpu.register_y = 0xa5;
        cpu.run();
        assert_eq!(cpu.register_a, 0xa5);
        assert!(cpu.status.negative);
        assert!(!cpu.status.zero);
    }
}
