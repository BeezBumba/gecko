use crate::{
    gekko::Gekko,
    mmio::{
        Mmio,
        constants::{VI_BASE, VI_END},
    },
};

enum BusTarget {
    Vi,
    Fallback,
}

#[rustfmt::skip]
fn route(phys: u32) -> (BusTarget, u32) {
    match phys {
        VI_BASE..=VI_END => (BusTarget::Vi, phys - VI_BASE),
        _                => (BusTarget::Fallback, phys),
    }
}

impl Gekko {
    #[rustfmt::skip]
    pub fn read_u8(&mut self, addr: u32) -> u8 {
        let (target, offset) = route(Mmio::virt_to_phys(addr));
        match target {
            BusTarget::Vi       => self.vi.mmio_read_u8(offset),
            BusTarget::Fallback => self.mmio.phys_read_u8(offset),
        }
    }

    #[rustfmt::skip]
    pub fn read_u16(&mut self, addr: u32) -> u16 {
        let (target, offset) = route(Mmio::virt_to_phys(addr));
        match target {
            BusTarget::Vi       => self.vi.mmio_read_u16(offset),
            BusTarget::Fallback => self.mmio.phys_read_u16(offset),
        }
    }

    #[rustfmt::skip]
    pub fn read_u32(&mut self, addr: u32) -> u32 {
        let (target, offset) = route(Mmio::virt_to_phys(addr));
        match target {
            BusTarget::Vi       => self.vi.mmio_read_u32(offset),
            BusTarget::Fallback => self.mmio.phys_read_u32(offset),
        }
    }

    #[rustfmt::skip]
    pub fn write_u8(&mut self, addr: u32, val: u8) {
        let (target, offset) = route(Mmio::virt_to_phys(addr));
        match target {
            BusTarget::Vi       => self.vi.mmio_write_u8(offset, val),
            BusTarget::Fallback => self.mmio.phys_write_u8(offset, val),
        }
    }

    #[rustfmt::skip]
    pub fn write_u16(&mut self, addr: u32, val: u16) {
        let (target, offset) = route(Mmio::virt_to_phys(addr));
        match target {
            BusTarget::Vi       => self.vi.mmio_write_u16(offset, val),
            BusTarget::Fallback => self.mmio.phys_write_u16(offset, val),
        }
    }

    #[rustfmt::skip]
    pub fn write_u32(&mut self, addr: u32, val: u32) {
        let (target, offset) = route(Mmio::virt_to_phys(addr));
        match target {
            BusTarget::Vi       => self.vi.mmio_write_u32(offset, val),
            BusTarget::Fallback => self.mmio.phys_write_u32(offset, val),
        }
    }
}
