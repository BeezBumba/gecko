use super::ProcessorInterface;

// 0xCC003000  4  r    INTSR - Interrupt Cause

crate::mmio_register! {
    InterruptCause: u32 @ 0xCC003000 {
        #[bits(0, alias = "error")]
        pub gp_runtime_error: bool,

        #[bits(1, alias = "rsw")]
        pub reset_switch: bool,

        #[bits(2, alias = "di")]
        pub dvd: bool,

        #[bits(3, alias = "si")]
        pub serial: bool,

        #[bits(4)]
        pub exi: bool,

        #[bits(5, alias = "ai")]
        pub streaming: bool,

        #[bits(6)]
        pub dsp: bool,

        #[bits(7, alias = "mem")]
        pub memory: bool,

        #[bits(8, alias = "vi")]
        pub video: bool,

        #[bits(9, alias = "pe_token")]
        pub token_assertion_in_cmd_list: bool,

        #[bits(10, alias = "pe_finish")]
        pub frame_is_ready: bool,

        #[bits(11, alias = "cp")]
        pub command_fifo: bool,

        #[bits(12)]
        pub debug: bool,

        #[bits(13, alias = "hsp")]
        pub highspeed_port: bool,

        #[bits(16)]
        pub rswst: bool,
    }
}

impl crate::mmio::traits::MmioAccess<ProcessorInterface> for InterruptCause {
    fn read(dev: &ProcessorInterface) -> Self {
        dev.intsr
    }

    fn write(self, dev: &mut ProcessorInterface) {
        // yagcd seems to be wrong, we should not
        // clear everything on read, but just usual w1c instead
        const RSWST_MASK: u32 = 1 << 16;
        let cleared = dev.intsr.raw() & !self.raw();
        dev.intsr = InterruptCause::from_raw(cleared | (dev.intsr.raw() & RSWST_MASK));
    }
}

impl Default for InterruptCause {
    fn default() -> Self {
        // RSWST needs to be set
        Self::from_raw(0).with_rswst(true)
    }
}

// 0xCC003004  4  r/w  INTMR - Interrupt Mask
crate::mmio_register! {
    InterruptMask: u32 @ 0xCC003004 => ProcessorInterface.intmr {
        #[bits(0, alias = "error")]
        pub gp_runtime_error: bool,

        #[bits(1, alias = "rsw")]
        pub reset_switch: bool,

        #[bits(2, alias = "di")]
        pub dvd: bool,

        #[bits(3, alias = "si")]
        pub serial: bool,

        #[bits(4)]
        pub exi: bool,

        #[bits(5, alias = "ai")]
        pub streaming: bool,

        #[bits(6)]
        pub dsp: bool,

        #[bits(7, alias = "mem")]
        pub memory: bool,

        #[bits(8, alias = "vi")]
        pub video: bool,

        #[bits(9, alias = "pe_token")]
        pub token_assertion_in_cmd_list: bool,

        #[bits(10, alias = "pe_finish")]
        pub frame_is_ready: bool,

        #[bits(11, alias = "cp")]
        pub command_fifo: bool,

        #[bits(12)]
        pub debug: bool,

        #[bits(13, alias = "hsp")]
        pub highspeed_port: bool,
    }
}

// 0xCC003024  4  r/w  Console Reset Code

crate::mmio_register! {
    ResetCode: u32 @ 0xCC003024 {}
}

impl crate::mmio::traits::MmioAccess<ProcessorInterface> for ResetCode {
    fn read(_pi: &ProcessorInterface) -> Self {
        Self::from_raw(0)
    }

    fn write(self, _pi: &mut ProcessorInterface) {
        tracing::warn!("TODO: reset DVD");
    }
}

// 0xCC00302C  4  r    PI_FLIPPER_REV - Flipper Chip Revision

crate::mmio_register! {
    FlipperRev: u32 @ 0xCC00302C {
        #[bits(28..=31)]
        pub revision: u8,
    }
}

impl crate::mmio::traits::MmioAccess<super::ProcessorInterface> for FlipperRev {
    fn read(_pi: &super::ProcessorInterface) -> Self {
        // FLIPPER_REV_C from Dolphin
        Self::from_raw(0x2465_00B1)
    }

    fn write(self, _pi: &mut super::ProcessorInterface) {
        tracing::warn!("writing to FlipperRev???");
    }
}
