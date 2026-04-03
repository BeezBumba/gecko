use crate::flipper::dsp::condition::BranchControl;
use crate::flipper::dsp::core::reg;
use crate::flipper::dsp::core::regs::{SignExtensionMode, StatusRegister};
use crate::flipper::dsp::lut::*;

#[inline(always)]
fn product(regs: &crate::flipper::dsp::core::Registers) -> i64 {
    let ph = (regs.product_high as u8) as i8 as i64;
    let pm1 = regs.product_mid1 as i64;
    let pm2 = regs.product_mid2 as i64;
    let pl = regs.product_low as i64;
    (ph << 32) + ((pm1 + pm2) << 16) + pl
}

#[inline(always)]
fn write_product(regs: &mut crate::flipper::dsp::core::Registers, val: i64) {
    regs.product_high = (val >> 32) as u16;
    regs.product_mid1 = (val >> 16) as u16;
    regs.product_low = val as u16;
    regs.product_mid2 = 0;
}

#[inline(always)]
fn multiply(regs: &mut crate::flipper::dsp::core::Registers, a: i16, b: i16) {
    let mut result = a as i32 as i64 * b as i32 as i64;
    if !regs.status.am() {
        result <<= 1;
    }
    write_product(regs, result);
}

/// Compute a * b (with AM shift), then add/sub to current product.
#[inline(always)]
fn multiply_accumulate<const ADD: bool>(regs: &mut crate::flipper::dsp::core::Registers, a: i16, b: i16) {
    let mut mul_result = a as i32 as i64 * b as i32 as i64;
    if !regs.status.am() {
        mul_result <<= 1;
    }
    let prod = product(regs);
    let result = if ADD {
        prod.wrapping_add(mul_result)
    } else {
        prod.wrapping_sub(mul_result)
    };
    write_product(regs, result);
}

#[inline(always)]
fn move_prod_to_ac(ctx: &mut crate::gamecube::GameCube, r: u8) {
    let prod = product(&ctx.dsp.registers);
    ctx.dsp.registers.set_ac(r, prod);
    ctx.dsp.registers.update_flags_ac(prod);
    ctx.dsp.registers.status.set_o(false);
}

#[inline(always)]
fn move_prod_to_ac_zero(ctx: &mut crate::gamecube::GameCube, r: u8) {
    let prod = product(&ctx.dsp.registers) & !0xFFFFi64;
    ctx.dsp.registers.set_ac(r, prod);
    ctx.dsp.registers.update_flags_ac(prod);
    ctx.dsp.registers.status.set_o(false);
}

#[inline(always)]
fn add_prod_to_ac(ctx: &mut crate::gamecube::GameCube, r: u8) {
    let a = ctx.dsp.registers.ac(r);
    let b = product(&ctx.dsp.registers);
    let result = a.wrapping_add(b);
    ctx.dsp.registers.set_ac(r, result);
    ctx.dsp.registers.update_flags_add(a, b, result);
}

/// Dynamic shift based on a 7-bit shift control value.
/// LOGICAL: right shift is unsigned. !LOGICAL: right shift is arithmetic.
#[inline(always)]
fn dynamic_shift<const LOGICAL: bool>(regs: &mut crate::flipper::dsp::core::Registers, d: u8, shift_val: i16) {
    if shift_val & 64 != 0 {
        let amount = (shift_val & 63) as u32;
        if amount != 0 {
            let ac = if LOGICAL {
                ((regs.ac(d) as u64 & 0xFF_FFFF_FFFF) >> (64 - amount)) as i64
            } else {
                regs.ac(d) >> (64 - amount)
            };
            regs.set_ac(d, ac as i64);
        }
    } else {
        let amount = (shift_val & 63) as u32;
        let ac = ((regs.ac(d) as u64 & 0xFF_FFFF_FFFF) << amount) as i64;
        regs.set_ac(d, ac);
    }
    let ac = regs.ac(d);
    regs.update_flags_ac(ac);
    regs.status.set_o(false);
    regs.status.set_c(false);
}

/// Get the ax0/ax1 operands for MULX instructions.
#[inline(always)]
fn mulx_operands(regs: &crate::flipper::dsp::core::Registers, s: u8, t: u8) -> (i16, i16) {
    let a = if s != 0 { regs.axh[0] } else { regs.ax[0] } as i16;
    let b = if t != 0 { regs.axh[1] } else { regs.ax[1] } as i16;
    (a, b)
}

/// Get the acS.m / axT.h operands for MULC instructions.
#[inline(always)]
fn mulc_operands(regs: &crate::flipper::dsp::core::Registers, s: u8, t: u8) -> (i16, i16) {
    let a = regs.ac_mid(s) as i16;
    let b = regs.axh[t as usize] as i16;
    (a, b)
}

