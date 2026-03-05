use super::Pi;

// 0xCC003004	4	R/W	INTMR - Processor Interface Interrupt Mask

crate::mmio_register! {
    InterruptMask: u32 @ 0xCC003004 => Pi.intmr {
        #[bits(0..=31, alias = "intmr")] pub mask: u32,
    }
}

// 0xCC00302C	4	R	Console Type

crate::mmio_register! {
    ConsoleType: u32 @ 0xCC00302C => Pi.console_type {
        #[bits(0..=31, alias = "ct")] pub value: u32,
    }
}
