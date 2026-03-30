use chapa::BitEnum;

pub struct DspStack<const N: usize> {
    data: [u16; N],
    ptr: u8,
}

impl<const N: usize> Default for DspStack<N> {
    fn default() -> Self {
        Self { data: [0; N], ptr: 0 }
    }
}

impl<const N: usize> DspStack<N> {
    #[inline(always)]
    pub fn top(&self) -> u16 {
        self.data[self.ptr as usize]
    }

    #[inline(always)]
    pub fn set_top(&mut self, value: u16) {
        self.data[self.ptr as usize] = value;
    }

    #[inline(always)]
    pub fn push(&mut self, value: u16) {
        self.ptr += 1;
        self.data[self.ptr as usize] = value;
    }

    #[inline(always)]
    pub fn pop(&mut self) -> u16 {
        let value = self.data[self.ptr as usize];
        self.ptr -= 1;
        value
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.ptr == 0
    }
}

#[derive(Default)]
pub struct Registers {
    pub pc: u16,
    pub nia: u16,
    pub cia: u16,
    pub ar: [u16; 4],
    pub ix: [u16; 4],
    pub wr: [u16; 4],
    pub call_stack: DspStack<8>,   // st0
    pub data_stack: DspStack<4>,   // st1
    pub loop_addr: DspStack<4>,    // st2
    pub loop_counter: DspStack<4>, // st3
    pub ac0_high: u16,
    pub ac1_high: u16,
    pub config: u16,
    pub status: StatusRegister,
    pub product_low: u16,
    pub product_mid1: u16,
    pub product_high: u16,
    pub product_mid2: u16,
    pub ax: [u16; 2],
    pub axh: [u16; 2],
    pub ac0_low: u16,
    pub ac1_low: u16,
    pub ac0_mid: u16,
    pub ac1_mid: u16,
}

impl Registers {
    #[inline(always)]
    pub fn sign_extended(&self) -> bool {
        self.status.sxm() == SignExtensionMode::Bits40
    }

    #[inline(always)]
    pub fn read<const ALLOW_SATURATION: bool>(&self, index: u8) -> u16 {
        match index {
            0..=3 => self.ar[index as usize],
            4..=7 => self.ix[(index - 4) as usize],
            8..=11 => self.wr[(index - 8) as usize],
            12 => self.call_stack.top(),
            13 => self.data_stack.top(),
            14 => self.loop_addr.top(),
            15 => self.loop_counter.top(),
            16 => self.ac0_high,
            17 => self.ac1_high,
            18 => self.config,
            19 => self.status.into(),
            20 => self.product_low,
            21 => self.product_mid1,
            22 => self.product_high,
            23 => self.product_mid2,
            24..=25 => self.ax[(index - 24) as usize],
            26..=27 => self.axh[(index - 26) as usize],
            28 => self.ac0_low,
            29 => self.ac1_low,
            30 => {
                if ALLOW_SATURATION && self.sign_extended() {
                    return self.saturate_ac_mid(self.ac0_high, self.ac0_mid);
                }
                self.ac0_mid
            }
            31 => {
                if ALLOW_SATURATION && self.sign_extended() {
                    return self.saturate_ac_mid(self.ac1_high, self.ac1_mid);
                }
                self.ac1_mid
            }
            _ => unreachable!(),
        }
    }

    /// Saturate $acX.m: if $acX.h is not the sign extension of $acX.m,
    /// return 0x7FFF (positive) or 0x8000 (negative).
    #[inline(always)]
    fn saturate_ac_mid(&self, high: u16, mid: u16) -> u16 {
        let sign_ext = if mid & 0x8000 != 0 { 0x00FF } else { 0 };
        if high != sign_ext {
            if high & 0x80 != 0 { 0x8000 } else { 0x7FFF }
        } else {
            mid
        }
    }

    #[inline(always)]
    pub fn write<const ALLOW_SIGN_EXTENSION: bool>(&mut self, index: u8, value: u16) {
        match index {
            0..=3 => self.ar[index as usize] = value,
            4..=7 => self.ix[(index - 4) as usize] = value,
            8..=11 => self.wr[(index - 8) as usize] = value,
            12 => self.call_stack.set_top(value),
            13 => self.data_stack.set_top(value),
            14 => self.loop_addr.set_top(value),
            15 => self.loop_counter.set_top(value),
            16 => self.ac0_high = value,
            17 => self.ac1_high = value,
            18 => self.config = value,
            19 => self.status = StatusRegister::from(value),
            20 => self.product_low = value,
            21 => self.product_mid1 = value,
            22 => self.product_high = value,
            23 => self.product_mid2 = value,
            24..=25 => self.ax[(index - 24) as usize] = value,
            26..=27 => self.axh[(index - 26) as usize] = value,
            28 => self.ac0_low = value,
            29 => self.ac1_low = value,
            30 => {
                self.ac0_mid = value;
                if ALLOW_SIGN_EXTENSION && self.sign_extended() {
                    self.ac0_high = if value & 0x8000 != 0 { 0x00FF } else { 0 };
                    self.ac0_low = 0;
                }
            }
            31 => {
                self.ac1_mid = value;
                if ALLOW_SIGN_EXTENSION && self.sign_extended() {
                    self.ac1_high = if value & 0x8000 != 0 { 0x00FF } else { 0 };
                    self.ac1_low = 0;
                }
            }
            _ => unreachable!(),
        }
    }
}

#[derive(BitEnum, PartialEq, PartialOrd)]
pub enum SignExtensionMode {
    Bits16,
    Bits40,
}

#[chapa::bitfield(u16, order = lsb0)]
#[derive(Clone, Copy, Default)]
pub struct StatusRegister {
    #[bits(0, alias = "c")]
    pub carry: bool,
    #[bits(1, alias = "o")]
    pub overflow: bool,
    #[bits(2, alias = "z")]
    pub arithmetic_zero: bool,
    #[bits(3, alias = "s")]
    pub sign: bool,
    #[bits(4, alias = "as32")]
    pub above_s32: bool,
    #[bits(5, alias = "tb")]
    pub top_two_bits_equal: bool,
    #[bits(6, alias = "lz")]
    pub logical_zero: bool,
    #[bits(7, alias = "os")]
    pub overflow_sticky: bool,
    #[bits(9, alias = "ie")]
    pub interrupt_enable: bool,
    #[bits(11, alias = "eie")]
    pub external_interrupt_enable: bool,
    #[bits(13, alias = "am")]
    pub product_multiply_result_by_2: bool, // when AM = 0
    #[bits(14, alias = "sxm")]
    pub sign_extension_mode: SignExtensionMode,
    #[bits(15, alias = "su")]
    pub multiplication_operands_are_signed: bool,
}
