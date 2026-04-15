pub const NOP_CMD: u8 = 0x00;
pub const CP_CMD: u8 = 0x08;
pub const XF_CMD: u8 = 0x10;
pub const CALL_DL_CMD: u8 = 0x40;
pub const INV_VTX_CACHE_CMD: u8 = 0x48;
pub const LOAD_INDX_A_CMD: u8 = 0x20;
pub const LOAD_INDX_B_CMD: u8 = 0x28;
pub const LOAD_INDX_C_CMD: u8 = 0x30;
pub const LOAD_INDX_D_CMD: u8 = 0x38;
pub const BP_CMD: u8 = 0x61;

pub const DRAW_COMMANDS_START: u8 = 0x80;
pub const DRAW_COMMANDS_END: u8 = 0xBF; // TODO: Double check this

pub const DRAW_QUADS_CMD: u8 = 0x80;
pub const DRAW_TRIANGLES_CMD: u8 = 0x90;
pub const DRAW_TRIANGLE_STRIP_CMD: u8 = 0x98;
pub const DRAW_TRIANGLE_FAN_CMD: u8 = 0xA0;
pub const DRAW_LINES_CMD: u8 = 0xA8;
pub const DRAW_LINE_STRIP_CMD: u8 = 0xB0;
pub const DRAW_POINTS_CMD: u8 = 0xB8;

pub const BP_REG_SIZE: usize = 0x100;
pub const CP_REG_SIZE: usize = 0xc0;
pub const XF_MEM_SIZE: usize = 0x1058;

pub const VCD_LO_REG: usize = 0x50;
pub const VCD_HI_REG: usize = 0x60;
pub const VATA_REG: usize = 0x70;
pub const VATB_REG: usize = 0x80;
pub const VATC_REG: usize = 0x90;
pub const ARRAY_BASE_REG: usize = 0xA0;
pub const ARRAY_STRIDE_REG: usize = 0xB0;

pub const ARRAY_POS: usize = 0;
pub const ARRAY_NRM: usize = 1;
pub const ARRAY_CLR0: usize = 2;
pub const ARRAY_CLR1: usize = 3;
pub const ARRAY_TEX0: usize = 4;
pub const ARRAY_POS_NRM_MTX: usize = 12; // INDX A
pub const ARRAY_NRM_MTX: usize = 13; // INDX B
pub const ARRAY_POST_MTX: usize = 14; // INDX C
pub const ARRAY_LIGHT: usize = 15; // INDX D

// XF memory addresses
pub const XF_MODELVIEW_BASE: usize = 0x0000;
pub const XF_MODELVIEW_END: usize = 0x000B;
pub const XF_PROJECTION_BASE: usize = 0x1020;
pub const XF_PROJECTION_END: usize = 0x1026;
pub const XF_MATRIX_INDEX_A: usize = 0x1018;
pub const XF_MATRIX_INDEX_B: usize = 0x1019;
pub const XF_POS_MTX_STRIDE: usize = 4;

// XF normal matrix (3x3)
pub const XF_NRM_MTX_BASE: usize = 0x0400;

// XF light objects
pub const XF_LIGHT_BASE: usize = 0x0600;
pub const XF_LIGHT_STRIDE: usize = 0x10;
pub const XF_LIGHT_COLOR: usize = 3;
pub const XF_LIGHT_A0: usize = 4;
pub const XF_LIGHT_A1: usize = 5;
pub const XF_LIGHT_A2: usize = 6;
pub const XF_LIGHT_K0: usize = 7;
pub const XF_LIGHT_K1: usize = 8;
pub const XF_LIGHT_K2: usize = 9;
pub const XF_LIGHT_PX: usize = 10;
pub const XF_LIGHT_PY: usize = 11;
pub const XF_LIGHT_PZ: usize = 12;
pub const XF_LIGHT_NX: usize = 13;
pub const XF_LIGHT_NY: usize = 14;
pub const XF_LIGHT_NZ: usize = 15;

// XF channel configuration
pub const XF_AMBIENT_COLOR0: usize = 0x100A;
pub const XF_AMBIENT_COLOR1: usize = 0x100B;
pub const XF_MATERIAL_COLOR0: usize = 0x100C;
pub const XF_MATERIAL_COLOR1: usize = 0x100D;
pub const XF_COLOR_CTRL0: usize = 0x100E; // COLOR0 channel control
pub const XF_COLOR_CTRL1: usize = 0x100F; // COLOR1 channel control
pub const XF_ALPHA_CTRL0: usize = 0x1010; // ALPHA0 channel control
pub const XF_ALPHA_CTRL1: usize = 0x1011; // ALPHA1 channel control

// XF texture coordinate generation
pub const XF_TEXGEN_BASE: usize = 0x1040; // 0x1040-0x1047
pub const XF_DUALTEX_BASE: usize = 0x1050; // 0x1050-0x1057
pub const XF_NUM_TEXGENS: usize = 0x103F;
pub const XF_DUAL_TEX_ENABLE: usize = 0x1012; // TODO: only bit 0 used?
pub const XF_TEX_MTX_BASE: usize = 0x0078; // texture transform matrices (each 2x4 or 3x4)
pub const XF_POST_MTX_BASE: usize = 0x0500; // post-transform matrices (3x4, stride 4x3)

