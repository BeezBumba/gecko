use super::VideoInterface;
use crate::mmio::traits::{MmioAccess, MmioRegister};
use chapa::BitEnum;

// 0xCC002000	2	R/W	VTR - Vertical Timing Register

crate::mmio_register! {
    VerticalTiming: u16 @ 0xCC002000 => VideoInterface.vtr {
        #[bits(0..=3, alias = "equ")] pub equalization_pulse: u8,
        #[bits(4..=13, alias = "acv")] pub active_video: u16,
    }
}

// 0xCC002002	2	R/W	DCR - Display Configuration Register

pub enum RefreshRate {
    Hz60,
    Hz50,
}

#[derive(Debug, BitEnum)]
pub enum VideoFormat {
    Ntsc = 0,
    Pal = 1,
    Mpal = 2,
    Debug = 3,
}

impl VideoFormat {
    pub fn refresh_rate(&self) -> RefreshRate {
        match self {
            VideoFormat::Ntsc | VideoFormat::Mpal => RefreshRate::Hz60,
            VideoFormat::Pal | VideoFormat::Debug => RefreshRate::Hz50,
        }
    }

    pub fn lines(&self) -> usize {
        match self {
            VideoFormat::Ntsc | VideoFormat::Mpal => 480,
            VideoFormat::Pal | VideoFormat::Debug => 574,
        }
    }

    pub fn columns(&self) -> usize {
        640
    }
}

crate::mmio_register! {
    DisplayConfiguration: u16 @ 0xCC002002 {
        #[bits(0, alias = "enb")] pub enable: bool,
        #[bits(1, alias = "rst")] pub reset: bool,
        #[bits(2, alias = "nin")] pub interlace_selector: bool,
        #[bits(3, alias = "dlr")] pub display_mode_3d: bool,
        #[bits(4..=5, alias = "le0")] pub display_latch0: u8,
        #[bits(6..=7, alias = "le1")] pub display_latch1: u8,
        #[bits(8..=9, alias = "fmt")] pub video_format: VideoFormat,
    }
}

impl DisplayConfiguration {
    pub fn interlaced(&self) -> bool {
        !self.nin()
    }
}

impl MmioAccess<VideoInterface> for DisplayConfiguration {
    fn read(vi: &VideoInterface) -> Self {
        vi.dcr
    }

    fn write(self, vi: &mut VideoInterface) {
        // TODO: Rising-edge on RST clears the register? Just to test for now
        if self.reset() && !vi.dcr.reset() {
            vi.dcr = <DisplayConfiguration as MmioRegister>::from_raw(0);
        } else {
            vi.dcr = self;
        }
    }
}

// 0xCC002004	4	R/W	HTR0 - Horizontal Timing 0

crate::mmio_register! {
    HorizontalTiming0: u32 @ 0xCC002004 => VideoInterface.htr0 {
        #[bits(0..=8, alias = "hlw")] pub halfline_width: u16,
        #[bits(16..=22, alias = "hce")] pub horizontal_sync_end: u8,
        #[bits(24..=30, alias = "hcs")] pub horizontal_sync_start: u8,
    }
}

// 0xCC002008	4	R/W	HTR1 - Horizontal Timing 1

crate::mmio_register! {
    HorizontalTiming1: u32 @ 0xCC002008 => VideoInterface.htr1 {
        #[bits(0..=6, alias = "hsy")] pub horizontal_sync_width: u8,
        #[bits(7..=16, alias = "hbe")] pub horizontal_blank_end: u16,
        #[bits(17..=26, alias = "hbs")] pub horizontal_blank_start: u16,
    }
}

// 0xCC00200C	4	R/W	VTO - Odd Field Vertical Timing Register

crate::mmio_register! {
    VerticalTimingOdd: u32 @ 0xCC00200C => VideoInterface.vto {
        #[bits(0..=9, alias = "prb")] pub pre_blanking_in_half_lines: u16,
        #[bits(16..=25, alias = "psb")] pub post_blanking_in_half_lines: u16,
    }
}

