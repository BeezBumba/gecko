use crate::gekko::condition::BranchControl;

#[inline(always)]
pub fn branch<const OP: u32>(ctx: &mut crate::gamecube::GameCube, instr: crate::gekko::instruction::Instruction) {
    // Read LR before potentially overwriting LR with CIA+4 (matters for blrl/bctrl)
    let old_lr = ctx.gekko.spr.lr;

    if instr.lk() {
        ctx.gekko.spr.lr = ctx.gekko.cia.wrapping_add(4);
    }

    match OP {
        crate::gekko::lut::OP_BX => {
            ctx.gekko.nia = if instr.aa() {
                instr.li() as u32
            } else {
                ctx.gekko.cia.wrapping_add_signed(instr.li())
            }
        }
        crate::gekko::lut::OP_BCLRX | crate::gekko::lut::OP_BCX | crate::gekko::lut::OP_BCCTRX => {
            let ctrl = BranchControl::from_bo(instr.bo());
            tracing::trace!("Branch control: {ctrl:?}");

            if ctrl.should_decrement_ctr() {
                ctx.gekko.spr.ctr = ctx.gekko.spr.ctr.wrapping_sub(1);
            }

            let condition = ctx.gekko.cr.get_bit(instr.bi());
            if !ctrl.should_branch(ctx.gekko.spr.ctr, condition) {
                return;
            }

            match OP {
                crate::gekko::lut::OP_BCLRX => ctx.gekko.nia = old_lr,
                crate::gekko::lut::OP_BCX => {
                    ctx.gekko.nia = if instr.aa() {
                        instr.bd() as u32
                    } else {
                        ctx.gekko.cia.wrapping_add_signed(instr.bd())
                    }
                }
                crate::gekko::lut::OP_BCCTRX => ctx.gekko.nia = ctx.gekko.spr.ctr,
                _ => tracing::error!("missing OP = {OP:#x}"),
            }
        }
        _ => todo!("branch instruction with OP = {OP:#x}"),
    };
}