#[inline(always)]
pub fn add_sub<const OP: u32>(
    ctx: &mut crate::gamecube::GameCube,
    instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_ADDR => {
            let ss = instr.ss() as usize;
            let d = instr.d_7_7();
            let a = ctx.dsp.registers.ac(d);
            let b = (ctx.dsp.registers.read::<true>(reg::AX0L + ss as u8) as i16 as i64) << 16;
            let result = a.wrapping_add(b);
            ctx.dsp.registers.set_ac(d, result);
            ctx.dsp.registers.update_flags_add(a, b, result);
        }
        OP_ADDAX => {
            let s = instr.s_6_6() as usize;
            let d = instr.d_7_7();
            let a = ctx.dsp.registers.ac(d);
            let b = ((ctx.dsp.registers.axh[s] as i16 as i64) << 16) | (ctx.dsp.registers.ax[s] as i64);
            let result = a.wrapping_add(b);
            ctx.dsp.registers.set_ac(d, result);
            ctx.dsp.registers.update_flags_add(a, b, result);
        }
        OP_ADD => {
            let d = instr.d_7_7();
            let a = ctx.dsp.registers.ac(d);
            let b = ctx.dsp.registers.ac(1 - d);
            let result = a.wrapping_add(b);
            ctx.dsp.registers.set_ac(d, result);
            ctx.dsp.registers.update_flags_add(a, b, result);
        }
        OP_ADDP => {
            let d = instr.d_7_7();
            let a = ctx.dsp.registers.ac(d);
            let b = product(&ctx.dsp.registers);
            let result = a.wrapping_add(b);
            ctx.dsp.registers.set_ac(d, result);
            ctx.dsp.registers.update_flags_add(a, b, result);
        }
        OP_SUBR => {
            let ss = instr.ss() as u8;
            let d = instr.d_7_7();
            let a = ctx.dsp.registers.ac(d);
            let b = (ctx.dsp.registers.read::<true>(reg::AX0L + ss) as i16 as i64) << 16;
            let result = a.wrapping_sub(b);
            ctx.dsp.registers.set_ac(d, result);
            ctx.dsp.registers.update_flags_sub(a, b, result);
        }
        OP_SUBAX => {
            let s = instr.s_6_6() as usize;
            let d = instr.d_7_7();
            let a = ctx.dsp.registers.ac(d);
            let b = ((ctx.dsp.registers.axh[s] as i16 as i64) << 16) | (ctx.dsp.registers.ax[s] as i64);
            let result = a.wrapping_sub(b);
            ctx.dsp.registers.set_ac(d, result);
            ctx.dsp.registers.update_flags_sub(a, b, result);
        }
        OP_SUB => {
            let d = instr.d_7_7();
            let a = ctx.dsp.registers.ac(d);
            let b = ctx.dsp.registers.ac(1 - d);
            let result = a.wrapping_sub(b);
            ctx.dsp.registers.set_ac(d, result);
            ctx.dsp.registers.update_flags_sub(a, b, result);
        }
        OP_SUBP => {
            let d = instr.d_7_7();
            let a = ctx.dsp.registers.ac(d);
            let b = product(&ctx.dsp.registers);
            let result = a.wrapping_sub(b);
            ctx.dsp.registers.set_ac(d, result);
            ctx.dsp.registers.update_flags_sub(a, b, result);
        }
        OP_ADDAXL => {
            let s = instr.s_6_6() as usize;
            let d = instr.d_7_7();
            let a = ctx.dsp.registers.ac(d);
            let b = ctx.dsp.registers.ax[s] as u16 as i64;
            let result = a.wrapping_add(b);
            ctx.dsp.registers.set_ac(d, result);
            ctx.dsp.registers.update_flags_add(a, b, result);
        }
        OP_ADDPAXZ => {
            let s = instr.s_6_6() as usize;
            let d = instr.d_7_7();
            let a = product(&ctx.dsp.registers);
            let b = (ctx.dsp.registers.axh[s] as i16 as i64) << 16;
            let result = a.wrapping_add(b) & !0xFFFFi64;
            ctx.dsp.registers.set_ac(d, result);
            ctx.dsp.registers.update_flags_ac(result);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(
                ((a as u64 & 0xFF_FFFF_FFFF).wrapping_add(b as u64 & 0xFF_FFFF_FFFF) & 0xFF_FFFF_FFFF)
                    < (a as u64 & 0xFF_FFFF_FFFF),
            );
        }
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn addr_reg<const OP: u32>(
    ctx: &mut crate::gamecube::GameCube,
    instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_DAR => {
            let d = instr.d_14_15() as usize;
            ctx.dsp.registers.ar[d] = ctx.dsp.registers.ar[d].wrapping_sub(1);
        }
        OP_IAR => {
            let d = instr.d_14_15() as usize;
            ctx.dsp.registers.ar[d] = ctx.dsp.registers.ar[d].wrapping_add(1);
        }
        OP_SUBARN => {
            let d = instr.d_14_15() as usize;
            ctx.dsp.registers.ar[d] = ctx.dsp.registers.ar[d].wrapping_sub(ctx.dsp.registers.ix[d]);
        }
        OP_ADDARN => {
            let s = instr.s_12_13() as usize;
            let d = instr.d_14_15() as usize;
            ctx.dsp.registers.ar[d] = ctx.dsp.registers.ar[d].wrapping_add(ctx.dsp.registers.ix[s]);
        }
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn cmp_test<const OP: u32>(
    ctx: &mut crate::gamecube::GameCube,
    instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_CMP => {
            let a = ctx.dsp.registers.ac(0);
            let b = ctx.dsp.registers.ac(1);
            let result = a.wrapping_sub(b);
            ctx.dsp.registers.update_flags_sub(a, b, result);
        }
        OP_CMPAXH => {
            let s = instr.s_4_4();
            let r = instr.s_3_3();
            let a = ctx.dsp.registers.ac(r);
            let b = (ctx.dsp.registers.axh[s as usize] as i16 as i64) << 16;
            let result = a.wrapping_sub(b);
            ctx.dsp.registers.update_flags_sub(a, b, result);
        }
        OP_TST => {
            let r = instr.r_4_4();
            let ac = ctx.dsp.registers.ac(r);
            ctx.dsp.registers.update_flags_ac(ac);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        OP_TSTPROD => {
            let prod = product(&ctx.dsp.registers);
            ctx.dsp.registers.update_flags_ac(prod);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        OP_TSTAXH => {
            let r = instr.r_7_7() as usize;
            let val = (ctx.dsp.registers.axh[r] as i16 as i64) << 16;
            ctx.dsp.registers.update_flags_ac(val);
            ctx.dsp.registers.status.set_as32(false);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        OP_NX_0 | OP_NX_1 => {}
        OP_CLR => {
            let r = instr.r_4_4();
            ctx.dsp.registers.set_ac(r, 0);
            ctx.dsp.registers.status.set_tb(true);
            ctx.dsp.registers.status.set_as32(false);
            ctx.dsp.registers.status.set_s(false);
            ctx.dsp.registers.status.set_z(true);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        OP_CLRP => {
            ctx.dsp.registers.product_low = 0x0000;
            ctx.dsp.registers.product_mid1 = 0xFFF0;
            ctx.dsp.registers.product_high = 0x00FF;
            ctx.dsp.registers.product_mid2 = 0x0010;
        }
        OP_CLRL => {
            let r = instr.r_7_7();
            let ac = ctx.dsp.registers.ac(r);
            let rounded = if (ac & 0x10000) != 0 {
                (ac.wrapping_add(0x8000)) & !0xFFFF
            } else {
                (ac.wrapping_add(0x7FFF)) & !0xFFFF
            };
            ctx.dsp.registers.set_ac(r, rounded);
            ctx.dsp.registers.update_flags_ac(rounded);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn control<const OP: u32>(
    ctx: &mut crate::gamecube::GameCube,
    instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_JCC => {
            let branch_control = BranchControl::from(instr.cond());
            if branch_control.evaluate(&ctx.dsp) {
                ctx.dsp.registers.nia = instr.addr();
            }
        }
        OP_NOP => {}
        OP_HALT => {
            ctx.dsp.csr.set_halt(true);
        }
        OP_IFCC => {
            let branch_control = BranchControl::from(instr.cond());
            if !branch_control.evaluate(&ctx.dsp) {
                ctx.dsp.registers.nia = ctx.dsp.registers.nia.wrapping_add(1);
            }
        }
        OP_CALLCC => {
            let branch_control = BranchControl::from(instr.cond());
            if branch_control.evaluate(&ctx.dsp) {
                ctx.dsp.registers.call_stack.push(ctx.dsp.registers.nia);
                ctx.dsp.registers.nia = instr.addr();
            }
        }
        OP_RETCC => {
            let branch_control = BranchControl::from(instr.cond());
            if branch_control.evaluate(&ctx.dsp) {
                ctx.dsp.registers.nia = ctx.dsp.registers.call_stack.pop();
            }
        }
        OP_RTICC => {
            let branch_control = BranchControl::from(instr.cond());
            if branch_control.evaluate(&ctx.dsp) {
                ctx.dsp.registers.status = StatusRegister::from(ctx.dsp.registers.data_stack.pop());
                ctx.dsp.registers.nia = ctx.dsp.registers.call_stack.pop();
            }
        }
        OP_JRCC => {
            let branch_control = BranchControl::from(instr.cond());
            if branch_control.evaluate(&ctx.dsp) {
                ctx.dsp.registers.nia = ctx.dsp.registers.read::<true>(instr.reg_8_10());
            }
        }
        OP_CALLRCC => {
            let branch_control = BranchControl::from(instr.cond());
            if branch_control.evaluate(&ctx.dsp) {
                ctx.dsp.registers.call_stack.push(ctx.dsp.registers.nia);
                ctx.dsp.registers.nia = ctx.dsp.registers.read::<true>(instr.reg_8_10());
            }
        }
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn imm_alu<const OP: u32>(
    ctx: &mut crate::gamecube::GameCube,
    instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_ADDI => {
            let d = instr.d_7_7();
            let a = ctx.dsp.registers.ac(d);
            let b = (instr.imm_16_31() as i16 as i64) << 16;
            let result = a.wrapping_add(b);
            ctx.dsp.registers.set_ac(d, result);
            ctx.dsp.registers.update_flags_add(a, b, result);
        }
        OP_XORI => {
            let d = instr.d_7_7();
            let result = ctx.dsp.registers.ac_mid(d) ^ instr.imm_16_31();
            ctx.dsp.registers.write::<false>(reg::AC0M + d, result);
            let ac = ctx.dsp.registers.ac(d);
            ctx.dsp.registers.update_flags_ac(ac);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        OP_ANDI => {
            let d = instr.d_7_7();
            let result = ctx.dsp.registers.ac_mid(d) & instr.imm_16_31();
            ctx.dsp.registers.write::<false>(reg::AC0M + d, result);
            let ac = ctx.dsp.registers.ac(d);
            ctx.dsp.registers.update_flags_ac(ac);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        OP_ORI => {
            let d = instr.d_7_7();
            let result = ctx.dsp.registers.ac_mid(d) | instr.imm_16_31();
            ctx.dsp.registers.write::<false>(reg::AC0M + d, result);
            let ac = ctx.dsp.registers.ac(d);
            ctx.dsp.registers.update_flags_ac(ac);
            ctx.dsp.registers.status.set_o(false);
        }
        OP_CMPI => {
            let d = instr.d_7_7();
            let a = ctx.dsp.registers.ac(d);
            let b = (instr.imm_16_31() as i16 as i64) << 16;
            let result = a.wrapping_sub(b);
            ctx.dsp.registers.update_flags_sub(a, b, result);
        }
        OP_ANDF | OP_ANDCF => {
            let imm = instr.imm_16_31();
            let result = ctx.dsp.registers.ac_mid(instr.d_7_7()) & imm;
            let lz = match OP {
                OP_ANDF => result == 0,
                OP_ANDCF => result == imm,
                _ => unreachable!(),
            };
            ctx.dsp.registers.status.set_lz(lz);
        }
        OP_ADDIS => {
            let d = instr.d_7_7();
            let a = ctx.dsp.registers.ac(d);
            let b = (instr.imm_8_15_i16() as i64) << 16;
            let result = a.wrapping_add(b);
            ctx.dsp.registers.set_ac(d, result);
            ctx.dsp.registers.update_flags_add(a, b, result);
        }
        OP_CMPIS => {
            let d = instr.d_7_7();
            let a = ctx.dsp.registers.ac(d);
            let b = (instr.imm_8_15_i16() as i64) << 16;
            let result = a.wrapping_sub(b);
            ctx.dsp.registers.update_flags_sub(a, b, result);
        }
        OP_LRIS => {
            let dst = reg::AX0L + instr.reg_5_7();
            let imm = instr.imm_8_15_i16() as u16;
            ctx.dsp.registers.write::<true>(dst, imm);
        }
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn inc_dec<const OP: u32>(
    ctx: &mut crate::gamecube::GameCube,
    instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_INCM => {
            let d = instr.d_7_7();
            let a = ctx.dsp.registers.ac(d);
            let result = a.wrapping_add(0x10000);
            ctx.dsp.registers.set_ac(d, result);
            ctx.dsp.registers.update_flags_add(a, 0x10000, result);
        }
        OP_INC => {
            let d = instr.d_7_7();
            let a = ctx.dsp.registers.ac(d);
            let result = a.wrapping_add(1);
            ctx.dsp.registers.set_ac(d, result);
            ctx.dsp.registers.update_flags_add(a, 1, result);
        }
        OP_DECM => {
            let d = instr.d_7_7();
            let a = ctx.dsp.registers.ac(d);
            let result = a.wrapping_sub(0x10000);
            ctx.dsp.registers.set_ac(d, result);
            ctx.dsp.registers.update_flags_sub(a, 0x10000, result);
        }
        OP_DEC => {
            let d = instr.d_7_7();
            let a = ctx.dsp.registers.ac(d);
            let result = a.wrapping_sub(1);
            ctx.dsp.registers.set_ac(d, result);
            ctx.dsp.registers.update_flags_sub(a, 1, result);
        }
        OP_NEG => {
            let d = instr.d_7_7();
            let a = ctx.dsp.registers.ac(d);
            let result = 0i64.wrapping_sub(a);
            ctx.dsp.registers.set_ac(d, result);
            ctx.dsp.registers.update_flags_sub(0, a, result);
        }
        OP_ABS_AC => {
            let d = instr.d_4_4();
            let a = ctx.dsp.registers.ac(d);
            if a < 0 {
                ctx.dsp.registers.set_ac(d, a.wrapping_neg());
            }
            let ac = ctx.dsp.registers.ac(d);
            ctx.dsp.registers.update_flags_ac(ac);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn load_store<const OP: u32>(
    ctx: &mut crate::gamecube::GameCube,
    instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_LRI => {
            ctx.dsp.registers.write::<true>(instr.d_11_15(), instr.imm_16_31());
        }
        OP_LR => {
            let value = ctx.dsp.read_dmem(instr.imm_16_31());
            ctx.dsp.registers.write::<true>(instr.d_11_15(), value);
        }
        OP_SR => {
            let value = ctx.dsp.registers.read::<true>(instr.d_11_15());
            ctx.dsp.write_dmem(instr.imm_16_31(), value);
        }
        OP_MRR => {
            let value = ctx.dsp.registers.read::<true>(instr.src());
            ctx.dsp.registers.write::<true>(instr.dst(), value);
        }
        OP_SI => {
            let addr = 0xFF00 | (instr.mem_8_15_u16());
            ctx.dsp.write_dmem(addr, instr.imm_16_31());
        }
        OP_LRR | OP_LRRD | OP_LRRI | OP_LRRN => {
            let s = instr.s_9_10() as usize;
            let d = instr.d_11_15();
            let value = ctx.dsp.read_dmem(ctx.dsp.registers.ar[s]);
            ctx.dsp.registers.write::<true>(d, value);
            match OP {
                OP_LRRD => ctx.dsp.registers.ar[s] = ctx.dsp.registers.ar[s].wrapping_sub(1),
                OP_LRRI => ctx.dsp.registers.ar[s] = ctx.dsp.registers.ar[s].wrapping_add(1),
                OP_LRRN => ctx.dsp.registers.ar[s] = ctx.dsp.registers.ar[s].wrapping_add(ctx.dsp.registers.ix[s]),
                _ => {}
            }
        }
        OP_SRR | OP_SRRD | OP_SRRI | OP_SRRN => {
            let d = instr.s_9_10() as usize;
            let value = ctx.dsp.registers.read::<true>(instr.d_11_15());
            ctx.dsp.write_dmem(ctx.dsp.registers.ar[d], value);
            match OP {
                OP_SRRD => ctx.dsp.registers.ar[d] = ctx.dsp.registers.ar[d].wrapping_sub(1),
                OP_SRRI => ctx.dsp.registers.ar[d] = ctx.dsp.registers.ar[d].wrapping_add(1),
                OP_SRRN => ctx.dsp.registers.ar[d] = ctx.dsp.registers.ar[d].wrapping_add(ctx.dsp.registers.ix[d]),
                _ => {}
            }
        }
        OP_LRS => {
            let dst = reg::AX0L + instr.reg_5_7();
            let addr = ((ctx.dsp.registers.config as u16) << 8) | instr.mem_8_15_u16();
            let value = ctx.dsp.read_dmem(addr);
            ctx.dsp.registers.write::<true>(dst, value);
        }
        OP_SRSH | OP_SRS => {
            let addr = ((ctx.dsp.registers.config as u16) << 8) | instr.mem_8_15_u16();
            let src = match OP {
                OP_SRSH => {
                    if instr.s_7_7() != 0 {
                        reg::AC1H
                    } else {
                        reg::AC0H
                    }
                }
                OP_SRS => reg::AC0L + instr.reg_6_7(),
                _ => unreachable!(),
            };
            let value = ctx.dsp.registers.read::<true>(src);
            ctx.dsp.write_dmem(addr, value);
        }
        OP_ILRR | OP_ILRRD | OP_ILRRI | OP_ILRRN => {
            let src = instr.s_14_15() as usize;
            let dst = if instr.d_7_7() != 0 { reg::AC1M } else { reg::AC0M };
            let value = ctx.dsp.read_imem(ctx.dsp.registers.ar[src]);
            ctx.dsp.registers.write::<true>(dst, value);
            match OP {
                OP_ILRRD => ctx.dsp.registers.ar[src] = ctx.dsp.registers.ar[src].wrapping_sub(1),
                OP_ILRRI => ctx.dsp.registers.ar[src] = ctx.dsp.registers.ar[src].wrapping_add(1),
                OP_ILRRN => {
                    ctx.dsp.registers.ar[src] = ctx.dsp.registers.ar[src].wrapping_add(ctx.dsp.registers.ix[src])
                }
                _ => {}
            }
        }
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn logic<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: crate::flipper::dsp::instruction::Instruction) {
    match OP {
        OP_XORR => {
            let s = instr.s_6_6() as usize;
            let d = instr.d_7_7();
            let result = ctx.dsp.registers.ac_mid(d) ^ ctx.dsp.registers.axh[s];
            ctx.dsp.registers.write::<false>(reg::AC0M + d, result);
            let ac = ctx.dsp.registers.ac(d);
            ctx.dsp.registers.update_flags_ac(ac);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        OP_ANDR => {
            let s = instr.s_6_6() as usize;
            let d = instr.d_7_7();
            let result = ctx.dsp.registers.ac_mid(d) & ctx.dsp.registers.axh[s];
            ctx.dsp.registers.write::<false>(reg::AC0M + d, result);
            let ac = ctx.dsp.registers.ac(d);
            ctx.dsp.registers.update_flags_ac(ac);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        OP_ORR => {
            let s = instr.s_6_6() as usize;
            let d = instr.d_7_7();
            let result = ctx.dsp.registers.ac_mid(d) | ctx.dsp.registers.axh[s];
            ctx.dsp.registers.write::<false>(reg::AC0M + d, result);
            let ac = ctx.dsp.registers.ac(d);
            ctx.dsp.registers.update_flags_ac(ac);
            ctx.dsp.registers.status.set_o(false);
        }
        OP_ANDC => {
            let d = instr.d_7_7();
            let result = ctx.dsp.registers.ac_mid(d) & ctx.dsp.registers.ac_mid(1 - d);
            ctx.dsp.registers.write::<false>(reg::AC0M + d, result);
            let ac = ctx.dsp.registers.ac(d);
            ctx.dsp.registers.update_flags_ac(ac);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        OP_ORC => {
            let d = instr.d_7_7();
            let result = ctx.dsp.registers.ac_mid(d) | ctx.dsp.registers.ac_mid(1 - d);
            ctx.dsp.registers.write::<false>(reg::AC0M + d, result);
        }
        OP_XORC => {
            let d = instr.d_7_7();
            let result = ctx.dsp.registers.ac_mid(d) ^ ctx.dsp.registers.ac_mid(1 - d);
            ctx.dsp.registers.write::<false>(reg::AC0M + d, result);
            let ac = ctx.dsp.registers.ac(d);
            ctx.dsp.registers.update_flags_ac(ac);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        OP_NOT_AC => {
            let d = instr.d_7_7();
            ctx.dsp
                .registers
                .write::<false>(reg::AC0M + d, !ctx.dsp.registers.ac_mid(d));
            let ac = ctx.dsp.registers.ac(d);
            ctx.dsp.registers.update_flags_ac(ac);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn loops<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: crate::flipper::dsp::instruction::Instruction) {
    match OP {
        OP_LOOP_REG | OP_LOOPI => {
            let counter = match OP {
                OP_LOOP_REG => ctx.dsp.registers.read::<true>(instr.reg_11_15()),
                OP_LOOPI => instr.imm_8_15_u8() as u16,
                _ => unreachable!(),
            };
            let end_addr = ctx.dsp.registers.nia;
            if counter != 0 {
                ctx.dsp.registers.call_stack.push(end_addr);
                ctx.dsp.registers.loop_addr.push(end_addr.wrapping_add(1));
                ctx.dsp.registers.loop_counter.push(counter);
            } else {
                ctx.dsp.registers.nia = end_addr.wrapping_add(1);
            }
        }
        OP_BLOOP | OP_BLOOPI => {
            let counter = match OP {
                OP_BLOOP => ctx.dsp.registers.read::<true>(instr.reg_11_15()),
                OP_BLOOPI => instr.imm_8_15_u8() as u16,
                _ => unreachable!(),
            };
            let end_addr = instr.addr();
            if counter != 0 {
                ctx.dsp.registers.call_stack.push(ctx.dsp.registers.nia);
                ctx.dsp.registers.loop_addr.push(end_addr.wrapping_add(1));
                ctx.dsp.registers.loop_counter.push(counter);
            } else {
                ctx.dsp.registers.nia = end_addr.wrapping_add(1);
            }
        }
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn move_ops<const OP: u32>(
    ctx: &mut crate::gamecube::GameCube,
    instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_MOVR => {
            let ss = instr.ss() as u8;
            let d = instr.d_7_7();
            let val = (ctx.dsp.registers.read::<true>(reg::AX0L + ss) as i16 as i64) << 16;
            ctx.dsp.registers.set_ac(d, val);
            ctx.dsp.registers.update_flags_ac(val);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        OP_MOVAX => {
            let s = instr.s_6_6() as usize;
            let d = instr.d_7_7();
            let val = ((ctx.dsp.registers.axh[s] as i16 as i64) << 16) | (ctx.dsp.registers.ax[s] as i64);
            ctx.dsp.registers.set_ac(d, val);
            ctx.dsp.registers.update_flags_ac(val);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        OP_MOV => {
            let d = instr.d_7_7();
            let val = ctx.dsp.registers.ac(1 - d);
            ctx.dsp.registers.set_ac(d, val);
            ctx.dsp.registers.update_flags_ac(val);
            ctx.dsp.registers.status.set_as32(false);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        OP_MOVP => {
            let d = instr.d_7_7();
            move_prod_to_ac(ctx, d);
        }
        OP_MOVPZ => {
            let d = instr.d_7_7();
            move_prod_to_ac_zero(ctx, d);
        }
        OP_MOVNP => {
            let d = instr.d_7_7();
            let val = product(&ctx.dsp.registers).wrapping_neg();
            ctx.dsp.registers.set_ac(d, val);
            ctx.dsp.registers.update_flags_ac(val);
            ctx.dsp.registers.status.set_o(false);
        }
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn mul<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: crate::flipper::dsp::instruction::Instruction) {
    match OP {
        OP_MUL => {
            let s = instr.r_4_4() as usize;
            let (a, b) = (ctx.dsp.registers.ax[s] as i16, ctx.dsp.registers.axh[s] as i16);
            multiply(&mut ctx.dsp.registers, a, b);
        }
        OP_MULX => {
            let (a, b) = mulx_operands(&ctx.dsp.registers, instr.s_3_3(), instr.t_4_4());
            multiply(&mut ctx.dsp.registers, a, b);
        }
        OP_MULC => {
            let (a, b) = mulc_operands(&ctx.dsp.registers, instr.s_3_3(), instr.t_4_4());
            multiply(&mut ctx.dsp.registers, a, b);
        }
        OP_MULAXH => {
            let (a, b) = (ctx.dsp.registers.axh[0] as i16, ctx.dsp.registers.ac0_mid as i16);
            multiply(&mut ctx.dsp.registers, a, b);
        }
        OP_MULMV => {
            let s = instr.r_4_4() as usize;
            let (a, b) = (ctx.dsp.registers.ax[s] as i16, ctx.dsp.registers.axh[s] as i16);
            move_prod_to_ac(ctx, instr.r_7_7());
            multiply(&mut ctx.dsp.registers, a, b);
        }
        OP_MULMVZ => {
            let s = instr.r_4_4() as usize;
            let (a, b) = (ctx.dsp.registers.ax[s] as i16, ctx.dsp.registers.axh[s] as i16);
            move_prod_to_ac_zero(ctx, instr.r_7_7());
            multiply(&mut ctx.dsp.registers, a, b);
        }
        OP_MULAC => {
            let s = instr.r_4_4() as usize;
            let (a, b) = (ctx.dsp.registers.ax[s] as i16, ctx.dsp.registers.axh[s] as i16);
            add_prod_to_ac(ctx, instr.r_7_7());
            multiply(&mut ctx.dsp.registers, a, b);
        }
        OP_MULXMV => {
            let (a, b) = mulx_operands(&ctx.dsp.registers, instr.s_3_3(), instr.t_4_4());
            move_prod_to_ac(ctx, instr.r_7_7());
            multiply(&mut ctx.dsp.registers, a, b);
        }
        OP_MULXMVZ => {
            let (a, b) = mulx_operands(&ctx.dsp.registers, instr.s_3_3(), instr.t_4_4());
            move_prod_to_ac_zero(ctx, instr.r_7_7());
            multiply(&mut ctx.dsp.registers, a, b);
        }
        OP_MULXAC => {
            let (a, b) = mulx_operands(&ctx.dsp.registers, instr.s_3_3(), instr.t_4_4());
            add_prod_to_ac(ctx, instr.r_7_7());
            multiply(&mut ctx.dsp.registers, a, b);
        }
        OP_MULCMV => {
            let (a, b) = mulc_operands(&ctx.dsp.registers, instr.s_3_3(), instr.t_4_4());
            move_prod_to_ac(ctx, instr.r_7_7());
            multiply(&mut ctx.dsp.registers, a, b);
        }
        OP_MULCMVZ => {
            let (a, b) = mulc_operands(&ctx.dsp.registers, instr.s_3_3(), instr.t_4_4());
            move_prod_to_ac_zero(ctx, instr.r_7_7());
            multiply(&mut ctx.dsp.registers, a, b);
        }
        OP_MULCAC => {
            let (a, b) = mulc_operands(&ctx.dsp.registers, instr.s_3_3(), instr.t_4_4());
            add_prod_to_ac(ctx, instr.r_7_7());
            multiply(&mut ctx.dsp.registers, a, b);
        }
        OP_MADD => {
            let s = instr.s_7_7() as usize;
            let (a, b) = (ctx.dsp.registers.ax[s] as i16, ctx.dsp.registers.axh[s] as i16);
            multiply_accumulate::<true>(&mut ctx.dsp.registers, a, b);
        }
        OP_MSUB => {
            let s = instr.s_7_7() as usize;
            let (a, b) = (ctx.dsp.registers.ax[s] as i16, ctx.dsp.registers.axh[s] as i16);
            multiply_accumulate::<false>(&mut ctx.dsp.registers, a, b);
        }
        OP_MADDX | OP_MSUBX => {
            let (a, b) = mulx_operands(&ctx.dsp.registers, instr.s_6_6(), instr.t_7_7());
            if matches!(OP, OP_MADDX) {
                multiply_accumulate::<true>(&mut ctx.dsp.registers, a, b);
            } else {
                multiply_accumulate::<false>(&mut ctx.dsp.registers, a, b);
            }
        }
        OP_MADDC | OP_MSUBC => {
            let (a, b) = mulc_operands(&ctx.dsp.registers, instr.s_6_6(), instr.t_7_7());
            if matches!(OP, OP_MADDC) {
                multiply_accumulate::<true>(&mut ctx.dsp.registers, a, b);
            } else {
                multiply_accumulate::<false>(&mut ctx.dsp.registers, a, b);
            }
        }
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn shifts<const OP: u32>(
    ctx: &mut crate::gamecube::GameCube,
    instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_LSL | OP_ASL => {
            let r = instr.r_7_7();
            let i = instr.n() as u32;
            let ac = ((ctx.dsp.registers.ac(r) as u64 & 0xFF_FFFF_FFFF) << i) as i64;
            ctx.dsp.registers.set_ac(r, ac);
            let ac = ctx.dsp.registers.ac(r);
            ctx.dsp.registers.update_flags_ac(ac);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        OP_LSR => {
            let r = instr.r_7_7();
            let i = instr.n() as u32;
            if i != 0 {
                let ac = (ctx.dsp.registers.ac(r) as u64 & 0xFF_FFFF_FFFF) >> (64 - i);
                ctx.dsp.registers.set_ac(r, ac as i64);
            }
            let ac = ctx.dsp.registers.ac(r);
            ctx.dsp.registers.update_flags_ac(ac);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        OP_ASR => {
            let r = instr.r_7_7();
            let i = instr.n() as u32;
            if i != 0 {
                let ac = ctx.dsp.registers.ac(r) >> (64 - i);
                ctx.dsp.registers.set_ac(r, ac);
            }
            let ac = ctx.dsp.registers.ac(r);
            ctx.dsp.registers.update_flags_ac(ac);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        OP_LSL16 => {
            let r = instr.r_7_7();
            let ac = ((ctx.dsp.registers.ac(r) as u64 & 0xFF_FFFF_FFFF) << 16) as i64;
            ctx.dsp.registers.set_ac(r, ac);
            let ac = ctx.dsp.registers.ac(r);
            ctx.dsp.registers.update_flags_ac(ac);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        OP_LSR16 => {
            let r = instr.r_7_7();
            let ac = (ctx.dsp.registers.ac(r) as u64 & 0xFF_FFFF_FFFF) >> 16;
            ctx.dsp.registers.set_ac(r, ac as i64);
            let ac = ctx.dsp.registers.ac(r);
            ctx.dsp.registers.update_flags_ac(ac);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        OP_ASR16 => {
            let r = instr.r_4_4();
            let ac = ctx.dsp.registers.ac(r) >> 16;
            ctx.dsp.registers.set_ac(r, ac);
            let ac = ctx.dsp.registers.ac(r);
            ctx.dsp.registers.update_flags_ac(ac);
            ctx.dsp.registers.status.set_o(false);
            ctx.dsp.registers.status.set_c(false);
        }
        OP_LSRN => {
            let sv = ctx.dsp.registers.ac1_mid as i16;
            dynamic_shift::<true>(&mut ctx.dsp.registers, 0, sv);
        }
        OP_ASRN => {
            let sv = ctx.dsp.registers.ac1_mid as i16;
            dynamic_shift::<false>(&mut ctx.dsp.registers, 0, sv);
        }
        OP_LSRNRX => {
            let s = instr.s_6_6() as usize;
            let d = instr.d_7_7();
            let sv = ctx.dsp.registers.axh[s] as i16;
            dynamic_shift::<true>(&mut ctx.dsp.registers, d, sv);
        }
        OP_ASRNRX => {
            let s = instr.s_6_6() as usize;
            let d = instr.d_7_7();
            let sv = ctx.dsp.registers.axh[s] as i16;
            dynamic_shift::<false>(&mut ctx.dsp.registers, d, sv);
        }
        OP_LSRNR => {
            let d = instr.d_7_7();
            let sv = ctx.dsp.registers.ac_mid(1 - d) as i16;
            dynamic_shift::<true>(&mut ctx.dsp.registers, d, sv);
        }
        OP_ASRNR => {
            let d = instr.d_7_7();
            let sv = ctx.dsp.registers.ac_mid(1 - d) as i16;
            dynamic_shift::<false>(&mut ctx.dsp.registers, d, sv);
        }
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn status<const OP: u32>(
    ctx: &mut crate::gamecube::GameCube,
    instr: crate::flipper::dsp::instruction::Instruction,
) {
    match OP {
        OP_SBCLR => {
            ctx.dsp.registers.status &= !(1 << (6 + instr.bit())) as u16;
        }
        OP_SBSET => {
            ctx.dsp.registers.status |= (1 << (6 + instr.bit())) as u16;
        }
        OP_M2 => {
            ctx.dsp.registers.status.set_am(false);
        }
        OP_M0 => {
            ctx.dsp.registers.status.set_am(true);
        }
        OP_CLR15 => {
            ctx.dsp.registers.status.set_su(false);
        }
        OP_SET15 => {
            ctx.dsp.registers.status.set_su(true);
        }
        OP_SET16 => {
            ctx.dsp.registers.status.set_sxm(SignExtensionMode::Bits16);
        }
        OP_SET40 => {
            ctx.dsp.registers.status.set_sxm(SignExtensionMode::Bits40);
        }
        _ => unreachable!(),
    }
}

// Extension opcode handlers
use crate::flipper::dsp::instruction::GcDspExt;

#[inline(always)]
pub fn ext_nop(_ctx: &mut crate::gamecube::GameCube, _instr: GcDspExt) {}

#[inline(always)]
pub fn ext_addr<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: GcDspExt) {
    let r = instr.r_6_7() as usize;
    match OP {
        OP_EXT_DR => ctx.dsp.registers.ar[r] = ctx.dsp.registers.ar[r].wrapping_sub(1),
        OP_EXT_IR => ctx.dsp.registers.ar[r] = ctx.dsp.registers.ar[r].wrapping_add(1),
        OP_EXT_NR => ctx.dsp.registers.ar[r] = ctx.dsp.registers.ar[r].wrapping_add(ctx.dsp.registers.ix[r]),
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn ext_mv(ctx: &mut crate::gamecube::GameCube, instr: GcDspExt) {
    let d = instr.d_4_5() as u8;
    let s = instr.s_6_7() as u8;
    let value = ctx.dsp.registers.read::<true>(reg::AX0L + s);
    ctx.dsp.registers.write::<true>(reg::AX0L + d, value);
}

#[inline(always)]
pub fn ext_store<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: GcDspExt) {
    let s = instr.s_3_4() as usize;
    let d = instr.d_6_7() as u8;
    let value = ctx.dsp.registers.read::<true>(reg::AC0M + d);
    ctx.dsp.write_dmem(ctx.dsp.registers.ar[s], value);
    match OP {
        OP_EXT_S => {}
        OP_EXT_SN => ctx.dsp.registers.ar[s] = ctx.dsp.registers.ar[s].wrapping_add(ctx.dsp.registers.ix[s]),
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn ext_load<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: GcDspExt) {
    let d = instr.d_2_4() as u8;
    let s = instr.s_6_7() as usize;
    let value = ctx.dsp.read_dmem(ctx.dsp.registers.ar[s]);
    ctx.dsp.registers.write::<true>(reg::AX0L + d, value);
    match OP {
        OP_EXT_L => {}
        OP_EXT_LN => ctx.dsp.registers.ar[s] = ctx.dsp.registers.ar[s].wrapping_add(ctx.dsp.registers.ix[s]),
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn ext_load_store<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: GcDspExt) {
    let s = instr.s_7_7() as usize;
    let d = instr.d_2_3() as u8;

    let store_value = ctx.dsp.registers.read::<true>(reg::AC0M + s as u8);
    ctx.dsp.write_dmem(ctx.dsp.registers.ar[3], store_value);

    let load_value = ctx.dsp.read_dmem(ctx.dsp.registers.ar[d as usize]);
    ctx.dsp.registers.write::<true>(reg::AX0H + d, load_value);

    match OP {
        OP_EXT_LS => {
            ctx.dsp.registers.ar[d as usize] =
                ctx.dsp.registers.ar[d as usize].wrapping_add(ctx.dsp.registers.ix[d as usize]);
            ctx.dsp.registers.ar[3] = ctx.dsp.registers.ar[3].wrapping_add(ctx.dsp.registers.ix[3]);
        }
        OP_EXT_LSM => {
            ctx.dsp.registers.ar[d as usize] =
                ctx.dsp.registers.ar[d as usize].wrapping_add(ctx.dsp.registers.ix[d as usize]);
            ctx.dsp.registers.ar[3] = ctx.dsp.registers.ar[3].wrapping_sub(1);
        }
        OP_EXT_LSN => {
            ctx.dsp.registers.ar[d as usize] = ctx.dsp.registers.ar[d as usize].wrapping_sub(1);
            ctx.dsp.registers.ar[3] = ctx.dsp.registers.ar[3].wrapping_add(ctx.dsp.registers.ix[3]);
        }
        OP_EXT_LSNM => {
            ctx.dsp.registers.ar[d as usize] = ctx.dsp.registers.ar[d as usize].wrapping_sub(1);
            ctx.dsp.registers.ar[3] = ctx.dsp.registers.ar[3].wrapping_sub(1);
        }
        _ => unreachable!(),
    }
}

#[inline(always)]
pub fn ext_ld<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: GcDspExt) {
    let d = instr.d_2_2() as u8;
    let r = instr.r_3_3() as usize;
    let s = instr.s_6_7();

    let value0 = ctx.dsp.read_dmem(ctx.dsp.registers.ar[0]);
    ctx.dsp.registers.write::<true>(reg::AX0L + d * 2, value0);

    let value1 = ctx.dsp.read_dmem(ctx.dsp.registers.ar[3]);
    ctx.dsp.registers.write::<true>(reg::AX0L + d * 2 + 1, value1);

    match OP {
        OP_EXT_LD_00 => {
            ctx.dsp.registers.ar[0] = ctx.dsp.registers.ar[0].wrapping_add(ctx.dsp.registers.ix[0]);
            ctx.dsp.registers.ar[3] = ctx.dsp.registers.ar[3].wrapping_add(ctx.dsp.registers.ix[3]);
        }
        OP_EXT_LDM_10 => {
            ctx.dsp.registers.ar[0] = ctx.dsp.registers.ar[0].wrapping_add(ctx.dsp.registers.ix[0]);
            ctx.dsp.registers.ar[3] = ctx.dsp.registers.ar[3].wrapping_sub(1);
        }
        OP_EXT_LDN_01 => {
            ctx.dsp.registers.ar[0] = ctx.dsp.registers.ar[0].wrapping_sub(1);
            ctx.dsp.registers.ar[3] = ctx.dsp.registers.ar[3].wrapping_add(ctx.dsp.registers.ix[3]);
        }
        OP_EXT_LDNM_11 => {
            ctx.dsp.registers.ar[0] = ctx.dsp.registers.ar[0].wrapping_sub(1);
            ctx.dsp.registers.ar[3] = ctx.dsp.registers.ar[3].wrapping_sub(1);
        }
        _ => unreachable!(),
    }

    let _ = (r, s);
}

#[inline(always)]
pub fn ext_ldax<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: GcDspExt) {
    let r = instr.r_3_3() as usize;
    let s = instr.s_2_2();

    let value0 = ctx.dsp.read_dmem(ctx.dsp.registers.ar[0]);
    ctx.dsp
        .registers
        .write::<true>(if s != 0 { reg::AX1L } else { reg::AX0L }, value0);

    let value1 = ctx.dsp.read_dmem(ctx.dsp.registers.ar[3]);
    ctx.dsp
        .registers
        .write::<true>(if s != 0 { reg::AX1H } else { reg::AX0H }, value1);

    match OP {
        OP_EXT_LDAX => {
            ctx.dsp.registers.ar[0] = ctx.dsp.registers.ar[0].wrapping_add(ctx.dsp.registers.ix[0]);
            ctx.dsp.registers.ar[3] = ctx.dsp.registers.ar[3].wrapping_add(ctx.dsp.registers.ix[3]);
        }
        OP_EXT_LDAXM => {
            ctx.dsp.registers.ar[0] = ctx.dsp.registers.ar[0].wrapping_add(ctx.dsp.registers.ix[0]);
            ctx.dsp.registers.ar[3] = ctx.dsp.registers.ar[3].wrapping_sub(1);
        }
        OP_EXT_LDAXN => {
            ctx.dsp.registers.ar[0] = ctx.dsp.registers.ar[0].wrapping_sub(1);
            ctx.dsp.registers.ar[3] = ctx.dsp.registers.ar[3].wrapping_add(ctx.dsp.registers.ix[3]);
        }
        OP_EXT_LDAXNM => {
            ctx.dsp.registers.ar[0] = ctx.dsp.registers.ar[0].wrapping_sub(1);
            ctx.dsp.registers.ar[3] = ctx.dsp.registers.ar[3].wrapping_sub(1);
        }
        _ => unreachable!(),
    }

    let _ = r;
}
