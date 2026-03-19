use crate::{
    flipper::pe::Pe,
    mmio::traits::{MmioAccess, MmioRegister},
};

// 0xCC001000	2	R/W	Z Configuration

crate::mmio_register! {
    ZConfig: u16 @ 0xCC001000 => Pe.zconf {}
}

// 0xCC001002	2	R/W	Alpha Configuration

crate::mmio_register! {
    AlphaConfig: u16 @ 0xCC001002 => Pe.alphaconf {}
}

// 0xCC001004	2	R/W	Destination Alpha

crate::mmio_register! {
    DstAlphaConfig: u16 @ 0xCC001004 => Pe.dst_alphaconf {}
}

// 0xCC001006	2	R/W	Alpha Compare Mode

crate::mmio_register! {
    AlphaMode: u16 @ 0xCC001006 => Pe.alphamode {}
}

// 0xCC001008	2	R/W	Alpha Read Mode

crate::mmio_register! {
    AlphaRead: u16 @ 0xCC001008 => Pe.alpharead {}
}

// 0xCC00100A	2	R/W	Interrupt Status

crate::mmio_register! {
    InterruptStatus: u16 @ 0xCC00100A {
        #[bits(0)]
        pub pe_token_enable: bool,

        #[bits(1)]
        pub pe_finish_enable: bool,

        #[bits(2)]
        pub pe_token: bool,

        #[bits(3)]
        pub pe_finish: bool,
    }
}

impl MmioAccess<Pe> for InterruptStatus {
    fn read(pe: &Pe) -> Self {
        pe.sr
    }

    fn write(self, pe: &mut Pe) {
        Self::write_at(pe, Self::ADDR, Self::SIZE as u32, self.to_raw());
    }

    fn write_at(pe: &mut Pe, addr: u32, access_size: u32, val: u32) {
        const ENABLE_MASK: u32 = (1 << 0) | (1 << 1);
        const STATUS_MASK: u32 = (1 << 2) | (1 << 3);

        let current = pe.sr.to_raw();
        let merged = Self::write_sub(current, addr, access_size, val);
        let written_bits = Self::write_sub(0, addr, access_size, val);
        let next_status = (current & STATUS_MASK) & !(written_bits & STATUS_MASK);
        let next = (merged & ENABLE_MASK) | next_status;

        pe.sr = <Self as MmioRegister>::from_raw(next);
    }
}

crate::mmio_register! {
    Token: u16 @ 0xCC00100E => Pe.token {}
}
