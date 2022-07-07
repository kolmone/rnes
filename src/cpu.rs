#[derive(Debug)]
pub struct Cpu {
    register_a: u8,
    register_x: u8,
    register_y: u8,
    program_counter: u16,
    stack_pointer: u8,
    status: u8,
    memory: [u8; 0x10000],
}

const CARRY_FLAG: u8 = 0x1 << 0;
const ZERO_FLAG: u8 = 0x1 << 1;
const IRQ_FLAG: u8 = 0x1 << 2;
const DECIMAL_FLAG: u8 = 0x1 << 3;
const BREAK_CMD: u8 = 0x3 << 4;
const OVERFLOW_FLAG: u8 = 0x1 << 6;
const NEGATIVE_FLAG: u8 = 0x1 << 7;

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
    instruction: &'static str,
    addressing_mode: AddressingMode,
    bytes: u8,
    duration: u8,
}

impl Instruction {
    pub fn new(opcode: u8, instruction: &'static str, bytes: u8, duration: u8, addressing_mode: AddressingMode) -> Self {
        Instruction{opcode, instruction, bytes, duration, addressing_mode}
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
        Instruction::new(0xE8, "INX", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0xC8, "INY", 1, 2, AddressingMode::NoneAddressing),
        
        // Transfers
        Instruction::new(0xAA, "TAX", 1, 2, AddressingMode::NoneAddressing),
        Instruction::new(0xA8, "TAY", 1, 2, AddressingMode::NoneAddressing),
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
            status: 0,
            memory: [0; 0x10000],
        }
    }

    fn reset(&mut self) {
        self.register_a = 0;
        self.register_x = 0;
        self.register_y = 0;
        self.status = 0;

        self.program_counter = self.read_mem_u16(RESET_ADDR);
    }

    fn update_zero_neg(&mut self, val: u8) {
        if val == 0 {
            self.status |= ZERO_FLAG;
        } else {
            self.status &= !ZERO_FLAG;
        }

        if val >= 128 {
            self.status |= NEGATIVE_FLAG;
        } else {
            self.status &= !NEGATIVE_FLAG;
        }
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

    fn lda(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_addr(mode);
        self.register_a = self.read_mem(addr);
        self.update_zero_neg(self.register_a);
    }

    fn tax(&mut self) {
        self.register_x = self.register_a;
        self.update_zero_neg(self.register_x);
    }

    fn inx(&mut self) {
        self.register_x = self.register_x.wrapping_add(1);
        self.update_zero_neg(self.register_x);
    }

    pub fn run(&mut self) {
        loop {
            let op = self.read_mem(self.program_counter);
            self.program_counter += 1;

            let instruction: &Instruction = INSTRUCTIONS.iter().find(|&x| x.opcode == op).unwrap();
            println!("{:?}", instruction);

            match instruction.instruction {
                "BRK" => return,

                "LDA" => self.lda(&instruction.addressing_mode),

                "TAX" => self.tax(),

                "INX" => self.inx(),

                // not yet implemented
                _ => todo!(),
            }
            self.program_counter += (instruction.bytes-1) as u16;
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
        assert!(cpu.status & ZERO_FLAG == 0);
        assert!(cpu.status & NEGATIVE_FLAG == 0);
    }

    #[test]
    fn test_lda_immediate_zero() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xa9, 0x00, 0x00]);
        cpu.run();
        assert_eq!(cpu.register_a, 0x00);
        assert!(cpu.status & ZERO_FLAG == ZERO_FLAG);
        assert!(cpu.status & NEGATIVE_FLAG == 0);
    }

    #[test]
    fn test_lda_immediate_neg() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xa9, 0xFF, 0x00]);
        cpu.run();
        assert_eq!(cpu.register_a, 0xFF);
        assert!(cpu.status & ZERO_FLAG == 0);
        assert!(cpu.status & NEGATIVE_FLAG == NEGATIVE_FLAG);
    }

    #[test]
    fn test_tax_a_to_x() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xAA, 0x00]);
        cpu.register_a = 0xFF;
        cpu.run();
        assert_eq!(cpu.register_x, 0xFF);
        assert!(cpu.status & ZERO_FLAG == 0);
        assert!(cpu.status & NEGATIVE_FLAG == NEGATIVE_FLAG);
    }

    #[test]
    fn test_inx_x_to_nonzero() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xE8, 0x00]);
        cpu.register_x = 0x56;
        cpu.run();
        assert_eq!(cpu.register_x, 0x57);
        assert!(cpu.status & ZERO_FLAG == 0);
        assert!(cpu.status & NEGATIVE_FLAG == 0);
    }

    #[test]
    fn test_inx_x_to_negative() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xE8, 0x00]);
        cpu.register_x = 0x7F;
        cpu.run();
        assert_eq!(cpu.register_x, 0x80);
        assert!(cpu.status & ZERO_FLAG == 0);
        assert!(cpu.status & NEGATIVE_FLAG == NEGATIVE_FLAG);
    }

    #[test]
    fn test_inx_x_to_zero() {
        let mut cpu = Cpu::new();
        cpu.setup(vec![0xE8, 0x00]);
        cpu.register_x = 0xFF;
        cpu.run();
        assert_eq!(cpu.register_x, 0x0);
        assert!(cpu.status & ZERO_FLAG == ZERO_FLAG);
        assert!(cpu.status & NEGATIVE_FLAG == 0);
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
}
