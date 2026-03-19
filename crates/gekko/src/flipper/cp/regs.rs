use super::Cp;
use crate::mmio::traits::MmioAccess;

// 0xCC000000  2  R/W   CP_STATUS - CP Status Register

crate::mmio_register! {
    CpStatus: u16 @ 0xCC000000 => Cp.status {
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
    CpControl: u16 @ 0xCC000002 => Cp.control {
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

impl MmioAccess<Cp> for CpClear {
    fn read(_cp: &Cp) -> Self {
        tracing::warn!("attempted to read from write-only CpClear register");
        Self::from_raw(0)
    }

    fn write(self, cp: &mut Cp) {
        if self.clear_overflow() {
            cp.status = cp.status.with_fifo_overflow(false);
        }

        if self.clear_underflow() {
            cp.status = cp.status.with_fifo_underflow(false);
        }
    }
}

crate::mmio_register! { FifoBaseLo: u16 @ 0xCC000020 => Cp.fifo_base_lo {} }
crate::mmio_register! { FifoBaseHi: u16 @ 0xCC000022 => Cp.fifo_base_hi {} }
crate::mmio_register! { FifoEndLo: u16 @ 0xCC000024 => Cp.fifo_end_lo {} }
crate::mmio_register! { FifoEndHi: u16 @ 0xCC000026 => Cp.fifo_end_hi {} }
crate::mmio_register! { FifoHiWatermarkLo: u16 @ 0xCC000028 => Cp.fifo_hi_watermark_lo {} }
crate::mmio_register! { FifoHiWatermarkHi: u16 @ 0xCC00002A => Cp.fifo_hi_watermark_hi {} }
crate::mmio_register! { FifoLoWatermarkLo: u16 @ 0xCC00002C => Cp.fifo_lo_watermark_lo {} }
crate::mmio_register! { FifoLoWatermarkHi: u16 @ 0xCC00002E => Cp.fifo_lo_watermark_hi {} }
crate::mmio_register! { FifoRwDistanceLo: u16 @ 0xCC000030 => Cp.fifo_rw_distance_lo {} }
crate::mmio_register! { FifoRwDistanceHi: u16 @ 0xCC000032 => Cp.fifo_rw_distance_hi {} }
crate::mmio_register! { FifoWritePtrLo: u16 @ 0xCC000034 => Cp.fifo_write_ptr_lo {} }
crate::mmio_register! { FifoWritePtrHi: u16 @ 0xCC000036 => Cp.fifo_write_ptr_hi {} }
crate::mmio_register! { FifoReadPtrLo: u16 @ 0xCC000038 => Cp.fifo_read_ptr_lo {} }
crate::mmio_register! { FifoReadPtrHi: u16 @ 0xCC00003A => Cp.fifo_read_ptr_hi {} }