// 0xCC002010	4	R/W	VTE - Even Field Vertical Timing Register

crate::mmio_register! {
    VerticalTimingEven: u32 @ 0xCC002010 => VideoInterface.vte {
        #[bits(0..=9, alias = "prb")] pub pre_blanking_in_half_lines: u16,
        #[bits(16..=25, alias = "psb")] pub post_blanking_in_half_lines: u16,
    }
}

// 0xCC002014	4	R/W	BBEI - Burst Blanking Even Interval

crate::mmio_register! {
    BurstBlankingEvenInterval: u32 @ 0xCC002014 => VideoInterface.bbei {
        #[bits(0..=4, alias = "bs1")] pub burst_start_1: u8,
        #[bits(5..=15, alias = "be1")] pub burst_end_1: u16,
        #[bits(16..=20, alias = "bs3")] pub burst_start_3: u8,
        #[bits(21..=31, alias = "be3")] pub burst_end_3: u16,
    }
}

// 0xCC002018	4	R/W	BBOI - Burst Blanking Odd Interval

crate::mmio_register! {
    BurstBlankingOddInterval: u32 @ 0xCC002018 => VideoInterface.bboi {
        #[bits(0..=4, alias = "bs2")] pub burst_start_2: u8,
        #[bits(5..=15, alias = "be2")] pub burst_end_2: u16,
        #[bits(16..=20, alias = "bs4")] pub burst_start_4: u8,
        #[bits(21..=31, alias = "be4")] pub burst_end_4: u16,
    }
}

// 0xCC00201c	4	R/W	TFBL - Top Field Base Register (L) (External Framebuffer Half 1)

crate::mmio_register! {
    TopFieldBase: u32 @ 0xCC00201C => VideoInterface.tfbl {
        #[bits(0..=23, alias = "fbb")]
        pub xfb_addr: u32,

        #[bits(24..=27, alias = "xof")]
        pub horizontal_offset: u8,

        #[bits(28)]
        pub page_offset: bool,
        // TODO: 29-31	y	always zero (maybe some write only control register stuff?, setting bit 31 clears bits 31-28 (?))
    }
}

// 0xCC002020	4	R/W	TFBR - Top Field Base Register (R)

crate::mmio_register! {
    TopFieldBaseRight: u32 @ 0xCC002020 => VideoInterface.tfbr {
        #[bits(0..=23, alias = "fbb")] pub xfb_addr: u32,
    }
}

// 0xCC002024	4	R/W	BFBL - Bottom Field Base Register (L) (External Framebuffer Half 2)

crate::mmio_register! {
    BottomFieldBase: u32 @ 0xCC002024 => VideoInterface.bfbl {
        #[bits(0..=23, alias = "fbb")]
        pub xfb_addr: u32,

        #[bits(28)]
        pub page_offset: bool,
        // TODO:  	y	always zero (maybe some write-only control register stuff?)
    }
}

// 0xCC002028	4	R/W	BFBR - Bottom Field Base Register (R)

crate::mmio_register! {
    BottomFieldBaseRight: u32 @ 0xCC002028 => VideoInterface.bfbr {
        #[bits(0..=23, alias = "fbb")] pub xfb_addr: u32,
    }
}

// 0xCC00202C	2	R	DPV - Display Position Vertical

crate::mmio_register! {
    DisplayPositionVertical: u16 @ 0xCC00202C => VideoInterface.dpv {
        #[bits(0..=10, alias = "vct")] pub vertical_count: u16,
    }
}

// 0xCC00202E	2	R	DPH - Display Position Horizontal

crate::mmio_register! {
    DisplayPositionHorizontal: u16 @ 0xCC00202E => VideoInterface.dph {
        #[bits(0..=10, alias = "hct")] pub horizontal_count: u16,
    }
}

// 0xCC002030	4	R/W	DI0 - Display Interrupt 0
// THESE ARE NOT RESET ON WRITE 1!!

