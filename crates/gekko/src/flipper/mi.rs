pub mod regs;

use crate::mmio::constants::MI_BASE;
use crate::mmio::traits::{MmioAccess, MmioRegister, MmioRw};

pub struct MemoryInterface {
    pub interrupt_mask: regs::MiInterruptMask,
}

impl MemoryInterface {
    pub fn new() -> Self {
        Self {
            interrupt_mask: regs::MiInterruptMask::from_raw(0),
        }
    }

}

impl MmioRw for MemoryInterface {
    const BASE: u32 = MI_BASE;
    const NAME: &'static str = "MI";

    crate::impl_mmio_dispatch!(regs::MiInterruptMask,);
}
