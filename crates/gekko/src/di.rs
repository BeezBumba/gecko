pub mod regs;

use crate::mmio::constants::DI_BASE;
use crate::mmio::traits::{MmioAccess, MmioRegister, MmioRw};

pub struct DvdInterface {
    pub status: regs::DiStatusRegister,
    pub cover: regs::DiCoverRegister,
}

impl DvdInterface {
    pub fn new() -> Self {
        Self {
            status: regs::DiStatusRegister::from_raw(0),
            cover: regs::DiCoverRegister::from_raw(0),
        }
    }

    pub fn interrupt_active(&self) -> bool {
        (self.status.break_complete() && self.status.break_complete_mask())
            || (self.status.device_error() && self.status.device_error_mask())
            || (self.status.transfer_complete() && self.status.transfer_complete_mask())
            || (self.cover.cover_interrupt() && self.cover.cover_interrupt_mask())
    }

}

impl MmioRw for DvdInterface {
    const BASE: u32 = DI_BASE;
    const NAME: &'static str = "DI";

    crate::impl_mmio_dispatch!(regs::DiStatusRegister, regs::DiCoverRegister,);
}

impl crate::gekko::Gekko {
    pub fn check_di_interrupts(&mut self) {
        if self.di.interrupt_active() {
            self.pi.assert_interrupt(crate::flipper::pi::InterruptFlag::Di);
        } else {
            self.pi.clear_interrupt(crate::flipper::pi::InterruptFlag::Di);
        }
    }

    pub fn open_cover(&mut self) {
        tracing::debug!("DVD drive cover opened");
        self.di.cover = self.di.cover.with_cover_interrupt(true).with_cover_status(true);
        self.check_di_interrupts();
    }

    pub fn close_cover(&mut self) {
        tracing::debug!("DVD drive cover closed");
        self.di.cover = self.di.cover.with_cover_status(false);
        self.check_di_interrupts();
    }
}
