pub mod regs;

use crate::mmio::constants::EXI_BASE;
use crate::mmio::traits::{MmioAccess, MmioRegister};

pub struct Exi {
    // Channel 0
    pub ch0_csr: regs::Channel0Status,
    pub ch0_mar: regs::Channel0DmaAddress,
    pub ch0_length: regs::Channel0DmaLength,
    pub ch0_cr: regs::Channel0Control,
    pub ch0_data: regs::Channel0Data,
    // Channel 1
    pub ch1_csr: regs::Channel1Status,
    pub ch1_mar: regs::Channel1DmaAddress,
    pub ch1_length: regs::Channel1DmaLength,
    pub ch1_cr: regs::Channel1Control,
    pub ch1_data: regs::Channel1Data,
    // Channel 2
    pub ch2_csr: regs::Channel2Status,
    pub ch2_mar: regs::Channel2DmaAddress,
    pub ch2_length: regs::Channel2DmaLength,
    pub ch2_cr: regs::Channel2Control,
    pub ch2_data: regs::Channel2Data,
}

impl Exi {
    pub fn new() -> Self {
        Exi {
            ch0_csr: regs::Channel0Status::from_raw(0),
            ch0_mar: regs::Channel0DmaAddress::from_raw(0),
            ch0_length: regs::Channel0DmaLength::from_raw(0),
            ch0_cr: regs::Channel0Control::from_raw(0),
            ch0_data: regs::Channel0Data::from_raw(0),
            ch1_csr: regs::Channel1Status::from_raw(0),
            ch1_mar: regs::Channel1DmaAddress::from_raw(0),
            ch1_length: regs::Channel1DmaLength::from_raw(0),
            ch1_cr: regs::Channel1Control::from_raw(0),
            ch1_data: regs::Channel1Data::from_raw(0),
            ch2_csr: regs::Channel2Status::from_raw(0),
            ch2_mar: regs::Channel2DmaAddress::from_raw(0),
            ch2_length: regs::Channel2DmaLength::from_raw(0),
            ch2_cr: regs::Channel2Control::from_raw(0),
            ch2_data: regs::Channel2Data::from_raw(0),
        }
    }

    crate::impl_mmio_dispatch!(
        regs::Channel0Status,
        regs::Channel0DmaAddress,
        regs::Channel0DmaLength,
        regs::Channel0Control,
        regs::Channel0Data,
        regs::Channel1Status,
        regs::Channel1DmaAddress,
        regs::Channel1DmaLength,
        regs::Channel1Control,
        regs::Channel1Data,
        regs::Channel2Status,
        regs::Channel2DmaAddress,
        regs::Channel2DmaLength,
        regs::Channel2Control,
        regs::Channel2Data,
    );

    pub fn mmio_read_u8(&self, offset: u32) -> u8 {
        self.read_raw(EXI_BASE + offset, 1).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled EXI read_u8");
            0
        }) as u8
    }

    pub fn mmio_write_u8(&mut self, offset: u32, val: u8) {
        if !self.write_raw(EXI_BASE + offset, 1, val as u32) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled EXI write_u8");
        }
    }

    pub fn mmio_read_u16(&self, offset: u32) -> u16 {
        self.read_raw(EXI_BASE + offset, 2).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled EXI read_u16");
            0
        }) as u16
    }

    pub fn mmio_write_u16(&mut self, offset: u32, val: u16) {
        if !self.write_raw(EXI_BASE + offset, 2, val as u32) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled EXI write_u16");
        }
    }

    pub fn mmio_read_u32(&self, offset: u32) -> u32 {
        self.read_raw(EXI_BASE + offset, 4).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled EXI read_u32");
            0
        })
    }

    pub fn mmio_write_u32(&mut self, offset: u32, val: u32) {
        if !self.write_raw(EXI_BASE + offset, 4, val) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled EXI write_u32");
        }
    }
}
