use crate::gekko::condition::ConditionField;
use crate::gekko::instruction::Instruction;
use crate::gekko::lut::*;
use crate::system::{System, SystemId};

#[inline(always)]
fn ea_disp<const SYSTEM: SystemId>(ctx: &System<SYSTEM>, ra: u8, disp: i32) -> u32 {
    ctx.gekko.read_gpr_or_zero(ra).wrapping_add_signed(disp)
}

#[inline(always)]
fn ea_index<const SYSTEM: SystemId>(ctx: &System<SYSTEM>, ra: u8, rb: u8) -> u32 {
    ctx.gekko
        .read_gpr_or_zero(ra)
        .wrapping_add(ctx.gekko.read_gpr(rb))
}

#[inline(always)]
pub fn store_load<const OP: u32, const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, instr: Instruction) {
    let ra = instr.ra();
    let rb = instr.rb();
    let rs = instr.rs();
    let rd = instr.rd();
    let disp = instr.disp();

    match OP {
        OP_STW | OP_STWU => {
            let addr = ea_disp(ctx, ra, disp);
            ctx.write_u32(addr, ctx.gekko.read_gpr(rs));
            if OP == OP_STWU {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_STH | OP_STHU => {
            let addr = ea_disp(ctx, ra, disp);
            ctx.write_u16(addr, ctx.gekko.read_gpr(rs) as u16);
            if OP == OP_STHU {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_STB | OP_STBU => {
            let addr = ea_disp(ctx, ra, disp);
            ctx.write_u8(addr, ctx.gekko.read_gpr(rs) as u8);
            if OP == OP_STBU {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_LWZ | OP_LWZU => {
            let addr = ea_disp(ctx, ra, disp);
            let val = ctx.read_u32(addr);
            ctx.gekko.write_gpr(rd, val);
            if OP == OP_LWZU {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_LBZ | OP_LBZU => {
            let addr = ea_disp(ctx, ra, disp);
            let val = ctx.read_u8(addr) as u32;
            ctx.gekko.write_gpr(rd, val);
            if OP == OP_LBZU {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_LHZ | OP_LHZU => {
            let addr = ea_disp(ctx, ra, disp);
            let val = ctx.read_u16(addr) as u32;
            ctx.gekko.write_gpr(rd, val);
            if OP == OP_LHZU {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_LHA | OP_LHAU => {
            let addr = ea_disp(ctx, ra, disp);
            let val = ctx.read_u16(addr) as i16 as i32 as u32;
            ctx.gekko.write_gpr(rd, val);
            if OP == OP_LHAU {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_LMW => {
            let mut addr = ea_disp(ctx, ra, disp);
            for r in rd..32 {
                let val = ctx.read_u32(addr);
                ctx.gekko.write_gpr(r, val);
                addr = addr.wrapping_add(4);
            }
        }
        OP_STMW => {
            let mut addr = ea_disp(ctx, ra, disp);
            for r in rs..32 {
                let val = ctx.gekko.read_gpr(r);
                ctx.write_u32(addr, val);
                addr = addr.wrapping_add(4);
            }
        }
        OP_LWZX | OP_LWZUX => {
            let addr = ea_index(ctx, ra, rb);
            let val = ctx.read_u32(addr);
            ctx.gekko.write_gpr(rd, val);
            if OP == OP_LWZUX {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_LBZX | OP_LBZUX => {
            let addr = ea_index(ctx, ra, rb);
            let val = ctx.read_u8(addr) as u32;
            ctx.gekko.write_gpr(rd, val);
            if OP == OP_LBZUX {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_LHZX | OP_LHZUX => {
            let addr = ea_index(ctx, ra, rb);
            let val = ctx.read_u16(addr) as u32;
            ctx.gekko.write_gpr(rd, val);
            if OP == OP_LHZUX {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_LHAX | OP_LHAUX => {
            let addr = ea_index(ctx, ra, rb);
            let val = ctx.read_u16(addr) as i16 as i32 as u32;
            ctx.gekko.write_gpr(rd, val);
            if OP == OP_LHAUX {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_STWX | OP_STWUX => {
            let addr = ea_index(ctx, ra, rb);
            ctx.write_u32(addr, ctx.gekko.read_gpr(rs));
            if OP == OP_STWUX {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_STBX | OP_STBUX => {
            let addr = ea_index(ctx, ra, rb);
            ctx.write_u8(addr, ctx.gekko.read_gpr(rs) as u8);
            if OP == OP_STBUX {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_STHX | OP_STHUX => {
            let addr = ea_index(ctx, ra, rb);
            ctx.write_u16(addr, ctx.gekko.read_gpr(rs) as u16);
            if OP == OP_STHUX {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_LWBRX => {
            let addr = ea_index(ctx, ra, rb);
            let val = ctx.read_u32(addr).swap_bytes();
            ctx.gekko.write_gpr(rd, val);
        }
        OP_LHBRX => {
            let addr = ea_index(ctx, ra, rb);
            let val = ctx.read_u16(addr).swap_bytes() as u32;
            ctx.gekko.write_gpr(rd, val);
        }
        OP_STWBRX => {
            let addr = ea_index(ctx, ra, rb);
            ctx.write_u32(addr, ctx.gekko.read_gpr(rs).swap_bytes());
        }
        OP_STHBRX => {
            let addr = ea_index(ctx, ra, rb);
            ctx.write_u16(addr, (ctx.gekko.read_gpr(rs) as u16).swap_bytes());
        }
        _ => todo!("Store/Load instruction with OP = {OP:#x}"),
    }
}

#[inline(always)]
pub fn lwarx<const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, instr: Instruction) {
    let addr = ea_index(ctx, instr.ra(), instr.rb());
    let val = ctx.read_u32(addr);
    ctx.gekko.write_gpr(instr.rd(), val);
    ctx.gekko.reserve_addr = addr;
}

#[inline(always)]
pub fn stwcx_dot<const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, instr: Instruction) {
    let addr = ea_index(ctx, instr.ra(), instr.rb());
    let so = ctx.gekko.spr.xer.summary_overflow();
    let store_performed = ctx.gekko.reserve_addr == addr;
    if store_performed {
        ctx.write_u32(addr, ctx.gekko.read_gpr(instr.rs()));
        ctx.gekko.reserve_addr = crate::gekko::Gekko::NO_RESERVATION;
        ctx.gekko.cr.set_cr0(ConditionField::new().with_eq(true).with_so(so));
    } else {
        ctx.gekko.cr.set_cr0(ConditionField::new().with_so(so));
    }
}

#[inline(always)]
pub fn store_load_fp<const OP: u32, const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, instr: Instruction) {
    if !ctx.check_fp_available() {
        return;
    }

    let ra = instr.ra();
    let rb = instr.rb();
    let rs = instr.rs();
    let rd = instr.rd();
    let disp = instr.disp();

    match OP {
        OP_LFD | OP_LFDU => {
            let addr = ea_disp(ctx, ra, disp);
            let val = ctx.read_f64(addr);
            ctx.gekko.write_fpr(rd, val);
            if OP == OP_LFDU {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_STFD | OP_STFDU => {
            let addr = ea_disp(ctx, ra, disp);
            ctx.write_f64(addr, ctx.gekko.read_fpr(rs));
            if OP == OP_STFDU {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_LFS | OP_LFSU => {
            let addr = ea_disp(ctx, ra, disp);
            let val = ctx.read_f32(addr);
            ctx.gekko.write_fpr(rd, val);
            ctx.gekko.write_ps1(rd, val);
            if OP == OP_LFSU {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_STFS | OP_STFSU => {
            let addr = ea_disp(ctx, ra, disp);
            ctx.write_f32(addr, ctx.gekko.read_fpr(rs));
            if OP == OP_STFSU {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_LFSX | OP_LFSUX => {
            let addr = ea_index(ctx, ra, rb);
            let val = ctx.read_f32(addr);
            ctx.gekko.write_fpr(rd, val);
            ctx.gekko.write_ps1(rd, val);
            if OP == OP_LFSUX {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_LFDX | OP_LFDUX => {
            let addr = ea_index(ctx, ra, rb);
            let val = ctx.read_f64(addr);
            ctx.gekko.write_fpr(rd, val);
            if OP == OP_LFDUX {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_STFSX | OP_STFSUX => {
            let addr = ea_index(ctx, ra, rb);
            ctx.write_f32(addr, ctx.gekko.read_fpr(rs));
            if OP == OP_STFSUX {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_STFDX | OP_STFDUX => {
            let addr = ea_index(ctx, ra, rb);
            ctx.write_f64(addr, ctx.gekko.read_fpr(rs));
            if OP == OP_STFDUX {
                ctx.gekko.write_gpr(ra, addr);
            }
        }
        OP_STFIWX => {
            let addr = ea_index(ctx, ra, rb);
            ctx.write_u32(addr, ctx.gekko.read_fpr(rs).to_bits() as u32);
        }
        _ => todo!("FP Store/Load instruction with OP = {OP:#x}"),
    }
}

#[inline(always)]
pub fn lswx<const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, instr: Instruction) {
    let ea = ctx
        .gekko
        .read_gpr_or_zero(instr.ra())
        .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
    let mut n = ctx.gekko.spr.xer.byte_count() as u32;
    if n == 0 {
        return;
    }
    let mut r = (instr.rd() as u32).wrapping_sub(1) & 31;
    let mut shift = 0u32;
    let mut reg_val = 0u32;
    let mut addr = ea;
    while n > 0 {
        if shift == 0 {
            r = (r + 1) & 31;
            reg_val = 0;
        }
        let byte = ctx.read_u8(addr) as u32;
        reg_val |= byte << (24 - shift);
        shift += 8;
        if shift == 32 {
            ctx.gekko.write_gpr(r as u8, reg_val);
            shift = 0;
        }
        addr = addr.wrapping_add(1);
        n -= 1;
    }

    if shift != 0 {
        ctx.gekko.write_gpr(r as u8, reg_val);
    }
}

#[inline(always)]
pub fn stswx<const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, instr: Instruction) {
    let ea = ctx
        .gekko
        .read_gpr_or_zero(instr.ra())
        .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
    let mut n = ctx.gekko.spr.xer.byte_count() as u32;
    let mut r = (instr.rs() as u32).wrapping_sub(1) & 31;
    let mut shift = 0u32;
    let mut reg_val = 0u32;
    let mut addr = ea;
    while n > 0 {
        if shift == 0 {
            r = (r + 1) & 31;
            reg_val = ctx.gekko.read_gpr(r as u8);
        }
        let byte = (reg_val >> (24 - shift)) as u8;
        ctx.write_u8(addr, byte);
        shift += 8;
        if shift == 32 {
            shift = 0;
        }
        addr = addr.wrapping_add(1);
        n -= 1;
    }
}

#[inline(always)]
pub fn lswi<const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, instr: Instruction) {
    let ea = ctx.gekko.read_gpr_or_zero(instr.ra());
    let nb = instr.nb();
    let mut n = if nb == 0 { 32u32 } else { nb as u32 };
    let mut r = (instr.rd() as u32).wrapping_sub(1) & 31;
    let mut shift = 0u32;
    let mut reg_val = 0u32;
    let mut addr = ea;
    while n > 0 {
        if shift == 0 {
            r = (r + 1) & 31;
            reg_val = 0;
        }
        let byte = ctx.read_u8(addr) as u32;
        reg_val |= byte << (24 - shift);
        shift += 8;
        if shift == 32 {
            ctx.gekko.write_gpr(r as u8, reg_val);
            shift = 0;
        }
        addr = addr.wrapping_add(1);
        n -= 1;
    }

    if shift != 0 {
        ctx.gekko.write_gpr(r as u8, reg_val);
    }
}

#[inline(always)]
pub fn stswi<const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, instr: Instruction) {
    let ea = ctx.gekko.read_gpr_or_zero(instr.ra());
    let nb = instr.nb();
    let mut n = if nb == 0 { 32u32 } else { nb as u32 };
    let mut r = (instr.rs() as u32).wrapping_sub(1) & 31;
    let mut shift = 0u32;
    let mut reg_val = 0u32;
    let mut addr = ea;
    while n > 0 {
        if shift == 0 {
            r = (r + 1) & 31;
            reg_val = ctx.gekko.read_gpr(r as u8);
        }
        let byte = (reg_val >> (24 - shift)) as u8;
        ctx.write_u8(addr, byte);
        shift += 8;
        if shift == 32 {
            shift = 0;
        }
        addr = addr.wrapping_add(1);
        n -= 1;
    }
}

#[inline(always)]
pub fn eciwx<const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, instr: Instruction) {
    let ea = ea_index(ctx, instr.ra(), instr.rb());
    let val = ctx.read_u32(ea);
    ctx.gekko.write_gpr(instr.rd(), val);
}

#[inline(always)]
pub fn ecowx<const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, instr: Instruction) {
    let ea = ea_index(ctx, instr.ra(), instr.rb());
    let val = ctx.gekko.read_gpr(instr.rs());
    ctx.write_u32(ea, val);
}
