use super::Exi;

// --- Channel 0 ---

// 0xCC006800	4	R/W	EXI0CSR - EXI Channel 0 Status Register

crate::mmio_register! {
    Channel0Status: u32 @ 0xCC006800 => Exi.ch0_csr {
        #[bits(0, alias = "exiintmask")] pub exi_interrupt_mask: bool,
        #[bits(1, alias = "exiint")] pub exi_interrupt: bool,
        #[bits(2, alias = "tcintmask")] pub tc_interrupt_mask: bool,
        #[bits(3, alias = "tcint")] pub tc_interrupt: bool,
        #[bits(4..=6, alias = "clk")] pub clock: u8,
        #[bits(7..=9, alias = "cs")] pub chip_select: u8,
        #[bits(10, alias = "extintmask")] pub ext_interrupt_mask: bool,
        #[bits(11, alias = "extint")] pub ext_interrupt: bool,
        #[bits(12, alias = "ext")] pub device_connected: bool,
        #[bits(13, alias = "romdis")] pub rom_descramble_disabled: bool,
    }
}

// 0xCC006804	4	R/W	EXI0MAR - EXI Channel 0 DMA Start Address

crate::mmio_register! {
    Channel0DmaAddress: u32 @ 0xCC006804 => Exi.ch0_mar {
        #[bits(5..=25, alias = "addr")] pub address: u32,
    }
}

// 0xCC006808	4	R/W	EXI0LENGTH - EXI Channel 0 DMA Transfer Length

crate::mmio_register! {
    Channel0DmaLength: u32 @ 0xCC006808 => Exi.ch0_length {
        #[bits(5..=25, alias = "len")] pub length: u32,
    }
}

// 0xCC00680C	4	R/W	EXI0CR - EXI Channel 0 Control Register

crate::mmio_register! {
    Channel0Control: u32 @ 0xCC00680C => Exi.ch0_cr {
        #[bits(0, alias = "tstart")] pub transfer_start: bool,
        #[bits(1, alias = "dma")] pub dma_mode: bool,
        #[bits(2..=3, alias = "rw")] pub transfer_type: u8,
        #[bits(4..=5, alias = "tlen")] pub transfer_length: u8,
    }
}

// 0xCC006810	4	R/W	EXI0DATA - EXI Channel 0 Immediate Data

crate::mmio_register! {
    Channel0Data: u32 @ 0xCC006810 => Exi.ch0_data {
        #[bits(0..=31, alias = "data")] pub value: u32,
    }
}

// --- Channel 1 ---

// 0xCC006814	4	R/W	EXI1CSR - EXI Channel 1 Status Register

crate::mmio_register! {
    Channel1Status: u32 @ 0xCC006814 => Exi.ch1_csr {
        #[bits(0, alias = "exiintmask")] pub exi_interrupt_mask: bool,
        #[bits(1, alias = "exiint")] pub exi_interrupt: bool,
        #[bits(2, alias = "tcintmask")] pub tc_interrupt_mask: bool,
        #[bits(3, alias = "tcint")] pub tc_interrupt: bool,
        #[bits(4..=6, alias = "clk")] pub clock: u8,
        #[bits(7..=9, alias = "cs")] pub chip_select: u8,
        #[bits(10, alias = "extintmask")] pub ext_interrupt_mask: bool,
        #[bits(11, alias = "extint")] pub ext_interrupt: bool,
        #[bits(12, alias = "ext")] pub device_connected: bool,
    }
}

// 0xCC006818	4	R/W	EXI1MAR - EXI Channel 1 DMA Start Address

crate::mmio_register! {
    Channel1DmaAddress: u32 @ 0xCC006818 => Exi.ch1_mar {
        #[bits(5..=25, alias = "addr")] pub address: u32,
    }
}

// 0xCC00681C	4	R/W	EXI1LENGTH - EXI Channel 1 DMA Transfer Length

crate::mmio_register! {
    Channel1DmaLength: u32 @ 0xCC00681C => Exi.ch1_length {
        #[bits(5..=25, alias = "len")] pub length: u32,
    }
}

// 0xCC006820	4	R/W	EXI1CR - EXI Channel 1 Control Register

crate::mmio_register! {
    Channel1Control: u32 @ 0xCC006820 => Exi.ch1_cr {
        #[bits(0, alias = "tstart")] pub transfer_start: bool,
        #[bits(1, alias = "dma")] pub dma_mode: bool,
        #[bits(2..=3, alias = "rw")] pub transfer_type: u8,
        #[bits(4..=5, alias = "tlen")] pub transfer_length: u8,
    }
}

// 0xCC006824	4	R/W	EXI1DATA - EXI Channel 1 Immediate Data

crate::mmio_register! {
    Channel1Data: u32 @ 0xCC006824 => Exi.ch1_data {
        #[bits(0..=31, alias = "data")] pub value: u32,
    }
}

// --- Channel 2 ---

// 0xCC006828	4	R/W	EXI2CSR - EXI Channel 2 Status Register

crate::mmio_register! {
    Channel2Status: u32 @ 0xCC006828 => Exi.ch2_csr {
        #[bits(0, alias = "exiintmask")] pub exi_interrupt_mask: bool,
        #[bits(1, alias = "exiint")] pub exi_interrupt: bool,
        #[bits(2, alias = "tcintmask")] pub tc_interrupt_mask: bool,
        #[bits(3, alias = "tcint")] pub tc_interrupt: bool,
        #[bits(4..=6, alias = "clk")] pub clock: u8,
        #[bits(7..=9, alias = "cs")] pub chip_select: u8,
        #[bits(10, alias = "extintmask")] pub ext_interrupt_mask: bool,
        #[bits(11, alias = "extint")] pub ext_interrupt: bool,
        #[bits(12, alias = "ext")] pub device_connected: bool,
    }
}

// 0xCC00682C	4	R/W	EXI2MAR - EXI Channel 2 DMA Start Address

crate::mmio_register! {
    Channel2DmaAddress: u32 @ 0xCC00682C => Exi.ch2_mar {
        #[bits(5..=25, alias = "addr")] pub address: u32,
    }
}

// 0xCC006830	4	R/W	EXI2LENGTH - EXI Channel 2 DMA Transfer Length

crate::mmio_register! {
    Channel2DmaLength: u32 @ 0xCC006830 => Exi.ch2_length {
        #[bits(5..=25, alias = "len")] pub length: u32,
    }
}

// 0xCC006834	4	R/W	EXI2CR - EXI Channel 2 Control Register

crate::mmio_register! {
    Channel2Control: u32 @ 0xCC006834 => Exi.ch2_cr {
        #[bits(0, alias = "tstart")] pub transfer_start: bool,
        #[bits(1, alias = "dma")] pub dma_mode: bool,
        #[bits(2..=3, alias = "rw")] pub transfer_type: u8,
        #[bits(4..=5, alias = "tlen")] pub transfer_length: u8,
    }
}

// 0xCC006838	4	R/W	EXI2DATA - EXI Channel 2 Immediate Data

crate::mmio_register! {
    Channel2Data: u32 @ 0xCC006838 => Exi.ch2_data {
        #[bits(0..=31, alias = "data")] pub value: u32,
    }
}
