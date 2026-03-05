use crate::{
    gekko::Gekko,
    mmio::{
        Mmio,
        constants::{
            AI_BASE, AI_END, DSP_BASE, DSP_END, EXI_BASE, EXI_END, MI_BASE, MI_END, PI_BASE, PI_END, VI_BASE, VI_END,
        },
    },
};

enum BusTarget {
    Vi,
    Pi,
    Mi,
    Dsp,
    Exi,
    Ai,
    Fallback,
}

#[rustfmt::skip]
fn route(phys: u32) -> (BusTarget, u32) {
    match phys {
        VI_BASE..=VI_END   => (BusTarget::Vi,  phys - VI_BASE),
        PI_BASE..=PI_END   => (BusTarget::Pi,  phys - PI_BASE),
        MI_BASE..=MI_END   => (BusTarget::Mi,  phys - MI_BASE),
        DSP_BASE..=DSP_END => (BusTarget::Dsp, phys - DSP_BASE),
        EXI_BASE..=EXI_END => (BusTarget::Exi, phys - EXI_BASE),
        AI_BASE..=AI_END   => (BusTarget::Ai,  phys - AI_BASE),
        _                  => (BusTarget::Fallback, phys),
    }
}

impl Gekko {
    #[rustfmt::skip]
    pub fn read_u8(&mut self, addr: u32) -> u8 {
        let (target, offset) = route(Mmio::virt_to_phys(addr));
        match target {
            BusTarget::Vi       => self.vi.mmio_read_u8(offset),
            BusTarget::Pi       => self.pi.mmio_read_u8(offset),
            BusTarget::Mi       => self.mi.mmio_read_u8(offset),
            BusTarget::Dsp      => self.dsp.mmio_read_u8(offset),
            BusTarget::Exi      => self.exi.mmio_read_u8(offset),
            BusTarget::Ai       => self.ai.mmio_read_u8(offset),
            BusTarget::Fallback => self.mmio.phys_read_u8(offset),
        }
    }

    #[rustfmt::skip]
    pub fn read_u16(&mut self, addr: u32) -> u16 {
        let (target, offset) = route(Mmio::virt_to_phys(addr));
        match target {
            BusTarget::Vi       => self.vi.mmio_read_u16(offset),
            BusTarget::Pi       => self.pi.mmio_read_u16(offset),
            BusTarget::Mi       => self.mi.mmio_read_u16(offset),
            BusTarget::Dsp      => self.dsp.mmio_read_u16(offset),
            BusTarget::Exi      => self.exi.mmio_read_u16(offset),
            BusTarget::Ai       => self.ai.mmio_read_u16(offset),
            BusTarget::Fallback => self.mmio.phys_read_u16(offset),
        }
    }

    #[rustfmt::skip]
    pub fn read_u32(&mut self, addr: u32) -> u32 {
        let (target, offset) = route(Mmio::virt_to_phys(addr));
        match target {
            BusTarget::Vi       => self.vi.mmio_read_u32(offset),
            BusTarget::Pi       => self.pi.mmio_read_u32(offset),
            BusTarget::Mi       => self.mi.mmio_read_u32(offset),
            BusTarget::Dsp      => self.dsp.mmio_read_u32(offset),
            BusTarget::Exi      => self.exi.mmio_read_u32(offset),
            BusTarget::Ai       => self.ai.mmio_read_u32(offset),
            BusTarget::Fallback => self.mmio.phys_read_u32(offset),
        }
    }

    #[rustfmt::skip]
    pub fn write_u8(&mut self, addr: u32, val: u8) {
        let (target, offset) = route(Mmio::virt_to_phys(addr));
        match target {
            BusTarget::Vi       => self.vi.mmio_write_u8(offset, val),
            BusTarget::Pi       => self.pi.mmio_write_u8(offset, val),
            BusTarget::Mi       => self.mi.mmio_write_u8(offset, val),
            BusTarget::Dsp      => self.dsp.mmio_write_u8(offset, val),
            BusTarget::Exi      => self.exi.mmio_write_u8(offset, val),
            BusTarget::Ai       => self.ai.mmio_write_u8(offset, val),
            BusTarget::Fallback => self.mmio.phys_write_u8(offset, val),
        }
    }

    #[rustfmt::skip]
    pub fn write_u16(&mut self, addr: u32, val: u16) {
        let (target, offset) = route(Mmio::virt_to_phys(addr));
        match target {
            BusTarget::Vi       => self.vi.mmio_write_u16(offset, val),
            BusTarget::Pi       => self.pi.mmio_write_u16(offset, val),
            BusTarget::Mi       => self.mi.mmio_write_u16(offset, val),
            BusTarget::Dsp      => self.dsp.mmio_write_u16(offset, val),
            BusTarget::Exi      => self.exi.mmio_write_u16(offset, val),
            BusTarget::Ai       => self.ai.mmio_write_u16(offset, val),
            BusTarget::Fallback => self.mmio.phys_write_u16(offset, val),
        }
    }

    #[rustfmt::skip]
    pub fn write_u32(&mut self, addr: u32, val: u32) {
        let (target, offset) = route(Mmio::virt_to_phys(addr));
        match target {
            BusTarget::Vi       => self.vi.mmio_write_u32(offset, val),
            BusTarget::Pi       => self.pi.mmio_write_u32(offset, val),
            BusTarget::Mi       => self.mi.mmio_write_u32(offset, val),
            BusTarget::Dsp      => self.dsp.mmio_write_u32(offset, val),
            BusTarget::Exi      => self.exi.mmio_write_u32(offset, val),
            BusTarget::Ai       => self.ai.mmio_write_u32(offset, val),
            BusTarget::Fallback => self.mmio.phys_write_u32(offset, val),
        }
    }
}
