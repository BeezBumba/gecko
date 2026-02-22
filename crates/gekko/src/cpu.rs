pub mod interpreter;
pub mod semantics;
pub mod branch;

#[allow(dead_code, unused_variables, non_upper_case_globals, clippy::all)]
pub mod lut {
    include!(concat!(env!("OUT_DIR"), "/gekko_lut.rs"));
}

pub struct Cpu {
    pub gprs: [u32; 32],
    pub fprs: [f64; 32],
    pub pc: u32,
    pub lr: u32,
    pub ctr: u32,
    pub xer: u32,

    // These are used during instruction execution to track the current
    // and next PC values. In essence, by writing to `next_pc`, instructions
    // can change the flow of execution (e.g. for branches and jumps).
    pub cia: u32, // Current Instruction Address
    pub nia: u32, // Next Instruction Address
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            gprs: [0; 32],
            fprs: [0.0; 32],
            pc: 0x80000100,
            cia: 0x80000100,
            nia: 0x80000104,
            lr: 0,
            ctr: 0,
            xer: 0,
        }
    }
}
