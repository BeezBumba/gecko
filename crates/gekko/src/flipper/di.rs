pub mod regs;

use crate::mmio::constants::DI_BASE;
use crate::mmio::traits::{MmioAccess, MmioRegister};

pub struct Di {
    pub status: regs::DiStatusRegister,
    pub cover: regs::DiCoverRegister,
}

impl Di {
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

    crate::impl_mmio_dispatch!(regs::DiStatusRegister, regs::DiCoverRegister,);

    pub fn mmio_read_u8(&mut self, offset: u32) -> u8 {
        self.read_raw(DI_BASE + offset, 1).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled DI read_u8");
            0
        }) as u8
    }

    pub fn mmio_write_u8(&mut self, offset: u32, val: u8) {
        if !self.write_raw(DI_BASE + offset, 1, val as u32) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled DI write_u8");
        }
    }

    pub fn mmio_read_u16(&mut self, offset: u32) -> u16 {
        self.read_raw(DI_BASE + offset, 2).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled DI read_u16");
            0
        }) as u16
    }

    pub fn mmio_write_u16(&mut self, offset: u32, val: u16) {
        if !self.write_raw(DI_BASE + offset, 2, val as u32) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled DI write_u16");
        }
    }

    pub fn mmio_read_u32(&mut self, offset: u32) -> u32 {
        self.read_raw(DI_BASE + offset, 4).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled DI read_u32");
            0
        })
    }

    pub fn mmio_write_u32(&mut self, offset: u32, val: u32) {
        if !self.write_raw(DI_BASE + offset, 4, val) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled DI write_u32");
        }
    }
}

impl crate::gekko::Gekko {
    pub fn check_di_interrupts(&mut self) {
        if self.di.interrupt_active() {
            self.pi.assert_interrupt(crate::flipper::pi::InterruptFlag::Di);
        } else {
            self.pi.clear_interrupt(crate::flipper::pi::InterruptFlag::Di);
        }
    }
}
