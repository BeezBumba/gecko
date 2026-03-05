pub mod regs;

use crate::mmio::constants::PI_BASE;
use crate::mmio::traits::{MmioAccess, MmioRegister};

pub struct Pi {
    pub intmr: regs::InterruptMask,
    pub console_type: regs::ConsoleType,
}

impl Pi {
    pub fn new() -> Self {
        Pi {
            intmr: regs::InterruptMask::from_raw(0),
            console_type: regs::ConsoleType::from_raw(0),
        }
    }

    crate::impl_mmio_dispatch!(regs::InterruptMask, regs::ConsoleType,);

    pub fn mmio_read_u8(&self, offset: u32) -> u8 {
        self.read_raw(PI_BASE + offset, 1).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled PI read_u8");
            0
        }) as u8
    }

    pub fn mmio_write_u8(&mut self, offset: u32, val: u8) {
        if !self.write_raw(PI_BASE + offset, 1, val as u32) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled PI write_u8");
        }
    }

    pub fn mmio_read_u16(&self, offset: u32) -> u16 {
        self.read_raw(PI_BASE + offset, 2).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled PI read_u16");
            0
        }) as u16
    }

    pub fn mmio_write_u16(&mut self, offset: u32, val: u16) {
        if !self.write_raw(PI_BASE + offset, 2, val as u32) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled PI write_u16");
        }
    }

    pub fn mmio_read_u32(&self, offset: u32) -> u32 {
        self.read_raw(PI_BASE + offset, 4).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled PI read_u32");
            0
        })
    }

    pub fn mmio_write_u32(&mut self, offset: u32, val: u32) {
        if !self.write_raw(PI_BASE + offset, 4, val) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled PI write_u32");
        }
    }
}
