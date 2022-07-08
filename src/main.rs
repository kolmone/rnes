mod cpu;

fn main() {
    let mut cpu = cpu::Cpu::new();

    cpu.setup_and_run(vec![0x00]);
}
