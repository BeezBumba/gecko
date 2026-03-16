#[derive(PartialEq, Eq, Clone, Copy)]
pub enum EmulatorState {
    Running,
    Paused,
    StepOne,
    RunUntilVsync,
}

pub struct DebuggerUi {
    pub emulator_state: EmulatorState,
    pub show_cpu: bool,
    pub show_gx_state: bool,
    pub show_mmio: bool,
    pub memory_base: u32,
    pub memory_addr_input: String,
}

impl Default for DebuggerUi {
    fn default() -> Self {
        DebuggerUi {
            emulator_state: EmulatorState::Paused,
            show_cpu: true,
            show_gx_state: false,
            show_mmio: false,
            memory_base: 0x8000_0000,
            memory_addr_input: "80000000".to_string(),
        }
    }
}
