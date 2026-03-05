use super::Ai;

// 0xCC006C00	4	R/W	AICR - Audio Interface Control Register

crate::mmio_register! {
    Control: u32 @ 0xCC006C00 => Ai.cr {
        #[bits(0..=31, alias = "cr")] pub value: u32,
    }
}
