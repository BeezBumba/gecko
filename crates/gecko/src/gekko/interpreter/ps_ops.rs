use crate::gekko::condition::ConditionField;

#[inline(always)]
pub fn ps_ops<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    if !ctx.check_fp_available() {
        return;
    }

    match OP {
        crate::gekko::lut::OP_PS_ADD => {
            let ps0 = (ctx.gekko.read_fpr(instr.ra()) + ctx.gekko.read_fpr(instr.rb())) as f32 as f64;
            let ps1 = (ctx.gekko.read_ps1(instr.ra()) + ctx.gekko.read_ps1(instr.rb())) as f32 as f64;
            ps_write(ctx, &instr, ps0, ps1);
        }
        crate::gekko::lut::OP_PS_SUB => {
            let ps0 = (ctx.gekko.read_fpr(instr.ra()) - ctx.gekko.read_fpr(instr.rb())) as f32 as f64;
            let ps1 = (ctx.gekko.read_ps1(instr.ra()) - ctx.gekko.read_ps1(instr.rb())) as f32 as f64;
            ps_write(ctx, &instr, ps0, ps1);
        }
        crate::gekko::lut::OP_PS_MUL => {
            let ps0 = (ctx.gekko.read_fpr(instr.ra()) * ctx.gekko.read_fpr(instr.fc())) as f32 as f64;
            let ps1 = (ctx.gekko.read_ps1(instr.ra()) * ctx.gekko.read_ps1(instr.fc())) as f32 as f64;
            ps_write(ctx, &instr, ps0, ps1);
        }
        crate::gekko::lut::OP_PS_DIV => {
            let ps0 = (ctx.gekko.read_fpr(instr.ra()) / ctx.gekko.read_fpr(instr.rb())) as f32 as f64;
            let ps1 = (ctx.gekko.read_ps1(instr.ra()) / ctx.gekko.read_ps1(instr.rb())) as f32 as f64;
            ps_write(ctx, &instr, ps0, ps1);
        }
        crate::gekko::lut::OP_PS_MADD => {
            let ps0 = (ctx.gekko.read_fpr(instr.ra()) * ctx.gekko.read_fpr(instr.fc()) + ctx.gekko.read_fpr(instr.rb()))
                as f32 as f64;
            let ps1 = (ctx.gekko.read_ps1(instr.ra()) * ctx.gekko.read_ps1(instr.fc()) + ctx.gekko.read_ps1(instr.rb()))
                as f32 as f64;
            ps_write(ctx, &instr, ps0, ps1);
        }
        crate::gekko::lut::OP_PS_MSUB => {
            let ps0 = (ctx.gekko.read_fpr(instr.ra()) * ctx.gekko.read_fpr(instr.fc()) - ctx.gekko.read_fpr(instr.rb()))
                as f32 as f64;
            let ps1 = (ctx.gekko.read_ps1(instr.ra()) * ctx.gekko.read_ps1(instr.fc()) - ctx.gekko.read_ps1(instr.rb()))
                as f32 as f64;
            ps_write(ctx, &instr, ps0, ps1);
        }
        crate::gekko::lut::OP_PS_NMADD => {
            let ps0 = -(ctx.gekko.read_fpr(instr.ra()) * ctx.gekko.read_fpr(instr.fc()) + ctx.gekko.read_fpr(instr.rb()))
                as f32 as f64;
            let ps1 = -(ctx.gekko.read_ps1(instr.ra()) * ctx.gekko.read_ps1(instr.fc()) + ctx.gekko.read_ps1(instr.rb()))
                as f32 as f64;
            ps_write(ctx, &instr, ps0, ps1);
        }
        crate::gekko::lut::OP_PS_NMSUB => {
            let ps0 = -(ctx.gekko.read_fpr(instr.ra()) * ctx.gekko.read_fpr(instr.fc()) - ctx.gekko.read_fpr(instr.rb()))
                as f32 as f64;
            let ps1 = -(ctx.gekko.read_ps1(instr.ra()) * ctx.gekko.read_ps1(instr.fc()) - ctx.gekko.read_ps1(instr.rb()))
                as f32 as f64;
            ps_write(ctx, &instr, ps0, ps1);
        }
        crate::gekko::lut::OP_PS_MULS0 => {
            let c0 = ctx.gekko.read_fpr(instr.fc());
            let ps0 = (ctx.gekko.read_fpr(instr.ra()) * c0) as f32 as f64;
            let ps1 = (ctx.gekko.read_ps1(instr.ra()) * c0) as f32 as f64;
            ps_write(ctx, &instr, ps0, ps1);
        }
        crate::gekko::lut::OP_PS_MULS1 => {
            let c1 = ctx.gekko.read_ps1(instr.fc());
            let ps0 = (ctx.gekko.read_fpr(instr.ra()) * c1) as f32 as f64;
            let ps1 = (ctx.gekko.read_ps1(instr.ra()) * c1) as f32 as f64;
            ps_write(ctx, &instr, ps0, ps1);
        }
        crate::gekko::lut::OP_PS_MADDS0 => {
            let c0 = ctx.gekko.read_fpr(instr.fc());
            let ps0 = (ctx.gekko.read_fpr(instr.ra()) * c0 + ctx.gekko.read_fpr(instr.rb())) as f32 as f64;
            let ps1 = (ctx.gekko.read_ps1(instr.ra()) * c0 + ctx.gekko.read_ps1(instr.rb())) as f32 as f64;
            ps_write(ctx, &instr, ps0, ps1);
        }
        crate::gekko::lut::OP_PS_MADDS1 => {
            let c1 = ctx.gekko.read_ps1(instr.fc());
            let ps0 = (ctx.gekko.read_fpr(instr.ra()) * c1 + ctx.gekko.read_fpr(instr.rb())) as f32 as f64;
            let ps1 = (ctx.gekko.read_ps1(instr.ra()) * c1 + ctx.gekko.read_ps1(instr.rb())) as f32 as f64;
            ps_write(ctx, &instr, ps0, ps1);
        }
        crate::gekko::lut::OP_PS_SUM0 => {
            let ps0 = (ctx.gekko.read_fpr(instr.ra()) + ctx.gekko.read_ps1(instr.rb())) as f32 as f64;
            let ps1 = ctx.gekko.read_ps1(instr.fc());
            ps_write(ctx, &instr, ps0, ps1);
        }
        crate::gekko::lut::OP_PS_SUM1 => {
            let ps0 = ctx.gekko.read_fpr(instr.fc());
            let ps1 = (ctx.gekko.read_fpr(instr.ra()) + ctx.gekko.read_ps1(instr.rb())) as f32 as f64;
            ps_write(ctx, &instr, ps0, ps1);
        }
        crate::gekko::lut::OP_PS_MR => {
            ps_write(ctx, &instr, ctx.gekko.read_fpr(instr.rb()), ctx.gekko.read_ps1(instr.rb()));
        }
        crate::gekko::lut::OP_PS_NEG => {
            ps_write(
                ctx,
                &instr,
                -ctx.gekko.read_fpr(instr.rb()),
                -ctx.gekko.read_ps1(instr.rb()),
            );
        }
        crate::gekko::lut::OP_PS_ABS => {
            ps_write(
                ctx,
                &instr,
                ctx.gekko.read_fpr(instr.rb()).abs(),
                ctx.gekko.read_ps1(instr.rb()).abs(),
            );
        }
        crate::gekko::lut::OP_PS_NABS => {
            ps_write(
                ctx,
                &instr,
                -ctx.gekko.read_fpr(instr.rb()).abs(),
                -ctx.gekko.read_ps1(instr.rb()).abs(),
            );
        }
        crate::gekko::lut::OP_PS_MERGE00 => {
            ps_write(ctx, &instr, ctx.gekko.read_fpr(instr.ra()), ctx.gekko.read_fpr(instr.rb()));
        }
        crate::gekko::lut::OP_PS_MERGE01 => {
            ps_write(ctx, &instr, ctx.gekko.read_fpr(instr.ra()), ctx.gekko.read_ps1(instr.rb()));
        }
        crate::gekko::lut::OP_PS_MERGE10 => {
            ps_write(ctx, &instr, ctx.gekko.read_ps1(instr.ra()), ctx.gekko.read_fpr(instr.rb()));
        }
        crate::gekko::lut::OP_PS_MERGE11 => {
            ps_write(ctx, &instr, ctx.gekko.read_ps1(instr.ra()), ctx.gekko.read_ps1(instr.rb()));
        }
        crate::gekko::lut::OP_PS_SEL => {
            let ps0 = if ctx.gekko.read_fpr(instr.ra()) >= 0.0 {
                ctx.gekko.read_fpr(instr.fc())
            } else {
                ctx.gekko.read_fpr(instr.rb())
            };
            let ps1 = if ctx.gekko.read_ps1(instr.ra()) >= 0.0 {
                ctx.gekko.read_ps1(instr.fc())
            } else {
                ctx.gekko.read_ps1(instr.rb())
            };
            ps_write(ctx, &instr, ps0, ps1);
        }
        crate::gekko::lut::OP_PS_CMPU0 | crate::gekko::lut::OP_PS_CMPO0 => {
            let fa = ctx.gekko.read_fpr(instr.ra());
            let fb = ctx.gekko.read_fpr(instr.rb());
            ctx.gekko.cr.set_field(instr.crfd(), fp_compare(fa, fb));
        }
        crate::gekko::lut::OP_PS_CMPU1 | crate::gekko::lut::OP_PS_CMPO1 => {
            let fa = ctx.gekko.read_ps1(instr.ra());
            let fb = ctx.gekko.read_ps1(instr.rb());
            ctx.gekko.cr.set_field(instr.crfd(), fp_compare(fa, fb));
        }
        crate::gekko::lut::OP_PS_RES => {
            let ps0 = (1.0f32 / ctx.gekko.read_fpr(instr.rb()) as f32) as f64;
            let ps1 = (1.0f32 / ctx.gekko.read_ps1(instr.rb()) as f32) as f64;
            ps_write(ctx, &instr, ps0, ps1);
        }
        crate::gekko::lut::OP_PS_RSQRTE => {
            let ps0 = (1.0f32 / (ctx.gekko.read_fpr(instr.rb()) as f32).sqrt()) as f64;
            let ps1 = (1.0f32 / (ctx.gekko.read_ps1(instr.rb()) as f32).sqrt()) as f64;
            ps_write(ctx, &instr, ps0, ps1);
        }
        _ => todo!("PS instruction with OP = {OP:#x}"),
    }

    ctx.check_fp_program_exception();
}

#[inline(always)]
fn ps_write(ctx: &mut crate::gamecube::GameCube, instr: &crate::gekko::instruction::Instruction, ps0: f64, ps1: f64) {
    ctx.gekko.write_fpr(instr.rd(), ps0);
    ctx.gekko.write_ps1(instr.rd(), ps1);
    if instr.rc() {
        ctx.gekko.update_cr1();
    }
}

#[inline(always)]
fn fp_compare(a: f64, b: f64) -> ConditionField {
    if a.is_nan() || b.is_nan() {
        ConditionField::new().with_so(true)
    } else if a < b {
        ConditionField::new().with_lt(true)
    } else if a > b {
        ConditionField::new().with_gt(true)
    } else {
        ConditionField::new().with_eq(true)
    }
}
