use super::Mi;

// 0xCC00401C	4	R/W	INTMR - Memory Interface Interrupt Mask

crate::mmio_register! {
    InterruptMask: u32 @ 0xCC00401C => Mi.intmr {
        #[bits(0..=31, alias = "intmr")] pub mask: u32,
    }
}
