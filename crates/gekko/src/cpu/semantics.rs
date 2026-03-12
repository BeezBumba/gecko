include!(concat!(env!("OUT_DIR"), "/gekko_instr.rs"));

impl Instruction {
    #[inline]
    pub fn disp(&self) -> i32 {
        self.d_16_31()
    }

    #[inline]
    pub fn disp_psq(&self) -> i32 {
        self.d_20_31()
    }

    /// The SPR field in PowerPC instructions has a special encoding where
    /// the two 5-bit halves are swapped. This method returns the decoded value
    #[inline]
    pub fn spr_swapped(&self) -> u32 {
        let raw = self.spr() as u32;
        (raw >> 5) | ((raw & 0x1f) << 5)
    }
}
