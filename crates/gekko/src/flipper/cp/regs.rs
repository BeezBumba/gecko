use super::CommandProcessor;
use crate::mmio::traits::MmioAccess;

// 0xCC000000  2  R/W   CP_STATUS - CP Status Register

crate::mmio_register! {
    CpStatus: u16 @ 0xCC000000 => CommandProcessor.status {
        #[bits(0)]
        pub fifo_overflow: bool,

        #[bits(1)]
        pub fifo_underflow: bool,

        #[bits(2)]
        pub read_idle: bool,

        #[bits(3)]
        pub cmd_idle: bool,

        #[bits(4)]
        pub bp_interrupt: bool,
    }
}

// 0xCC000002  2  R/W  CP_CTRL - CP Control Register

crate::mmio_register! {
    CpControl: u16 @ 0xCC000002 => CommandProcessor.control {
        #[bits(0)]
        pub gp_fifo_read_enable: bool,

        #[bits(1)]
        pub cp_interrupt_enable: bool,

        #[bits(2)]
        pub fifo_overflow_interrupt_enable: bool,

        #[bits(3)]
        pub fifo_underflow_interrupt_enable: bool,

        #[bits(4)]
        pub gp_link_enable: bool,

        #[bits(5)]
        pub bp_interrupt_enable: bool,
    }
}

// 0xCC000004  2  W   Clear Register

crate::mmio_register! {
    CpClear: u16 @ 0xCC000004 {
        #[bits(0)]
        pub clear_overflow: bool,

        #[bits(1)]
        pub clear_underflow: bool,
    }
}

impl MmioAccess<CommandProcessor> for CpClear {
    fn read(_cp: &CommandProcessor) -> Self {
        tracing::warn!("attempted to read from write-only CpClear register");
        Self::from_raw(0)
    }

    fn write(self, cp: &mut CommandProcessor) {
        if self.clear_overflow() {
            cp.status = cp.status.with_fifo_overflow(false);
        }

        if self.clear_underflow() {
            cp.status = cp.status.with_fifo_underflow(false);
        }
    }
}

crate::mmio_register! { FifoBaseLo: u16 @ 0xCC000020 => CommandProcessor.fifo_base_lo {} }
crate::mmio_register! { FifoBaseHi: u16 @ 0xCC000022 => CommandProcessor.fifo_base_hi {} }
crate::mmio_register! { FifoEndLo: u16 @ 0xCC000024 => CommandProcessor.fifo_end_lo {} }
crate::mmio_register! { FifoEndHi: u16 @ 0xCC000026 => CommandProcessor.fifo_end_hi {} }
crate::mmio_register! { FifoHiWatermarkLo: u16 @ 0xCC000028 => CommandProcessor.fifo_hi_watermark_lo {} }
crate::mmio_register! { FifoHiWatermarkHi: u16 @ 0xCC00002A => CommandProcessor.fifo_hi_watermark_hi {} }
crate::mmio_register! { FifoLoWatermarkLo: u16 @ 0xCC00002C => CommandProcessor.fifo_lo_watermark_lo {} }
crate::mmio_register! { FifoLoWatermarkHi: u16 @ 0xCC00002E => CommandProcessor.fifo_lo_watermark_hi {} }
crate::mmio_register! { FifoRwDistanceLo: u16 @ 0xCC000030 => CommandProcessor.fifo_rw_distance_lo {} }
crate::mmio_register! { FifoRwDistanceHi: u16 @ 0xCC000032 => CommandProcessor.fifo_rw_distance_hi {} }
crate::mmio_register! { FifoWritePtrLo: u16 @ 0xCC000034 => CommandProcessor.fifo_write_ptr_lo {} }
crate::mmio_register! { FifoWritePtrHi: u16 @ 0xCC000036 => CommandProcessor.fifo_write_ptr_hi {} }
crate::mmio_register! { FifoReadPtrLo: u16 @ 0xCC000038 => CommandProcessor.fifo_read_ptr_lo {} }
crate::mmio_register! { FifoReadPtrHi: u16 @ 0xCC00003A => CommandProcessor.fifo_read_ptr_hi {} }
