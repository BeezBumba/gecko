pub mod regs;

use crate::mmio::Mmio;
use crate::mmio::constants::DSP_BASE;
use crate::mmio::traits::{MmioAccess, MmioRegister};

pub struct Dsp {
    // IMEM = IRAM + IROM
    pub iram: Box<[u8; 0x1000]>, // 0x0000 - 0x0FFF
    pub irom: Box<[u8; 0x1000]>, // 0x8000 - 0x8FFF

    // DMEM = DRAM + COEF + IFX
    pub dram: Box<[u8; 0x1000]>, // 0x0000 - 0x0FFF
    pub coef: Box<[u8; 0x1000]>, // 0x1000 - 0x1FFF
    pub ifx: Box<[u8; 0x100]>,   // 0xFF00 - 0xFFFF

    // Auxiliary RAM (16 MB)
    pub aram: Box<[u8; 16 * 1024 * 1024]>,

    // I/O Registers
    pub csr: regs::ControlStatus,
    pub mailbox_to_cpu_hi: regs::MailboxToCpuHi,
    pub mailbox_to_cpu_lo: regs::MailboxToCpuLo,
    pub dma_mmio_addr: regs::DmaMmioAddr,
    pub dma_aram_addr: regs::DmaAramAddr,
    pub dma_control: regs::DmaControl,

    // Flags set by register write handlers, consumed by process_pending_dma
    pub pending_aram_dma: bool,
    pub pending_ucode_upload: bool,
}

impl Dsp {
    pub fn new() -> Self {
        let aram = unsafe { Box::<[u8; 16 * 1024 * 1024]>::new_zeroed().assume_init() };
        let iram = unsafe { Box::<[u8; 0x1000]>::new_zeroed().assume_init() };
        let irom = unsafe { Box::<[u8; 0x1000]>::new_zeroed().assume_init() };
        let dram = unsafe { Box::<[u8; 0x1000]>::new_zeroed().assume_init() };
        let coef = unsafe { Box::<[u8; 0x1000]>::new_zeroed().assume_init() };
        let ifx = unsafe { Box::<[u8; 0x100]>::new_zeroed().assume_init() };

        Dsp {
            iram,
            irom,
            dram,
            coef,
            ifx,
            aram,
            csr: regs::ControlStatus::default(),
            mailbox_to_cpu_hi: regs::MailboxToCpuHi::from_raw(0),
            mailbox_to_cpu_lo: regs::MailboxToCpuLo::from_raw(0),
            dma_mmio_addr: regs::DmaMmioAddr::from_raw(0),
            dma_aram_addr: regs::DmaAramAddr::from_raw(0),
            dma_control: regs::DmaControl::from_raw(0),
            pending_aram_dma: false,
            pending_ucode_upload: false,
        }
    }

    /// Called by the bus after every DSP MMIO write
    ///
    /// - If an ARAM DMA was triggered (write to DmaControl), execute the transfer and
    ///   assert ARINT in the CSR
    /// - If ucode upload (falling-edge) was detected (CSR bit 11: 1->0), copy from
    ///   ARAM[0x0000] into IRAM, assert ucode status (CSR bit 10, auto-clears on next CSR
    ///   read), and write the mailbox values (TODO: HLE)
    /// 
    /// TODO: However, this entire thing might be bogus. Dolphin also isn't sure whether
    /// There is a preliminary transfer to ARAM before transfering to IRAM. It might
    /// actually just be hardcoded to upload ucode from main RAM directly on bit 11 reset...
    /// We should be able to verify this on hardware by writing different instruction
    /// sequences to ARAM and main RAM. These sequences should write a unique message to
    /// the mailbox, so we can determine which one is actually being executed by the DSP.
    #[inline]
    pub fn process_pending_dma(&mut self, mmio: &mut Mmio) {
        // Handle ARAM DMA
        if self.pending_aram_dma {
            self.pending_aram_dma = false;

            let mmio_addr = self.dma_mmio_addr.raw() as usize;
            let aram_addr = self.dma_aram_addr.raw() as usize;
            let count = self.dma_control.count() as usize * 4;

            if self.dma_control.direction() == regs::DmaDirection::AramToRam {
                // ARAM -> main RAM
                let src = self.aram[aram_addr..aram_addr + count].to_vec();
                mmio.ram[mmio_addr..mmio_addr + count].copy_from_slice(&src);
            } else {
                // main RAM -> ARAM
                let src = mmio.ram[mmio_addr..mmio_addr + count].to_vec();
                self.aram[aram_addr..aram_addr + count].copy_from_slice(&src);
            }

            tracing::debug!(
                mmio_addr = format!("{mmio_addr:08X}"),
                aram_addr = format!("{aram_addr:08X}"),
                count,
                direction = ?self.dma_control.direction(),
                "ARAM DMA complete"
            );

            // TODO: Cause actual interrupt?
            self.csr = self.csr.with_ar_interrupt(true);
        }

        // Handle DSP ucode upload
        if self.pending_ucode_upload {
            self.pending_ucode_upload = false;

            let count = self.dma_control.count() as usize * 4;
            let src = self.aram[..count].to_vec();
            self.iram[..count].copy_from_slice(&src);

            tracing::debug!(count, "uploaded ucode from ARAM[0x0000] to IRAM");

            // Assert ucode upload finished until next read
            self.csr = self.csr.with_upload_ucode_finished(true);

            // HLE: Write expected response to mailbox
            self.mailbox_to_cpu_hi = regs::MailboxToCpuHi::from_raw(0x8071);
            self.mailbox_to_cpu_lo = regs::MailboxToCpuLo::from_raw(0xFEED);
        }
    }

    crate::impl_mmio_dispatch!(
        regs::ControlStatus,
        regs::MailboxToCpuHi,
        regs::MailboxToCpuLo,
        regs::DmaMmioAddr,
        regs::DmaAramAddr,
        regs::DmaControl,
    );

    pub fn mmio_read_u8(&mut self, offset: u32) -> u8 {
        self.read_raw(DSP_BASE + offset, 1).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled DSP read_u8");
            0
        }) as u8
    }

    pub fn mmio_write_u8(&mut self, offset: u32, val: u8) {
        if !self.write_raw(DSP_BASE + offset, 1, val as u32) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled DSP write_u8");
        }
    }

    pub fn mmio_read_u16(&mut self, offset: u32) -> u16 {
        let addr = DSP_BASE + offset;
        let val = self.read_raw(addr, 2).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled DSP read_u16");
            0
        }) as u16;
        // DSPInitCode (bit 10) auto-clears after being read
        if regs::ControlStatus::contains(addr) {
            self.csr = self.csr.with_upload_ucode_finished(false);
        }
        val
    }

    pub fn mmio_write_u16(&mut self, offset: u32, val: u16) {
        if !self.write_raw(DSP_BASE + offset, 2, val as u32) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled DSP write_u16");
        }
    }

    pub fn mmio_read_u32(&mut self, offset: u32) -> u32 {
        self.read_raw(DSP_BASE + offset, 4).unwrap_or_else(|| {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled DSP read_u32");
            0
        })
    }

    pub fn mmio_write_u32(&mut self, offset: u32, val: u32) {
        if !self.write_raw(DSP_BASE + offset, 4, val) {
            tracing::error!(offset = format!("{offset:08X}"), "unhandled DSP write_u32");
        }
    }
}
