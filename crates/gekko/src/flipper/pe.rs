pub mod regs;
use crate::{
    flipper::pi::InterruptFlag,
    gekko::Gekko,
    mmio::{
        constants::PE_BASE,
        traits::{MmioAccess, MmioRegister},
    },
};

pub struct Pe {
    pub zconf: regs::ZConfig,
    pub alphaconf: regs::AlphaConfig,
    pub dst_alphaconf: regs::DstAlphaConfig,
    pub alphamode: regs::AlphaMode,
    pub alpharead: regs::AlphaRead,
    pub sr: regs::InterruptStatus,
    pub token: regs::Token,
}

impl Pe {
    pub fn new() -> Self {
        Self {
            zconf: regs::ZConfig::from_raw(0),
            alphaconf: regs::AlphaConfig::from_raw(0),
            dst_alphaconf: regs::DstAlphaConfig::from_raw(0),
            alphamode: regs::AlphaMode::from_raw(0),
            alpharead: regs::AlphaRead::from_raw(0),
            sr: regs::InterruptStatus::from_raw(0),
            token: regs::Token::from_raw(0),
        }
    }

    pub fn finish_interrupt_active(&self) -> bool {
        self.sr.pe_finish_enable() && self.sr.pe_finish()
    }

    pub fn token_interrupt_active(&self) -> bool {
        self.sr.pe_token_enable() && self.sr.pe_token()
    }

    pub fn signal_finish(&mut self) {
        self.sr = self.sr.with_pe_finish(true);
    }

    pub fn signal_token(&mut self, token: u16) {
        self.token = regs::Token::from_raw(token.into());
        self.sr = self.sr.with_pe_token(true);
    }

    crate::impl_mmio_dispatch!(
        regs::ZConfig,
        regs::AlphaConfig,
        regs::DstAlphaConfig,
        regs::AlphaMode,
        regs::AlphaRead,
        regs::InterruptStatus,
        regs::Token,
    );

    pub fn mmio_read_u8(&mut self, offset: u32) -> u8 {
        self.read_raw(PE_BASE + offset, 1).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled PE read_u8");
            0
        }) as u8
    }

    pub fn mmio_write_u8(&mut self, offset: u32, val: u8) {
        if !self.write_raw(PE_BASE + offset, 1, val as u32) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled PE write_u8");
        }
    }

    pub fn mmio_read_u16(&mut self, offset: u32) -> u16 {
        self.read_raw(PE_BASE + offset, 2).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled PE read_u16");
            0
        }) as u16
    }

    pub fn mmio_write_u16(&mut self, offset: u32, val: u16) {
        if !self.write_raw(PE_BASE + offset, 2, val as u32) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled PE write_u16");
        }
    }

    pub fn mmio_read_u32(&mut self, offset: u32) -> u32 {
        self.read_raw(PE_BASE + offset, 4).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled PE read_u32");
            0
        })
    }

    pub fn mmio_write_u32(&mut self, offset: u32, val: u32) {
        if !self.write_raw(PE_BASE + offset, 4, val) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled PE write_u32");
        }
    }
}

impl Gekko {
    pub fn check_pe_interrupts(&mut self) {
        if self.pe.token_interrupt_active() {
            self.pi.assert_interrupt(InterruptFlag::PeToken);
        } else {
            self.pi.clear_interrupt(InterruptFlag::PeToken);
        }

        if self.pe.finish_interrupt_active() {
            self.pi.assert_interrupt(InterruptFlag::PeFinish);
        } else {
            self.pi.clear_interrupt(InterruptFlag::PeFinish);
        }
    }
}
