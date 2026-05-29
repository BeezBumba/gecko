use crate::gekko::instruction::Instruction;
use crate::gekko::lut::*;
use crate::system::{System, SystemId};

#[inline(always)]
fn eval_bo<const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, bo: u8, bi: u8) -> bool {
    let cond = ctx.gekko.cr.get_bit(bi);
    match bo & 0b11110 {
        0b00000 => {
            let ctr = ctx.gekko.spr.ctr.wrapping_sub(1);
            ctx.gekko.spr.ctr = ctr;
            ctr != 0 && !cond
        }
        0b00010 => {
            let ctr = ctx.gekko.spr.ctr.wrapping_sub(1);
            ctx.gekko.spr.ctr = ctr;
            ctr == 0 && !cond
        }
        0b00100 | 0b00110 => !cond,
        0b01000 => {
            let ctr = ctx.gekko.spr.ctr.wrapping_sub(1);
            ctx.gekko.spr.ctr = ctr;
            ctr != 0 && cond
        }
        0b01010 => {
            let ctr = ctx.gekko.spr.ctr.wrapping_sub(1);
            ctx.gekko.spr.ctr = ctr;
            ctr == 0 && cond
        }
        0b01100 | 0b01110 => cond,
        0b10000 | 0b11000 => {
            let ctr = ctx.gekko.spr.ctr.wrapping_sub(1);
            ctx.gekko.spr.ctr = ctr;
            ctr != 0
        }
        0b10010 | 0b11010 => {
            let ctr = ctx.gekko.spr.ctr.wrapping_sub(1);
            ctx.gekko.spr.ctr = ctr;
            ctr == 0
        }
        _ => true,
    }
}

#[inline(always)]
pub fn branch<const OP: u32, const SYSTEM: SystemId>(ctx: &mut System<SYSTEM>, instr: Instruction) {
    ctx.scheduler.cycles += crate::gekko::cycles::cycles_for_op(OP) as u64;
    match OP {
        OP_BX => {
            ctx.gekko.nia = if instr.aa() {
                instr.li() as u32
            } else {
                ctx.gekko.cia.wrapping_add_signed(instr.li())
            };
            if instr.lk() {
                ctx.gekko.spr.lr = ctx.gekko.cia.wrapping_add(4);
            }
        }
        OP_BCX => {
            if !eval_bo(ctx, instr.bo(), instr.bi()) {
                return;
            }

            ctx.gekko.nia = if instr.aa() {
                instr.bd() as u32
            } else {
                ctx.gekko.cia.wrapping_add_signed(instr.bd())
            };
            if instr.lk() {
                ctx.gekko.spr.lr = ctx.gekko.cia.wrapping_add(4);
            }
        }
        OP_BCLRX => {
            if !eval_bo(ctx, instr.bo(), instr.bi()) {
                return;
            }

            ctx.gekko.nia = ctx.gekko.spr.lr & !3;
            if instr.lk() {
                ctx.gekko.spr.lr = ctx.gekko.cia.wrapping_add(4);
            }
        }
        OP_BCCTRX => {
            let bo = instr.bo();
            let condition = (bo & 0x10) != 0 || (ctx.gekko.cr.get_bit(instr.bi()) == ((bo & 0x08) != 0));
            if !condition {
                return;
            }

            ctx.gekko.nia = ctx.gekko.spr.ctr & !3;
            if instr.lk() {
                ctx.gekko.spr.lr = ctx.gekko.cia.wrapping_add(4);
            }
        }
        _ => todo!("branch instruction with OP = {OP:#x}"),
    };
}