crate::mmio_register! {
    DisplayInterrupt0: u32 @ 0xCC002030 => VideoInterface.di0 {
        #[bits(0..=9, alias = "hct")] pub horizontal_count: u16,
        #[bits(16..=25, alias = "vct")] pub vertical_count: u16,
        #[bits(28, alias = "enb")] pub enable: bool,
        #[bits(31, alias = "int")] pub interrupt: bool,
    }
}

// 0xCC002034	4	R/W	DI1 - Display Interrupt 1

crate::mmio_register! {
    DisplayInterrupt1: u32 @ 0xCC002034 => VideoInterface.di1 {
        #[bits(0..=9, alias = "hct")] pub horizontal_count: u16,
        #[bits(16..=25, alias = "vct")] pub vertical_count: u16,
        #[bits(28, alias = "enb")] pub enable: bool,
        #[bits(31, alias = "int")] pub interrupt: bool,
    }
}

// 0xCC002038	4	R/W	DI2 - Display Interrupt 2

crate::mmio_register! {
    DisplayInterrupt2: u32 @ 0xCC002038 => VideoInterface.di2 {
        #[bits(0..=9, alias = "hct")] pub horizontal_count: u16,
        #[bits(16..=25, alias = "vct")] pub vertical_count: u16,
        #[bits(28, alias = "enb")] pub enable: bool,
        #[bits(31, alias = "int")] pub interrupt: bool,
    }
}

// 0xCC00203C	4	R/W	DI3 - Display Interrupt 3

crate::mmio_register! {
    DisplayInterrupt3: u32 @ 0xCC00203C => VideoInterface.di3 {
        #[bits(0..=9, alias = "hct")] pub horizontal_count: u16,
        #[bits(16..=25, alias = "vct")] pub vertical_count: u16,
        #[bits(28, alias = "enb")] pub enable: bool,
        #[bits(31, alias = "int")] pub interrupt: bool,
    }
}

// 0xCC002040	4	R/W	DL0 - Display Latch 0

crate::mmio_register! {
    DisplayLatch0: u32 @ 0xCC002040 => VideoInterface.dl0 {
        #[bits(0..=10, alias = "hct")] pub horizontal_count: u16,
        #[bits(16..=26, alias = "vct")] pub vertical_count: u16,
        #[bits(31, alias = "trg")] pub trigger: bool,
    }
}

// 0xCC002044	4	R/W	DL1 - Display Latch 1

crate::mmio_register! {
    DisplayLatch1: u32 @ 0xCC002044 => VideoInterface.dl1 {
        #[bits(0..=10, alias = "hct")] pub horizontal_count: u16,
        #[bits(16..=26, alias = "vct")] pub vertical_count: u16,
        #[bits(31, alias = "trg")] pub trigger: bool,
    }
}

// 0xCC002048	2	R/W	HSW - Horizontal Scaling Width

crate::mmio_register! {
    HorizontalScalingWidth: u16 @ 0xCC002048 => VideoInterface.hsw {
        #[bits(0..=9, alias = "srcwidth")] pub source_width: u16,
    }
}

// 0xCC00204A	2	R/W	HSR - Horizontal Scaling Register

crate::mmio_register! {
    HorizontalScalingRegister: u16 @ 0xCC00204A => VideoInterface.hsr {
        #[bits(0..=8, alias = "stp")] pub step_size: u16,
        #[bits(12, alias = "hs_en")] pub horizontal_scaling_enable: bool,
    }
}

// 0xCC00204C	4	R/W	FCT0 - Filter Coefficient Table 0

crate::mmio_register! {
    FilterCoefficient0: u32 @ 0xCC00204C => VideoInterface.fct0 {
        #[bits(0..=9, alias = "t0")] pub tap0: u16,
        #[bits(10..=19, alias = "t1")] pub tap1: u16,
        #[bits(20..=29, alias = "t2")] pub tap2: u16,
    }
}

// 0xCC002050	4	R/W	FCT1 - Filter Coefficient Table 1

crate::mmio_register! {
    FilterCoefficient1: u32 @ 0xCC002050 => VideoInterface.fct1 {
        #[bits(0..=9, alias = "t3")] pub tap3: u16,
        #[bits(10..=19, alias = "t4")] pub tap4: u16,
        #[bits(20..=29, alias = "t5")] pub tap5: u16,
    }
}

