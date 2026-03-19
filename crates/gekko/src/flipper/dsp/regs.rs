use chapa::BitEnum;

use crate::{flipper::dsp::Dsp, mmio::traits::MmioAccess};

// 0xCC00500A 2 [R/W] CSR - DSP Control/Status Register

crate::mmio_register! {
    ControlStatus: u16 @ 0xCC00500A {
        #[bits(0, alias = "res")]
        pub reset: bool,

        #[bits(1, alias = "piint")]
        pub pi_interrupt: bool,

        #[bits(2)]
        pub halt: bool,

        #[bits(3, alias = "aiint")]
        pub ai_interrupt: bool,

        #[bits(4, alias = "aiintmask")]
        pub ai_interrupt_mask: bool,

        #[bits(5, alias = "arint")]
        pub ar_interrupt: bool,

        #[bits(6, alias = "arintmask")]
        pub ar_interrupt_mask: bool,

        #[bits(7, alias = "dspint")]
        pub dsp_interrupt: bool,

        #[bits(8, alias = "dspintmask")]
        pub dsp_interrupt_mask: bool,

        #[bits(9, alias = "dspdma")]
        pub dma_status: bool,

        #[bits(11, alias = "ucode")]
        pub upload_ucode: bool, // also reset bit?
    }
}

impl MmioAccess<super::Dsp> for ControlStatus {
    fn read(dsp: &super::Dsp) -> Self {
        dsp.csr
    }

    fn write(self, dsp: &mut super::Dsp) {
        tracing::trace!("CSR write: {:016b}", self.raw());

        let mut csr = dsp.csr;

        if self.ai_interrupt() {
            csr = csr.with_ai_interrupt(false);
        }
        if self.ar_interrupt() {
            csr = csr.with_ar_interrupt(false);
        }
        if self.dsp_interrupt() {
            csr = csr.with_dsp_interrupt(false);
        }

        csr = csr
            .with_pi_interrupt(self.pi_interrupt())
            .with_halt(self.halt())
            .with_ai_interrupt_mask(self.ai_interrupt_mask())
            .with_ar_interrupt_mask(self.ar_interrupt_mask())
            .with_dsp_interrupt_mask(self.dsp_interrupt_mask())
            .with_dma_status(self.dma_status())
            .with_upload_ucode(self.upload_ucode());

        // ucode falling edge (bit 11: 1->0) schedules ucode upload from ARAM[0x0000]
        if dsp.csr.upload_ucode() && !self.upload_ucode() {
            tracing::debug!("scheduling ucode upload");
            dsp.pending_ucode_upload = true;
        }

        // ACK reset bit
        if self.reset() {
            tracing::debug!("reset");
        }
        csr = csr.with_reset(false);

        dsp.csr = csr;
    }
}

impl Default for ControlStatus {
    fn default() -> Self {
        ControlStatus::new().with_halt(true).with_upload_ucode(true)
    }
}

// 0xCC005000 2 [W] CPU-to-DSP Mailbox High
// TODO: Since we don't emulate the DSP processor, always read busy as 0

crate::mmio_register! {
    MailboxToDspHi: u16 @ 0xCC005000 {
        #[bits(0..=14)]
        pub data: u16,

        #[bits(15)]
        pub busy: bool,
    }
}

impl MmioAccess<Dsp> for MailboxToDspHi {
    fn read(dsp: &Dsp) -> Self {
        // TODO: instantly consume it for now
        dsp.mailbox_to_dsp_hi.with_busy(false)
    }

    fn write(self, dsp: &mut Dsp) {
        dsp.mailbox_to_dsp_hi = self;
    }
}

// 0xCC005002 2 [W] CPU-to-DSP Mailbox Low

crate::mmio_register! {
    MailboxToDspLo: u16 @ 0xCC005002 => Dsp.mailbox_to_dsp_lo {}
}

// 0xCC005004 2 [R] DSP-to-CPU Mailbox High

crate::mmio_register! {
    MailboxToCpuHi: u16 @ 0xCC005004 => Dsp.mailbox_to_cpu_hi {}
}

// 0xCC005006 2 [R] DSP-to-CPU Mailbox Low

crate::mmio_register! {
    MailboxToCpuLo: u16 @ 0xCC005006 => Dsp.mailbox_to_cpu_lo {}
}

// 0xCC005012 2 [R/W] AR_INFO - ARAM Info/Size

crate::mmio_register! {
    AramInfo: u16 @ 0xCC005012 => Dsp.aram_info {}
}

// 0xCC00501A 2 [R/W] AR_REFRESH - ARAM Refresh

crate::mmio_register! {
    AramRefresh: u16 @ 0xCC00501A => Dsp.aram_refresh {}
}

// 0xCC005020 4 [R/W] ARAM DMA MMIO Address

crate::mmio_register! {
    AramDmaMmioAddr: u32 @ 0xCC005020 => Dsp.aram_dma_mmio_addr {}
}

// 0xCC005024 4 [R/W] ARAM DMA ARAM Address

crate::mmio_register! {
    AramDmaAramAddr: u32 @ 0xCC005024 => Dsp.aram_dma_aram_addr {}
}

// 0xCC005028 4 [R/W] ARAM DMA Count/Control

#[derive(BitEnum, Debug, PartialEq, Eq)]
pub enum DmaDirection {
    RamToAram = 0,
    AramToRam = 1,
}

crate::mmio_register! {
    AramDmaControl: u32 @ 0xCC005028 {
        #[bits(0..=30)]
        pub count: u32,

        #[bits(31)]
        pub direction: DmaDirection,
    }
}

impl MmioAccess<super::Dsp> for AramDmaControl {
    fn read(dsp: &super::Dsp) -> Self {
        dsp.aram_dma_control
    }

    fn write(self, dsp: &mut super::Dsp) {
        dsp.aram_dma_control = self;

        // Schedule the ARAM->RAM DMA immediately when this register is written
        dsp.pending_aram_dma = true;
    }
}
