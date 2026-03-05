use super::Dsp;

// 0xCC00500A	2	R/W	CSR - DSP Control/Status Register

crate::mmio_register! {
    ControlStatus: u16 @ 0xCC00500A => Dsp.csr {
        #[bits(0..=15, alias = "csr")] pub value: u16,
    }
}
