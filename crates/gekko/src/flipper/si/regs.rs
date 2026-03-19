use super::Si;
use crate::mmio::traits::MmioAccess;
use chapa::BitEnum;

// 0xCC006430  4  R/W  SIPOLL - SI Poll Register
crate::mmio_register! {
    SiPoll: u32 @ 0xCC006430 => Si.poll {
        #[bits(0..=3)]
        pub vbcpy: u8,

        #[bits(4..=7)]
        pub enable: u8,

        #[bits(8..=15)]
        pub y_times: u8,

        #[bits(16..=25)]
        pub x_lines: u16,
    }
}

// 0xCC006434  4  R/W  SICOMCSR - SI Communication Control Status Register

#[derive(BitEnum, Debug, PartialEq, Eq)]
pub enum Channel {
    Channel0 = 0,
    Channel1 = 1,
    Channel2 = 2,
    Channel3 = 3,
}

crate::mmio_register! {
    SiComcsr: u32 @ 0xCC006434 {
        #[bits(0)]
        pub tstart: bool,

        #[bits(1..=2)]
        pub channel: Channel,

        #[bits(6)]
        pub callback_enable: bool,

        #[bits(7)]
        pub command_enable: bool,

        #[bits(8..=14)]
        pub in_length: u8,

        #[bits(16..=22)]
        pub out_length: u8,

        #[bits(24)]
        pub channel_enable: bool,

        #[bits(25..=26)]
        pub channel_number: u8,

        #[bits(27)]
        pub rdst_interrupt_mask: bool,

        #[bits(28)]
        pub rdst_interrupt: bool,

        #[bits(29)]
        pub com_error: bool,

        #[bits(30)]
        pub tc_interrupt_mask: bool,

        #[bits(31)]
        pub tc_interrupt: bool,
    }
}

impl MmioAccess<Si> for SiComcsr {
    fn read(si: &Si) -> Self {
        si.comcsr
    }

    fn write(self, si: &mut Si) {
        let mut csr = si.comcsr;

        if self.tc_interrupt() {
            csr = csr.with_tc_interrupt(false);
        }

        if self.rdst_interrupt() {
            csr = csr.with_rdst_interrupt(false);
        }

        csr = csr
            .with_tc_interrupt_mask(self.tc_interrupt_mask())
            .with_rdst_interrupt_mask(self.rdst_interrupt_mask())
            .with_command_enable(self.command_enable())
            .with_callback_enable(self.callback_enable())
            .with_channel(self.channel())
            .with_in_length(self.in_length())
            .with_out_length(self.out_length())
            .with_channel_enable(self.channel_enable())
            .with_channel_number(self.channel_number())
            .with_tstart(self.tstart());

        si.comcsr = csr;
    }
}

// 0xCC006438  4  R/W  SISR - SI Status Register

crate::mmio_register! {
    SiStatusRegister: u32 @ 0xCC006438 => Si.status {}
}

// 0xCC00643C  4  R/W  SI Channel 0 Input Buffer High

crate::mmio_register! {
    SiChannel0InBufHi: u32 @ 0xCC00643C => Si.ch0_in_buf_hi {}
}

// 0xCC006480  4  R/W  SI I/O Buffer

crate::mmio_register! {
    SiIoBuf: u32 @ 0xCC006480 => Si.io_buf {}
}
