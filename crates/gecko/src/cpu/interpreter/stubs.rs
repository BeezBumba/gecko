macro_rules! stub {
    ($($name:ident),* $(,)?) => {
        $(
            #[rustfmt::skip]
            #[inline(always)]
            pub fn $name(
                _ctx: &mut crate::gamecube::GameCube,
                _instr: crate::cpu::instruction::Instruction,
            ) {
                todo!(stringify!($name))
            }
        )*
    };
}

stub! {
    tw,
    mtsrin, mcrxr,
    lswi, mfsrin,
    stswi,
    eciwx, ecowx,
}

#[inline(always)]
pub fn lswx(ctx: &mut crate::gamecube::GameCube, instr: crate::cpu::instruction::Instruction) {
    let ea = ctx
        .cpu
        .read_gpr_or_zero(instr.ra())
        .wrapping_add(ctx.cpu.read_gpr(instr.rb()));
    let mut n = ctx.cpu.spr.xer.byte_count() as u32;
    if n == 0 {
        return;
    }
    let mut r = (instr.rd() as u32).wrapping_sub(1) & 31;
    let mut i = 0u32;
    let mut addr = ea;
    while n > 0 {
        if i == 0 {
            r = (r + 1) & 31;
            ctx.cpu.write_gpr(r as u8, 0);
        }
        let byte = ctx.read_u8(addr) as u32;
        let shift = 24 - i;
        let val = ctx.cpu.read_gpr(r as u8) | (byte << shift);
        ctx.cpu.write_gpr(r as u8, val);
        i += 8;
        if i == 32 {
            i = 0;
        }
        addr = addr.wrapping_add(1);
        n -= 1;
    }
}

#[inline(always)]
pub fn stswx(ctx: &mut crate::gamecube::GameCube, instr: crate::cpu::instruction::Instruction) {
    let ea = ctx
        .cpu
        .read_gpr_or_zero(instr.ra())
        .wrapping_add(ctx.cpu.read_gpr(instr.rb()));
    let mut n = ctx.cpu.spr.xer.byte_count() as u32;
    let mut r = (instr.rs() as u32).wrapping_sub(1) & 31;
    let mut i = 0u32;
    let mut addr = ea;
    while n > 0 {
        if i == 0 {
            r = (r + 1) & 31;
        }
        let byte = (ctx.cpu.read_gpr(r as u8) >> (24 - i)) as u8;
        ctx.write_u8(addr, byte);
        i += 8;
        if i == 32 {
            i = 0;
        }
        addr = addr.wrapping_add(1);
        n -= 1;
    }
}
