#[derive(Clone, Copy)]
pub struct CpuSnapshot {
    pub gprs: [u32; 32],
    pub lr: u32,
    pub ctr: u32,
}

impl CpuSnapshot {
    pub fn from_cpu(cpu: &gekko::cpu::Cpu) -> Self {
        Self {
            gprs: cpu.gprs,
            lr: cpu.lr,
            ctr: cpu.ctr,
        }
    }
}