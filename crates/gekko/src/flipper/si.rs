pub mod regs;

use crate::mmio::constants::SI_BASE;
use crate::mmio::traits::{MmioAccess, MmioRegister};

pub struct Si {
    pub poll: regs::SiPoll,
    pub comcsr: regs::SiComcsr,
    pub status: regs::SiStatusRegister,
    pub ch0_in_buf_hi: regs::SiChannel0InBufHi,
    pub io_buf: regs::SiIoBuf,
}

impl Si {
    pub fn new() -> Self {
        Self {
            poll: regs::SiPoll::from_raw(0),
            comcsr: regs::SiComcsr::from_raw(0),
            status: regs::SiStatusRegister::from_raw(0),
            ch0_in_buf_hi: regs::SiChannel0InBufHi::from_raw(0),
            io_buf: regs::SiIoBuf::from_raw(0),
        }
    }

    pub fn interrupt_active(&self) -> bool {
        (self.comcsr.tc_interrupt() && self.comcsr.tc_interrupt_mask())
            || (self.comcsr.rdst_interrupt() && self.comcsr.rdst_interrupt_mask())
    }

    crate::impl_mmio_dispatch!(
        regs::SiPoll,
        regs::SiComcsr,
        regs::SiStatusRegister,
        regs::SiChannel0InBufHi,
        regs::SiIoBuf,
    );

    pub fn mmio_read_u8(&mut self, offset: u32) -> u8 {
        self.read_raw(SI_BASE + offset, 1).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled SI read_u8");
            0
        }) as u8
    }

    pub fn mmio_write_u8(&mut self, offset: u32, val: u8) {
        if !self.write_raw(SI_BASE + offset, 1, val as u32) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled SI write_u8");
        }
    }

    pub fn mmio_read_u16(&mut self, offset: u32) -> u16 {
        self.read_raw(SI_BASE + offset, 2).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled SI read_u16");
            0
        }) as u16
    }

    pub fn mmio_write_u16(&mut self, offset: u32, val: u16) {
        if !self.write_raw(SI_BASE + offset, 2, val as u32) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled SI write_u16");
        }
    }

    pub fn mmio_read_u32(&mut self, offset: u32) -> u32 {
        self.read_raw(SI_BASE + offset, 4).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled SI read_u32");
            0
        })
    }

    pub fn mmio_write_u32(&mut self, offset: u32, val: u32) {
        if !self.write_raw(SI_BASE + offset, 4, val) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled SI write_u32");
        }
    }
}

impl crate::gekko::Gekko {
    pub fn check_si_interrupts(&mut self) {
        if self.si.interrupt_active() {
            self.pi.assert_interrupt(crate::flipper::pi::InterruptFlag::Si);
        } else {
            self.pi.clear_interrupt(crate::flipper::pi::InterruptFlag::Si);
        }
    }
}
