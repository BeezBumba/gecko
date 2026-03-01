#[cfg(test)]
mod tests {
    use crate::mmio::Mmio;

    #[test]
    fn ram_phys_read_write_roundtrip() {
        let mut mmio = Mmio::new();
        mmio.phys_write_u32(0x100, 0xDEAD_BEEF);
        assert_eq!(mmio.phys_read_u32(0x100), 0xDEAD_BEEF);
        assert_eq!(mmio.phys_read_u16(0x100), 0xDEAD);
        assert_eq!(mmio.phys_read_u8(0x100), 0xDE);
    }

    #[test]
    fn cached_virtual_maps_to_physical() {
        let mut mmio = Mmio::new();
        mmio.virt_write_u32(0x8000_0100, 0x1234_5678);
        assert_eq!(mmio.phys_read_u32(0x100), 0x1234_5678);
    }

    #[test]
    fn uncached_virtual_maps_to_physical() {
        let mut mmio = Mmio::new();
        mmio.virt_write_u32(0xC000_0100, 0xAAAA_BBBB);
        assert_eq!(mmio.phys_read_u32(0x100), 0xAAAA_BBBB);
    }

    #[test]
    fn efb_virtual_maps_to_efb_storage() {
        let mut mmio = Mmio::new();
        // 0xC8000000 virtual -> phys 0x08000000 (EFB)
        mmio.virt_write_u32(0xC800_0000, 0xFB_FB_FB_FB);
        assert_eq!(mmio.phys_read_u32(0x0800_0000), 0xFB_FB_FB_FB);
        // Confirm it did NOT touch RAM
        assert_eq!(mmio.phys_read_u32(0x0), 0);
    }

    #[test]
    fn efb_address_does_not_alias_ram() {
        let mut mmio = Mmio::new();
        mmio.phys_write_u32(0x0, 0x1111_1111);
        mmio.phys_write_u32(0x0800_0000, 0x2222_2222);
        assert_eq!(mmio.phys_read_u32(0x0), 0x1111_1111);
        assert_eq!(mmio.phys_read_u32(0x0800_0000), 0x2222_2222);
    }

    #[test]
    fn unmapped_address_returns_fallback() {
        let mmio = Mmio::new();
        let _ = mmio.phys_read_u8(0x1000_0000);
    }

    #[test]
    fn hw_register_range_reads_from_hwr_fallback() {
        // HW reg reads go to the hwr fallback Vec; default value is 0
        let mmio = Mmio::new();
        assert_eq!(mmio.phys_read_u8(0x0C00_0000), 0);
    }

    #[test]
    fn phys_slice_returns_correct_data() {
        let mut mmio = Mmio::new();
        mmio.phys_write_u32(0x200, 0xCAFE_BABE);
        let slice = mmio.phys_slice(0x200, 4);
        assert_eq!(slice, &[0xCA, 0xFE, 0xBA, 0xBE]);
    }
}