// BP texture register base addresses (maps 0-3: base, maps 4-7: base + 0x20)
pub const BP_TX_SETMODE0_I0: usize = 0x80; // TX_SETMODE0 maps 0-3
pub const BP_TX_SETMODE1_I0: usize = 0x84; // TX_SETMODE1 maps 0-3
pub const BP_TX_SETIMAGE0_I0: usize = 0x88; // TX_SETIMAGE0 maps 0-3 (width/height/format)
pub const BP_TX_SETIMAGE1_I0: usize = 0x8C; // TX_SETIMAGE1 maps 0-3
pub const BP_TX_SETIMAGE2_I0: usize = 0x90; // TX_SETIMAGE2 maps 0-3
pub const BP_TX_SETIMAGE3_I0: usize = 0x94; // TX_SETIMAGE3 maps 0-3 (IMAGE_BASE = addr >> 5)
pub const BP_TX_SETMODE0_I4: usize = 0xA0; // TX_SETMODE0 maps 4-7
pub const BP_TX_SETMODE1_I4: usize = 0xA4; // TX_SETMODE1 maps 4-7
pub const BP_TX_SETIMAGE0_I4: usize = 0xA8; // TX_SETIMAGE0 maps 4-7
pub const BP_TX_SETIMAGE1_I4: usize = 0xAC; // TX_SETIMAGE1 maps 4-7
pub const BP_TX_SETIMAGE2_I4: usize = 0xB0; // TX_SETIMAGE2 maps 4-7
pub const BP_TX_SETIMAGE3_I4: usize = 0xB4; // TX_SETIMAGE3 maps 4-7 (IMAGE_BASE = addr >> 5)

// BP PE (Pixel Engine) registers
pub const BP_PE_ZMODE: usize = 0x40;
pub const BP_PE_CMODE0: usize = 0x41; // blend mode
pub const BP_PE_ZCOMPARE: usize = 0x43;
pub const BP_PE_ALPHA_COMPARE: usize = 0xF3;
pub const BP_PE_DONE: usize = 0x45;
pub const BP_PE_DONE_FINISH_BIT: u32 = 0x02;
pub const BP_PE_TOKEN: usize = 0x47;
pub const BP_PE_TOKEN_INT: usize = 0x48;

// BP TEV (Texture Environment) registers
pub const BP_GEN_MODE: usize = 0x00;
pub const BP_RAS1_TREF0: usize = 0x28; // ..0x2F, order registers
pub const BP_RAS1_TREF_COUNT: usize = 8;
pub const BP_TEV_COLOR_ENV_0: usize = 0xC0; // stage N color = 0xC0 + N*2
pub const BP_TEV_ALPHA_ENV_0: usize = 0xC1; // stage N alpha = 0xC1 + N*2
// lo = 0xE0 + N*2 (R,A), hi = 0xE1 + N*2 (G,B)
pub const BP_TEV_REGISTERL_0: usize = 0xE0;
pub const BP_TEV_REGISTERH_0: usize = 0xE1;

// BP TEV KSEL (Konst Selection) registers
pub const BP_TEV_KSEL_0: usize = 0xF6; // 0xF6-0xFD, 8 registers covering 16 stages

// XF viewport registers
pub const XF_VIEWPORT_SCALE_X: usize = 0x101A;
pub const XF_VIEWPORT_SCALE_Y: usize = 0x101B;
pub const XF_VIEWPORT_SCALE_Z: usize = 0x101C;
pub const XF_VIEWPORT_OFFSET_X: usize = 0x101D;
pub const XF_VIEWPORT_OFFSET_Y: usize = 0x101E;
pub const XF_VIEWPORT_OFFSET_Z: usize = 0x101F;
pub const XF_VIEWPORT_BASE: usize = 0x101A;
pub const XF_VIEWPORT_END: usize = 0x101F;

// BP scissor registers
pub const BP_SU_SCIS_TL: usize = 0x20;
pub const BP_SU_SCIS_BR: usize = 0x21;
pub const BP_SU_SCIS_OFFSET: usize = 0x59;

// BP EFB copy registers
pub const BP_PE_COPY_SRC: usize = 0x49;
pub const BP_PE_COPY_DIMS: usize = 0x4A;
pub const BP_PE_COPY_DST: usize = 0x4B;
pub const BP_PE_COPY_DST_STRIDE: usize = 0x4D;
pub const BP_PE_COPY_YSCALE: usize = 0x4E;
pub const BP_PE_COPY_CLEAR_AR: usize = 0x4F;
pub const BP_PE_COPY_CLEAR_GB: usize = 0x50;
pub const BP_PE_COPY_CLEAR_Z: usize = 0x51;
pub const BP_PE_COPY_CMD: usize = 0x52;

// EFB dimensions
pub const EFB_WIDTH: u32 = 640;
pub const EFB_HEIGHT: u32 = 528;

// Depth max for 24-bit Z
pub const DEPTH_24_BIT_MAX: f32 = 16777215.0; // (1 << 24) - 1