// 0xCC002054	4	R/W	FCT2 - Filter Coefficient Table 2

crate::mmio_register! {
    FilterCoefficient2: u32 @ 0xCC002054 => VideoInterface.fct2 {
        #[bits(0..=9, alias = "t6")] pub tap6: u16,
        #[bits(10..=19, alias = "t7")] pub tap7: u16,
        #[bits(20..=29, alias = "t8")] pub tap8: u16,
    }
}

// 0xCC002058	4	R/W	FCT3 - Filter Coefficient Table 3

crate::mmio_register! {
    FilterCoefficient3: u32 @ 0xCC002058 => VideoInterface.fct3 {
        #[bits(0..=7, alias = "t9")] pub tap9: u8,
        #[bits(8..=15, alias = "t10")] pub tap10: u8,
        #[bits(16..=23, alias = "t11")] pub tap11: u8,
        #[bits(24..=31, alias = "t12")] pub tap12: u8,
    }
}

// 0xCC00205C	4	R/W	FCT4 - Filter Coefficient Table 4

crate::mmio_register! {
    FilterCoefficient4: u32 @ 0xCC00205C => VideoInterface.fct4 {
        #[bits(0..=7, alias = "t13")] pub tap13: u8,
        #[bits(8..=15, alias = "t14")] pub tap14: u8,
        #[bits(16..=23, alias = "t15")] pub tap15: u8,
        #[bits(24..=31, alias = "t16")] pub tap16: u8,
    }
}

// 0xCC002060	4	R/W	FCT5 - Filter Coefficient Table 5

crate::mmio_register! {
    FilterCoefficient5: u32 @ 0xCC002060 => VideoInterface.fct5 {
        #[bits(0..=7, alias = "t17")] pub tap17: u8,
        #[bits(8..=15, alias = "t18")] pub tap18: u8,
        #[bits(16..=23, alias = "t19")] pub tap19: u8,
        #[bits(24..=31, alias = "t20")] pub tap20: u8,
    }
}

// 0xCC002064	4	R/W	FCT6 - Filter Coefficient Table 6

crate::mmio_register! {
    FilterCoefficient6: u32 @ 0xCC002064 => VideoInterface.fct6 {
        #[bits(0..=7, alias = "t21")] pub tap21: u8,
        #[bits(8..=15, alias = "t22")] pub tap22: u8,
        #[bits(16..=23, alias = "t23")] pub tap23: u8,
        #[bits(24..=31, alias = "t24")] pub tap24: u8,
    }
}

// 0xCC00206C	2	R/W	VICLK - VI Clock Select

crate::mmio_register! {
    ViClockSelect: u16 @ 0xCC00206C => VideoInterface.viclk {
        #[bits(0, alias = "clk")] pub clock_select: bool,
    }
}

// 0xCC00206E	2	R/W	VISEL - VI DTV Status

crate::mmio_register! {
    ViDtvStatus: u16 @ 0xCC00206E => VideoInterface.visel {
        #[bits(2, alias = "visel")] pub dtv_status: bool,
    }
}

// 0xCC002070	2	R/W	God knows what this is, log for now

crate::mmio_register! {
    ViUnknown70: u16 @ 0xCC002070 => VideoInterface.unknown_70 {}
}

// 0xCC002072	2	R/W	BorderHBE - Border HBE

crate::mmio_register! {
    BorderHbe: u16 @ 0xCC002072 => VideoInterface.border_hbe {
        #[bits(0..=9, alias = "hbe656")] pub horizontal_blank_end_656: u16,
        #[bits(15, alias = "brdr_en")] pub border_enable: bool,
    }
}

// 0xCC002074	2	R/W	BorderHBS - Border HBS

crate::mmio_register! {
    BorderHbs: u16 @ 0xCC002074 => VideoInterface.border_hbs {
        #[bits(0..=9, alias = "hbs656")] pub horizontal_blank_start_656: u16,
    }
}
