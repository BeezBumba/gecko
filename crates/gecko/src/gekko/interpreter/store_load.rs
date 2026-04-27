use crate::gekko::condition::ConditionField;

#[inline(always)]
pub fn store_load<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    match OP {
        crate::gekko::lut::OP_STW | crate::gekko::lut::OP_STWU => {
            let addr = ctx.gekko.read_gpr_or_zero(instr.ra()).wrapping_add_signed(instr.disp());
            ctx.write_u32(addr, ctx.gekko.read_gpr(instr.rs()));
            if OP == crate::gekko::lut::OP_STWU {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_STH | crate::gekko::lut::OP_STHU => {
            let addr = ctx.gekko.read_gpr_or_zero(instr.ra()).wrapping_add_signed(instr.disp());
            ctx.write_u16(addr, ctx.gekko.read_gpr(instr.rs()) as u16);
            if OP == crate::gekko::lut::OP_STHU {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_STB | crate::gekko::lut::OP_STBU => {
            let addr = ctx.gekko.read_gpr_or_zero(instr.ra()).wrapping_add_signed(instr.disp());
            ctx.write_u8(addr, ctx.gekko.read_gpr(instr.rs()) as u8);
            if OP == crate::gekko::lut::OP_STBU {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_LWZ | crate::gekko::lut::OP_LWZU => {
            let addr = ctx.gekko.read_gpr_or_zero(instr.ra()).wrapping_add_signed(instr.disp());
            let val = ctx.read_u32(addr);
            ctx.gekko.write_gpr(instr.rd(), val);
            if OP == crate::gekko::lut::OP_LWZU {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_LBZ | crate::gekko::lut::OP_LBZU => {
            let addr = ctx.gekko.read_gpr_or_zero(instr.ra()).wrapping_add_signed(instr.disp());
            let val = ctx.read_u8(addr) as u32;
            ctx.gekko.write_gpr(instr.rd(), val);
            if OP == crate::gekko::lut::OP_LBZU {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_LHZ | crate::gekko::lut::OP_LHZU => {
            let addr = ctx.gekko.read_gpr_or_zero(instr.ra()).wrapping_add_signed(instr.disp());
            let val = ctx.read_u16(addr) as u32;
            ctx.gekko.write_gpr(instr.rd(), val);
            if OP == crate::gekko::lut::OP_LHZU {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_LHA | crate::gekko::lut::OP_LHAU => {
            let addr = ctx.gekko.read_gpr_or_zero(instr.ra()).wrapping_add_signed(instr.disp());
            let val = ctx.read_u16(addr) as i16 as i32 as u32;
            ctx.gekko.write_gpr(instr.rd(), val);
            if OP == crate::gekko::lut::OP_LHAU {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_LMW => {
            let mut addr = ctx.gekko.read_gpr_or_zero(instr.ra()).wrapping_add_signed(instr.disp());
            for r in instr.rd()..32 {
                let val = ctx.read_u32(addr);
                ctx.gekko.write_gpr(r, val);
                addr = addr.wrapping_add(4);
            }
        }
        crate::gekko::lut::OP_STMW => {
            let mut addr = ctx.gekko.read_gpr_or_zero(instr.ra()).wrapping_add_signed(instr.disp());
            for r in instr.rs()..32 {
                let val = ctx.gekko.read_gpr(r);
                ctx.write_u32(addr, val);
                addr = addr.wrapping_add(4);
            }
        }
        crate::gekko::lut::OP_LWZX | crate::gekko::lut::OP_LWZUX => {
            let addr = ctx
                .gekko
                .read_gpr_or_zero(instr.ra())
                .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
            let val = ctx.read_u32(addr);
            ctx.gekko.write_gpr(instr.rd(), val);
            if OP == crate::gekko::lut::OP_LWZUX {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_LBZX | crate::gekko::lut::OP_LBZUX => {
            let addr = ctx
                .gekko
                .read_gpr_or_zero(instr.ra())
                .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
            let val = ctx.read_u8(addr) as u32;
            ctx.gekko.write_gpr(instr.rd(), val);
            if OP == crate::gekko::lut::OP_LBZUX {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_LHZX | crate::gekko::lut::OP_LHZUX => {
            let addr = ctx
                .gekko
                .read_gpr_or_zero(instr.ra())
                .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
            let val = ctx.read_u16(addr) as u32;
            ctx.gekko.write_gpr(instr.rd(), val);
            if OP == crate::gekko::lut::OP_LHZUX {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_LHAX | crate::gekko::lut::OP_LHAUX => {
            let addr = ctx
                .gekko
                .read_gpr_or_zero(instr.ra())
                .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
            let val = ctx.read_u16(addr) as i16 as i32 as u32;
            ctx.gekko.write_gpr(instr.rd(), val);
            if OP == crate::gekko::lut::OP_LHAUX {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_STWX | crate::gekko::lut::OP_STWUX => {
            let addr = ctx
                .gekko
                .read_gpr_or_zero(instr.ra())
                .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
            ctx.write_u32(addr, ctx.gekko.read_gpr(instr.rs()));
            if OP == crate::gekko::lut::OP_STWUX {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_STBX | crate::gekko::lut::OP_STBUX => {
            let addr = ctx
                .gekko
                .read_gpr_or_zero(instr.ra())
                .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
            ctx.write_u8(addr, ctx.gekko.read_gpr(instr.rs()) as u8);
            if OP == crate::gekko::lut::OP_STBUX {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_STHX | crate::gekko::lut::OP_STHUX => {
            let addr = ctx
                .gekko
                .read_gpr_or_zero(instr.ra())
                .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
            ctx.write_u16(addr, ctx.gekko.read_gpr(instr.rs()) as u16);
            if OP == crate::gekko::lut::OP_STHUX {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_LWBRX => {
            let addr = ctx
                .gekko
                .read_gpr_or_zero(instr.ra())
                .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
            let val = ctx.read_u32(addr).swap_bytes();
            ctx.gekko.write_gpr(instr.rd(), val);
        }
        crate::gekko::lut::OP_LHBRX => {
            let addr = ctx
                .gekko
                .read_gpr_or_zero(instr.ra())
                .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
            let val = ctx.read_u16(addr).swap_bytes() as u32;
            ctx.gekko.write_gpr(instr.rd(), val);
        }
        crate::gekko::lut::OP_STWBRX => {
            let addr = ctx
                .gekko
                .read_gpr_or_zero(instr.ra())
                .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
            ctx.write_u32(addr, ctx.gekko.read_gpr(instr.rs()).swap_bytes());
        }
        crate::gekko::lut::OP_STHBRX => {
            let addr = ctx
                .gekko
                .read_gpr_or_zero(instr.ra())
                .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
            ctx.write_u16(addr, (ctx.gekko.read_gpr(instr.rs()) as u16).swap_bytes());
        }
        _ => todo!("Store/Load instruction with OP = {OP:#x}"),
    }
}

#[inline(always)]
pub fn lwarx(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    let addr = ctx
        .gekko
        .read_gpr_or_zero(instr.ra())
        .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
    let val = ctx.read_u32(addr);
    ctx.gekko.write_gpr(instr.rd(), val);
    ctx.gekko.reserve_addr = Some(addr);
}

#[inline(always)]
pub fn stwcx_dot(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    let addr = ctx
        .gekko
        .read_gpr_or_zero(instr.ra())
        .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
    let so = ctx.gekko.spr.xer.summary_overflow();
    let store_performed = ctx.gekko.reserve_addr.is_some();
    ctx.gekko.reserve_addr = None;
    if store_performed {
        ctx.write_u32(addr, ctx.gekko.read_gpr(instr.rs()));
        ctx.gekko.cr.set_cr0(ConditionField::new().with_eq(true).with_so(so));
    } else {
        ctx.gekko.cr.set_cr0(ConditionField::new().with_so(so));
    }
}

#[inline(always)]
pub fn store_load_fp<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    if !ctx.check_fp_available() {
        return;
    }

    match OP {
        crate::gekko::lut::OP_LFD | crate::gekko::lut::OP_LFDU => {
            let addr = ctx.gekko.read_gpr_or_zero(instr.ra()).wrapping_add_signed(instr.disp());
            let val = ctx.read_f64(addr);
            ctx.gekko.write_fpr(instr.rd(), val);
            if OP == crate::gekko::lut::OP_LFDU {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_STFD | crate::gekko::lut::OP_STFDU => {
            let addr = ctx.gekko.read_gpr_or_zero(instr.ra()).wrapping_add_signed(instr.disp());
            ctx.write_f64(addr, ctx.gekko.read_fpr(instr.rs()));
            if OP == crate::gekko::lut::OP_STFDU {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_LFS | crate::gekko::lut::OP_LFSU => {
            let addr = ctx.gekko.read_gpr_or_zero(instr.ra()).wrapping_add_signed(instr.disp());
            let val = ctx.read_f32(addr);
            ctx.gekko.write_fpr(instr.rd(), val);
            ctx.gekko.write_ps1(instr.rd(), val);
            if OP == crate::gekko::lut::OP_LFSU {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_STFS | crate::gekko::lut::OP_STFSU => {
            let addr = ctx.gekko.read_gpr_or_zero(instr.ra()).wrapping_add_signed(instr.disp());
            ctx.write_f32(addr, ctx.gekko.read_fpr(instr.rs()));
            if OP == crate::gekko::lut::OP_STFSU {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_LFSX | crate::gekko::lut::OP_LFSUX => {
            let addr = ctx
                .gekko
                .read_gpr_or_zero(instr.ra())
                .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
            let val = ctx.read_f32(addr);
            ctx.gekko.write_fpr(instr.rd(), val);
            ctx.gekko.write_ps1(instr.rd(), val);
            if OP == crate::gekko::lut::OP_LFSUX {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_LFDX | crate::gekko::lut::OP_LFDUX => {
            let addr = ctx
                .gekko
                .read_gpr_or_zero(instr.ra())
                .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
            let val = ctx.read_f64(addr);
            ctx.gekko.write_fpr(instr.rd(), val);
            if OP == crate::gekko::lut::OP_LFDUX {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_STFSX | crate::gekko::lut::OP_STFSUX => {
            let addr = ctx
                .gekko
                .read_gpr_or_zero(instr.ra())
                .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
            ctx.write_f32(addr, ctx.gekko.read_fpr(instr.rs()));
            if OP == crate::gekko::lut::OP_STFSUX {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_STFDX | crate::gekko::lut::OP_STFDUX => {
            let addr = ctx
                .gekko
                .read_gpr_or_zero(instr.ra())
                .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
            ctx.write_f64(addr, ctx.gekko.read_fpr(instr.rs()));
            if OP == crate::gekko::lut::OP_STFDUX {
                ctx.gekko.write_gpr(instr.ra(), addr);
            }
        }
        crate::gekko::lut::OP_STFIWX => {
            let addr = ctx
                .gekko
                .read_gpr_or_zero(instr.ra())
                .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
            ctx.write_u32(addr, ctx.gekko.read_fpr(instr.rs()).to_bits() as u32);
        }
        _ => todo!("FP Store/Load instruction with OP = {OP:#x}"),
    }
}

#[inline(always)]
pub fn lswx(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    let ea = ctx
        .gekko
        .read_gpr_or_zero(instr.ra())
        .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
    let mut n = ctx.gekko.spr.xer.byte_count() as u32;
    if n == 0 {
        return;
    }
    let mut r = (instr.rd() as u32).wrapping_sub(1) & 31;
    let mut i = 0u32;
    let mut addr = ea;
    while n > 0 {
        if i == 0 {
            r = (r + 1) & 31;
            ctx.gekko.write_gpr(r as u8, 0);
        }
        let byte = ctx.read_u8(addr) as u32;
        let shift = 24 - i;
        let val = ctx.gekko.read_gpr(r as u8) | (byte << shift);
        ctx.gekko.write_gpr(r as u8, val);
        i += 8;
        if i == 32 {
            i = 0;
        }
        addr = addr.wrapping_add(1);
        n -= 1;
    }
}

#[inline(always)]
pub fn stswx(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    let ea = ctx
        .gekko
        .read_gpr_or_zero(instr.ra())
        .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
    let mut n = ctx.gekko.spr.xer.byte_count() as u32;
    let mut r = (instr.rs() as u32).wrapping_sub(1) & 31;
    let mut i = 0u32;
    let mut addr = ea;
    while n > 0 {
        if i == 0 {
            r = (r + 1) & 31;
        }
        let byte = (ctx.gekko.read_gpr(r as u8) >> (24 - i)) as u8;
        ctx.write_u8(addr, byte);
        i += 8;
        if i == 32 {
            i = 0;
        }
        addr = addr.wrapping_add(1);
        n -= 1;
    }
}

#[inline(always)]
pub fn lswi(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    let ea = ctx.gekko.read_gpr_or_zero(instr.ra());
    let nb = instr.nb();
    let mut n = if nb == 0 { 32u32 } else { nb as u32 };
    let mut r = (instr.rd() as u32).wrapping_sub(1) & 31;
    let mut i = 0u32;
    let mut addr = ea;
    while n > 0 {
        if i == 0 {
            r = (r + 1) & 31;
            ctx.gekko.write_gpr(r as u8, 0);
        }
        let byte = ctx.read_u8(addr) as u32;
        let shift = 24 - i;
        let val = ctx.gekko.read_gpr(r as u8) | (byte << shift);
        ctx.gekko.write_gpr(r as u8, val);
        i += 8;
        if i == 32 {
            i = 0;
        }
        addr = addr.wrapping_add(1);
        n -= 1;
    }
}

#[inline(always)]
pub fn stswi(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    let ea = ctx.gekko.read_gpr_or_zero(instr.ra());
    let nb = instr.nb();
    let mut n = if nb == 0 { 32u32 } else { nb as u32 };
    let mut r = (instr.rs() as u32).wrapping_sub(1) & 31;
    let mut i = 0u32;
    let mut addr = ea;
    while n > 0 {
        if i == 0 {
            r = (r + 1) & 31;
        }
        let byte = (ctx.gekko.read_gpr(r as u8) >> (24 - i)) as u8;
        ctx.write_u8(addr, byte);
        i += 8;
        if i == 32 {
            i = 0;
        }
        addr = addr.wrapping_add(1);
        n -= 1;
    }
}

#[inline(always)]
pub fn eciwx(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    let ea = ctx
        .gekko
        .read_gpr_or_zero(instr.ra())
        .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
    let val = ctx.read_u32(ea);
    ctx.gekko.write_gpr(instr.rd(), val);
}

#[inline(always)]
pub fn ecowx(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    let ea = ctx
        .gekko
        .read_gpr_or_zero(instr.ra())
        .wrapping_add(ctx.gekko.read_gpr(instr.rb()));
    let val = ctx.gekko.read_gpr(instr.rs());
    ctx.write_u32(ea, val);
}
