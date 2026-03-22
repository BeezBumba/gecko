use crate::{
    gekko::Gekko,
    mmio::{Mmio, constants::RAM_END},
};

impl Gekko {
    // Load a 64-bit double from memory
    #[inline]
    pub fn read_f64(&mut self, addr: u32) -> f64 {
        let phys = Mmio::virt_to_phys(addr);
        if phys <= RAM_END - 7 {
            return f64::from_bits(self.mmio.ram_read_u64(phys));
        }

        let hi = self.load_u32_data(addr) as u64;
        let lo = self.load_u32_data(addr.wrapping_add(4)) as u64;
        f64::from_bits((hi << 32) | lo)
    }

    /// Store a 64-bit double to memory
    #[inline]
    pub fn write_f64(&mut self, addr: u32, val: f64) {
        let phys = Mmio::virt_to_phys(addr);
        let bits = val.to_bits();
        if phys <= RAM_END - 7 {
            self.mmio.ram_write_u64(phys, bits);
            return;
        }

        self.store_u32_data(addr, (bits >> 32) as u32);
        self.store_u32_data(addr.wrapping_add(4), bits as u32);
    }

    /// Load a 32-bit float from memory, return as f64
    #[inline]
    pub fn read_f32(&mut self, addr: u32) -> f64 {
        f32::from_bits(self.load_u32_data(addr)) as f64
    }

    /// Store f64 as 32-bit float to memory
    #[inline]
    pub fn write_f32(&mut self, addr: u32, val: f64) {
        self.store_u32_data(addr, (val as f32).to_bits());
    }
}
