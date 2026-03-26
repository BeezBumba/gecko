use super::MemoryInterface;

// 0xCC00401C  2  R/W  Memory Interface Interrupt Mask

crate::mmio_register! {
    MiInterruptMask: u16 @ 0xCC00401C => MemoryInterface.interrupt_mask {
        #[bits(0)]
        pub mem0_enable: bool,

        #[bits(1)]
        pub mem1_enable: bool,

        #[bits(2)]
        pub mem2_enable: bool,

        #[bits(3)]
        pub mem3_enable: bool,

        #[bits(4)]
        pub master_enable: bool,
    }
}
