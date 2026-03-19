pub mod regs;

use crate::mmio::constants::CP_BASE;
use crate::mmio::traits::{MmioAccess, MmioRegister};

pub struct Cp {
    pub status: regs::CpStatus,
    pub control: regs::CpControl,
    pub fifo_base_lo: regs::FifoBaseLo,
    pub fifo_base_hi: regs::FifoBaseHi,
    pub fifo_end_lo: regs::FifoEndLo,
    pub fifo_end_hi: regs::FifoEndHi,
    pub fifo_hi_watermark_lo: regs::FifoHiWatermarkLo,
    pub fifo_hi_watermark_hi: regs::FifoHiWatermarkHi,
    pub fifo_lo_watermark_lo: regs::FifoLoWatermarkLo,
    pub fifo_lo_watermark_hi: regs::FifoLoWatermarkHi,
    pub fifo_rw_distance_lo: regs::FifoRwDistanceLo,
    pub fifo_rw_distance_hi: regs::FifoRwDistanceHi,
    pub fifo_write_ptr_lo: regs::FifoWritePtrLo,
    pub fifo_write_ptr_hi: regs::FifoWritePtrHi,
    pub fifo_read_ptr_lo: regs::FifoReadPtrLo,
    pub fifo_read_ptr_hi: regs::FifoReadPtrHi,
}

impl Cp {
    pub fn new() -> Self {
        Self {
            status: regs::CpStatus::from_raw(0),
            control: regs::CpControl::from_raw(0),
            fifo_base_lo: regs::FifoBaseLo::from_raw(0),
            fifo_base_hi: regs::FifoBaseHi::from_raw(0),
            fifo_end_lo: regs::FifoEndLo::from_raw(0),
            fifo_end_hi: regs::FifoEndHi::from_raw(0),
            fifo_hi_watermark_lo: regs::FifoHiWatermarkLo::from_raw(0),
            fifo_hi_watermark_hi: regs::FifoHiWatermarkHi::from_raw(0),
            fifo_lo_watermark_lo: regs::FifoLoWatermarkLo::from_raw(0),
            fifo_lo_watermark_hi: regs::FifoLoWatermarkHi::from_raw(0),
            fifo_rw_distance_lo: regs::FifoRwDistanceLo::from_raw(0),
            fifo_rw_distance_hi: regs::FifoRwDistanceHi::from_raw(0),
            fifo_write_ptr_lo: regs::FifoWritePtrLo::from_raw(0),
            fifo_write_ptr_hi: regs::FifoWritePtrHi::from_raw(0),
            fifo_read_ptr_lo: regs::FifoReadPtrLo::from_raw(0),
            fifo_read_ptr_hi: regs::FifoReadPtrHi::from_raw(0),
        }
    }

    pub fn interrupt_active(&self) -> bool {
        (self.status.bp_interrupt() && self.control.bp_interrupt_enable())
            || (self.status.fifo_overflow() && self.control.fifo_overflow_interrupt_enable())
            || (self.status.fifo_underflow() && self.control.fifo_underflow_interrupt_enable())
    }

    crate::impl_mmio_dispatch!(
        regs::CpStatus,
        regs::CpControl,
        regs::CpClear,
        regs::FifoBaseLo,
        regs::FifoBaseHi,
        regs::FifoEndLo,
        regs::FifoEndHi,
        regs::FifoHiWatermarkLo,
        regs::FifoHiWatermarkHi,
        regs::FifoLoWatermarkLo,
        regs::FifoLoWatermarkHi,
        regs::FifoRwDistanceLo,
        regs::FifoRwDistanceHi,
        regs::FifoWritePtrLo,
        regs::FifoWritePtrHi,
        regs::FifoReadPtrLo,
        regs::FifoReadPtrHi,
    );

    pub fn mmio_read_u8(&mut self, offset: u32) -> u8 {
        self.read_raw(CP_BASE + offset, 1).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled CP read_u8");
            0
        }) as u8
    }

    pub fn mmio_write_u8(&mut self, offset: u32, val: u8) {
        if !self.write_raw(CP_BASE + offset, 1, val as u32) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled CP write_u8");
        }
    }

    pub fn mmio_read_u16(&mut self, offset: u32) -> u16 {
        self.read_raw(CP_BASE + offset, 2).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled CP read_u16");
            0
        }) as u16
    }

    pub fn mmio_write_u16(&mut self, offset: u32, val: u16) {
        if !self.write_raw(CP_BASE + offset, 2, val as u32) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled CP write_u16");
        }
    }

    pub fn mmio_read_u32(&mut self, offset: u32) -> u32 {
        self.read_raw(CP_BASE + offset, 4).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled CP read_u32");
            0
        })
    }

    pub fn mmio_write_u32(&mut self, offset: u32, val: u32) {
        if !self.write_raw(CP_BASE + offset, 4, val) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled CP write_u32");
        }
    }
}

impl crate::gekko::Gekko {
    pub fn check_cp_interrupts(&mut self) {
        if self.cp.interrupt_active() {
            self.pi.assert_interrupt(crate::flipper::pi::InterruptFlag::Cp);
        } else {
            self.pi.clear_interrupt(crate::flipper::pi::InterruptFlag::Cp);
        }
    }
}
