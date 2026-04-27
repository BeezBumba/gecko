use crate::gekko::condition::ConditionField;

#[inline(always)]
pub fn mcrxr(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    let xer = ctx.gekko.spr.xer;
    let field = ConditionField::new()
        .with_lt(xer.summary_overflow())
        .with_gt(xer.overflow())
        .with_eq(xer.carry())
        .with_so(false);
    ctx.gekko.cr.set_field(instr.crfd(), field);
    ctx.gekko.spr.xer = xer.with_summary_overflow(false).with_overflow(false).with_carry(false);
}

#[inline(always)]
pub fn cr_ops<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    match OP {
        crate::gekko::lut::OP_MTCRF => {
            let crm = instr.crm();
            let rs = ctx.gekko.read_gpr(instr.rs());
            let mut cr = ctx.gekko.cr.raw();
            for i in 0u8..8 {
                if crm & (1 << (7 - i)) != 0 {
                    let shift = (7 - i) * 4;
                    let mask = 0xFu32 << shift;
                    cr = (cr & !mask) | (rs & mask);
                }
            }
            ctx.gekko.cr = crate::gekko::condition::ConditionRegister::from(cr);
        }
        crate::gekko::lut::OP_MFCR => {
            ctx.gekko.write_gpr(instr.rd(), ctx.gekko.cr.raw());
        }
        crate::gekko::lut::OP_MCRF => {
            let src = ctx.gekko.cr.get_field(instr.crfs());
            ctx.gekko.cr.set_field(instr.crfd(), src);
        }
        // CR bit operations
        _ => {
            let a = ctx.gekko.cr.get_bit(instr.crba());
            let b = ctx.gekko.cr.get_bit(instr.crbb());
            let result = match OP {
                crate::gekko::lut::OP_CRXOR => a ^ b,
                crate::gekko::lut::OP_CROR => a | b,
                crate::gekko::lut::OP_CRAND => a & b,
                crate::gekko::lut::OP_CREQV => a == b,
                crate::gekko::lut::OP_CRNOR => !(a | b),
                crate::gekko::lut::OP_CRNAND => !(a & b),
                crate::gekko::lut::OP_CRANDC => a & !b,
                crate::gekko::lut::OP_CRORC => a | !b,
                _ => todo!("CR instruction with OP = {OP:#x}"),
            };
            ctx.gekko.cr.set_bit(instr.crbd(), result);
        }
    }
}
